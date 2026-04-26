//! Tauri commands and background poller for system monitoring.

use crate::sysmon_analysis;
use crate::sysmon_collect::{Collector, ProcessInfo, SystemSnapshot};
use crate::state::AppState;
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::RwLock;
use uuid::Uuid;

type AppStateHandle = Arc<RwLock<AppState>>;

// ── Threshold settings ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SysmonSettings {
    pub cpu_warn: f32,
    pub cpu_critical: f32,
    pub ram_warn: f32,
    pub ram_critical: f32,
    pub disk_warn: f32,
    pub disk_critical: f32,
    pub gpu_warn: f32,
    pub gpu_critical: f32,
}

impl Default for SysmonSettings {
    fn default() -> Self {
        Self {
            cpu_warn: 75.0,
            cpu_critical: 90.0,
            ram_warn: 80.0,
            ram_critical: 92.0,
            disk_warn: 80.0,
            disk_critical: 90.0,
            gpu_warn: 85.0,
            gpu_critical: 95.0,
        }
    }
}

// ── Alert ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SysmonAlert {
    pub id: String,
    pub fired_at: String,
    pub metric: String,
    pub value: f64,
    pub threshold: f64,
    pub severity: String,
    pub detail: Option<String>,
    pub resolved_at: Option<String>,
}

// ── Analysis ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SysmonAnalysisSummary {
    pub id: String,
    pub created_at: String,
    pub trigger: String,
    pub question: Option<String>,
    pub response: String,
}

// ── Commands ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn sysmon_get_current(
    state: State<'_, AppStateHandle>,
) -> Result<serde_json::Value, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let snapshot: Option<SystemSnapshot> = conn.query_row(
        "SELECT cpu_pct, ram_used_mb, ram_total_mb, swap_used_mb, load_avg_1,
                disks_json, gpus_json, net_json
         FROM sysmon_snapshots ORDER BY sampled_at DESC LIMIT 1",
        [],
        |r| {
            Ok(SystemSnapshot {
                cpu_pct: r.get::<_, f64>(0)? as f32,
                ram_used_mb: r.get(1)?,
                ram_total_mb: r.get(2)?,
                swap_used_mb: r.get(3)?,
                load_avg_1: r.get(4)?,
                disks: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(),
                gpus: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(),
                net: serde_json::from_str(&r.get::<_, String>(7)?).unwrap_or_default(),
            })
        },
    ).ok();

    let processes: Vec<ProcessInfo> = {
        let mut stmt = conn.prepare(
            "SELECT pid, name, cpu_pct, ram_mb, status, user_name
             FROM sysmon_processes
             WHERE sampled_at = (SELECT MAX(sampled_at) FROM sysmon_processes)
             ORDER BY cpu_pct DESC",
        ).map_err(|e| e.to_string())?;
        stmt.query_map([], |r| {
            Ok(ProcessInfo {
                pid: r.get::<_, i64>(0)? as u32,
                name: r.get(1)?,
                cpu_pct: r.get::<_, f64>(2)? as f32,
                ram_mb: r.get::<_, i64>(3)? as u64,
                status: r.get(4)?,
                user_name: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect()
    };

    Ok(serde_json::json!({ "snapshot": snapshot, "processes": processes }))
}

#[tauri::command]
pub async fn sysmon_get_history(
    state: State<'_, AppStateHandle>,
    metric: String,
    hours: u32,
) -> Result<Vec<serde_json::Value>, String> {
    let hours = hours.min(720);
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let cutoff = Utc::now()
        .checked_sub_signed(chrono::Duration::hours(hours as i64))
        .unwrap_or_else(Utc::now)
        .to_rfc3339();

    let col = match metric.as_str() {
        "cpu" => "cpu_pct",
        "ram" => "CAST(ram_used_mb AS REAL) / CAST(ram_total_mb AS REAL) * 100",
        "swap" => "swap_used_mb",
        _ => "cpu_pct",
    };

    let sql = format!(
        "SELECT sampled_at, {} AS value FROM sysmon_snapshots
         WHERE sampled_at > ?1 ORDER BY sampled_at ASC",
        col
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows: Vec<serde_json::Value> = stmt
        .query_map(params![cutoff], |r| {
            Ok(serde_json::json!({ "t": r.get::<_,String>(0)?, "v": r.get::<_,f64>(1)? }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

#[tauri::command]
pub async fn sysmon_list_alerts(
    state: State<'_, AppStateHandle>,
    limit: Option<u32>,
) -> Result<Vec<SysmonAlert>, String> {
    let limit = limit.unwrap_or(50).min(500);
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT id, fired_at, metric, value, threshold, severity, detail, resolved_at
         FROM sysmon_alerts ORDER BY fired_at DESC LIMIT ?1",
    ).map_err(|e| e.to_string())?;

    let rows: Vec<SysmonAlert> = stmt
        .query_map(params![limit], |r| {
            Ok(SysmonAlert {
                id: r.get(0)?,
                fired_at: r.get(1)?,
                metric: r.get(2)?,
                value: r.get(3)?,
                threshold: r.get(4)?,
                severity: r.get(5)?,
                detail: r.get(6)?,
                resolved_at: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

#[tauri::command]
pub async fn sysmon_resolve_alert(
    state: State<'_, AppStateHandle>,
    id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE sysmon_alerts SET resolved_at = ?1 WHERE id = ?2",
        params![now, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn sysmon_list_processes(
    state: State<'_, AppStateHandle>,
) -> Result<Vec<ProcessInfo>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT pid, name, cpu_pct, ram_mb, status, user_name
         FROM sysmon_processes
         WHERE sampled_at = (SELECT MAX(sampled_at) FROM sysmon_processes)
         ORDER BY cpu_pct DESC",
    ).map_err(|e| e.to_string())?;

    let rows: Vec<ProcessInfo> = stmt
        .query_map([], |r| {
            Ok(ProcessInfo {
                pid: r.get::<_, i64>(0)? as u32,
                name: r.get(1)?,
                cpu_pct: r.get::<_, f64>(2)? as f32,
                ram_mb: r.get::<_, i64>(3)? as u64,
                status: r.get(4)?,
                user_name: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

#[tauri::command]
pub async fn sysmon_kill_process(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::process::Command;
        Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .output()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("kill not supported on this platform".into())
}

#[tauri::command]
pub async fn sysmon_get_disk_breakdown(
    path: String,
) -> Result<Vec<serde_json::Value>, String> {
    use std::process::Command;

    #[cfg(unix)]
    {
        let out = Command::new("du")
            .args(["-sm", "--max-depth=1", &path])
            .output()
            .map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout);
        let mut entries: Vec<(u64, String)> = text
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(2, '\t');
                let size: u64 = parts.next()?.parse().ok()?;
                let dir = parts.next()?.to_string();
                Some((size, dir))
            })
            .collect();
        entries.sort_by(|a, b| b.0.cmp(&a.0));
        entries.truncate(20);
        return Ok(entries
            .iter()
            .map(|(s, d)| serde_json::json!({"path": d, "size_mb": s}))
            .collect());
    }

    #[allow(unreachable_code)]
    Ok(Vec::new())
}

#[tauri::command]
pub async fn sysmon_run_analysis(
    state: State<'_, AppStateHandle>,
    question: Option<String>,
) -> Result<Option<SysmonAnalysisSummary>, String> {
    let cutoff = Utc::now()
        .checked_sub_signed(chrono::Duration::hours(1))
        .unwrap_or_else(Utc::now)
        .to_rfc3339();

    let (snapshots, alerts) = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        let snaps = load_snapshots_since(&conn, &cutoff)?;
        let alrts = load_active_alerts(&conn)?;
        (snaps, alrts)
    };

    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let id = sysmon_analysis::run_analysis(
        &conn,
        "manual",
        None,
        snapshots,
        alerts,
        question.as_deref(),
    )
    .await?;

    let Some(analysis_id) = id else {
        return Ok(None);
    };

    let row: SysmonAnalysisSummary = conn
        .query_row(
            "SELECT id, created_at, trigger, question, response
             FROM sysmon_analyses WHERE id = ?1",
            params![analysis_id],
            |r| {
                Ok(SysmonAnalysisSummary {
                    id: r.get(0)?,
                    created_at: r.get(1)?,
                    trigger: r.get(2)?,
                    question: r.get(3)?,
                    response: r.get(4)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    Ok(Some(row))
}

#[tauri::command]
pub async fn sysmon_list_analyses(
    state: State<'_, AppStateHandle>,
    limit: Option<u32>,
) -> Result<Vec<SysmonAnalysisSummary>, String> {
    let limit = limit.unwrap_or(20).min(100);
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT id, created_at, trigger, question, response
         FROM sysmon_analyses ORDER BY created_at DESC LIMIT ?1",
    ).map_err(|e| e.to_string())?;

    let rows: Vec<SysmonAnalysisSummary> = stmt
        .query_map(params![limit], |r| {
            Ok(SysmonAnalysisSummary {
                id: r.get(0)?,
                created_at: r.get(1)?,
                trigger: r.get(2)?,
                question: r.get(3)?,
                response: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

#[tauri::command]
pub async fn sysmon_get_settings(
    state: State<'_, AppStateHandle>,
) -> Result<SysmonSettings, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let json: Option<String> = conn.query_row(
        "SELECT value FROM config WHERE key = 'sysmon_settings'",
        [],
        |r| r.get(0),
    )
    .ok();

    match json {
        Some(j) => serde_json::from_str(&j).map_err(|e| e.to_string()),
        None => Ok(SysmonSettings::default()),
    }
}

#[tauri::command]
pub async fn sysmon_save_settings(
    state: State<'_, AppStateHandle>,
    settings: SysmonSettings,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let json = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO config (key, value) VALUES ('sysmon_settings', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![json],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

type PooledConn = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

fn load_snapshots_since(conn: &PooledConn, cutoff: &str) -> Result<Vec<SystemSnapshot>, String> {
    let mut stmt = conn.prepare(
        "SELECT cpu_pct, ram_used_mb, ram_total_mb, swap_used_mb, load_avg_1,
                disks_json, gpus_json, net_json
         FROM sysmon_snapshots WHERE sampled_at > ?1 ORDER BY sampled_at ASC",
    ).map_err(|e| e.to_string())?;
    let rows: Vec<SystemSnapshot> = stmt
        .query_map(params![cutoff], |r| {
            Ok(SystemSnapshot {
                cpu_pct: r.get::<_, f64>(0)? as f32,
                ram_used_mb: r.get(1)?,
                ram_total_mb: r.get(2)?,
                swap_used_mb: r.get(3)?,
                load_avg_1: r.get(4)?,
                disks: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(),
                gpus: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(),
                net: serde_json::from_str(&r.get::<_, String>(7)?).unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

fn load_active_alerts(
    conn: &PooledConn,
) -> Result<Vec<(String, String, f64, f64, String)>, String> {
    let mut stmt = conn.prepare(
        "SELECT metric, COALESCE(detail,''), value, threshold, severity
         FROM sysmon_alerts WHERE resolved_at IS NULL ORDER BY fired_at DESC LIMIT 20",
    ).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, f64>(3)?,
                r.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

fn insert_alert(
    conn: &PooledConn,
    metric: &str,
    value: f64,
    threshold: f64,
    severity: &str,
    detail: Option<&str>,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO sysmon_alerts (id, fired_at, metric, value, threshold, severity, detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, now, metric, value, threshold, severity, detail],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

fn load_settings(conn: &PooledConn) -> SysmonSettings {
    conn.query_row(
        "SELECT value FROM config WHERE key = 'sysmon_settings'",
        [],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .and_then(|j| serde_json::from_str(&j).ok())
    .unwrap_or_default()
}

// ── Background poller ─────────────────────────────────────────────────────────

pub fn spawn_sysmon_poller(app: tauri::AppHandle, db: minion_db::Database) {
    tauri::async_runtime::spawn(async move {
        let mut collector = Collector::new();
        let mut tick_count: u64 = 0;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        interval.tick().await; // skip immediate first tick

        loop {
            interval.tick().await;
            tick_count += 1;

            let snapshot = collector.snapshot();

            // Persist snapshot
            let Ok(conn) = db.get() else { continue };
            let id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();
            let disks_json = serde_json::to_string(&snapshot.disks).unwrap_or_default();
            let gpus_json = serde_json::to_string(&snapshot.gpus).unwrap_or_default();
            let net_json = serde_json::to_string(&snapshot.net).unwrap_or_default();

            let _ = conn.execute(
                "INSERT INTO sysmon_snapshots
                 (id, sampled_at, cpu_pct, ram_used_mb, ram_total_mb, swap_used_mb,
                  load_avg_1, disks_json, gpus_json, net_json)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![
                    id,
                    now,
                    snapshot.cpu_pct as f64,
                    snapshot.ram_used_mb as i64,
                    snapshot.ram_total_mb as i64,
                    snapshot.swap_used_mb as i64,
                    snapshot.load_avg_1,
                    disks_json,
                    gpus_json,
                    net_json,
                ],
            );

            // Snapshot processes every 30 s (interval=5s → every 6 ticks)
            if tick_count % 6 == 0 {
                let procs = collector.top_processes();
                for p in &procs {
                    let pid_id = Uuid::new_v4().to_string();
                    let _ = conn.execute(
                        "INSERT INTO sysmon_processes
                         (id, sampled_at, pid, name, cpu_pct, ram_mb, status, user_name)
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                        params![
                            pid_id,
                            now,
                            p.pid as i64,
                            p.name,
                            p.cpu_pct as f64,
                            p.ram_mb as i64,
                            p.status,
                            p.user_name,
                        ],
                    );
                }

                // Prune process samples beyond 200 batches
                let _ = conn.execute(
                    "DELETE FROM sysmon_processes WHERE sampled_at <= (
                        SELECT sampled_at FROM sysmon_processes
                        GROUP BY sampled_at ORDER BY sampled_at DESC
                        LIMIT 1 OFFSET 199
                    )",
                    [],
                );

                // Check for zombies
                let settings = load_settings(&conn);
                for p in &procs {
                    if p.status == "zombie" {
                        if let Ok(alert_id) = insert_alert(
                            &conn,
                            "zombie",
                            1.0,
                            0.0,
                            "warn",
                            Some(&format!("pid {} ({})", p.pid, p.name)),
                        ) {
                            let _ = app.emit(
                                "sysmon-alert",
                                serde_json::json!({
                                    "id": alert_id,
                                    "metric": "zombie",
                                    "value": 1,
                                    "threshold": 0,
                                    "severity": "warn",
                                    "detail": format!("pid {} ({})", p.pid, p.name),
                                }),
                            );
                        }
                    }
                }
                // suppress unused warning for settings loaded before zombie loop
                let _ = settings;
            }

            // Threshold checks
            let settings = load_settings(&conn);
            check_threshold(
                &conn,
                &app,
                "cpu",
                snapshot.cpu_pct as f64,
                settings.cpu_warn as f64,
                settings.cpu_critical as f64,
                None,
            );

            let ram_pct = if snapshot.ram_total_mb > 0 {
                snapshot.ram_used_mb as f64 / snapshot.ram_total_mb as f64 * 100.0
            } else {
                0.0
            };
            check_threshold(
                &conn,
                &app,
                "ram",
                ram_pct,
                settings.ram_warn as f64,
                settings.ram_critical as f64,
                None,
            );

            for disk in &snapshot.disks {
                if disk.total_gb > 0.0 {
                    let pct = disk.used_gb / disk.total_gb * 100.0;
                    check_threshold(
                        &conn,
                        &app,
                        "disk",
                        pct,
                        settings.disk_warn as f64,
                        settings.disk_critical as f64,
                        Some(&disk.mount),
                    );
                }
            }

            for gpu in &snapshot.gpus {
                check_threshold(
                    &conn,
                    &app,
                    "gpu",
                    gpu.util_pct as f64,
                    settings.gpu_warn as f64,
                    settings.gpu_critical as f64,
                    Some(&gpu.name),
                );
            }

            // Emit live snapshot event
            let _ = app.emit("sysmon-snapshot", &snapshot);

            // Prune snapshots older than 30 days (run once per day ≈ every 17280 ticks)
            if tick_count % 17280 == 0 {
                let cutoff = Utc::now()
                    .checked_sub_signed(chrono::Duration::days(30))
                    .unwrap_or_else(Utc::now)
                    .to_rfc3339();
                let _ = conn.execute(
                    "DELETE FROM sysmon_snapshots WHERE sampled_at < ?1",
                    params![cutoff],
                );
            }

            // Auto-analysis: if an alert fired this tick, try LLM (debounced 2 min)
            let recent_alert: Option<String> = conn
                .query_row(
                    "SELECT id FROM sysmon_alerts WHERE fired_at > ?1 LIMIT 1",
                    params![now],
                    |r| r.get(0),
                )
                .ok();

            if recent_alert.is_some() && sysmon_analysis::auto_analysis_eligible(&conn) {
                let five_min_ago = Utc::now()
                    .checked_sub_signed(chrono::Duration::minutes(5))
                    .unwrap_or_else(Utc::now)
                    .to_rfc3339();

                if let Ok(snapshots) = load_snapshots_since(&conn, &five_min_ago) {
                    if let Ok(alerts) = load_active_alerts(&conn) {
                        let alert_id = recent_alert.clone();
                        let db_bg = db.clone();
                        let app_bg = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let Ok(conn2) = db_bg.get() else { return };
                            let result = sysmon_analysis::run_analysis(
                                &conn2,
                                "auto",
                                alert_id.as_deref(),
                                snapshots,
                                alerts,
                                None,
                            )
                            .await;
                            if let Ok(Some(id)) = result {
                                let response: Option<String> = conn2
                                    .query_row(
                                        "SELECT response FROM sysmon_analyses WHERE id = ?1",
                                        params![id],
                                        |r| r.get(0),
                                    )
                                    .ok();
                                if let Some(resp) = response {
                                    let _ = app_bg.emit(
                                        "sysmon-analysis-ready",
                                        serde_json::json!({
                                            "id": id,
                                            "trigger": "auto",
                                            "response": resp,
                                        }),
                                    );
                                }
                            }
                        });
                    }
                }
            }
        }
    });
}

fn check_threshold(
    conn: &PooledConn,
    app: &tauri::AppHandle,
    metric: &str,
    value: f64,
    warn: f64,
    critical: f64,
    detail: Option<&str>,
) {
    let (severity, threshold) = if value >= critical {
        ("critical", critical)
    } else if value >= warn {
        ("warn", warn)
    } else {
        return;
    };

    // Only fire if no unresolved alert for this metric (+detail) in the last 5 min
    let recent_exists: bool = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sysmon_alerts
                WHERE metric = ?1
                AND COALESCE(detail,'') = COALESCE(?2,'')
                AND resolved_at IS NULL
                AND fired_at > datetime('now', '-5 minutes')
            )",
            params![metric, detail],
            |r| r.get(0),
        )
        .unwrap_or(false);

    if recent_exists {
        return;
    }

    if let Ok(alert_id) = insert_alert(conn, metric, value, threshold, severity, detail) {
        let _ = app.emit(
            "sysmon-alert",
            serde_json::json!({
                "id": alert_id,
                "metric": metric,
                "value": value,
                "threshold": threshold,
                "severity": severity,
                "detail": detail,
            }),
        );
    }
}
