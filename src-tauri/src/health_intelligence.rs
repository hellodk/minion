//! Health Intelligence Phase C — timeline, anomaly detection, AI narrative.

use crate::state::AppState;
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;
use uuid::Uuid;

type AppStateHandle = Arc<RwLock<AppState>>;
type Conn = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

// ── Serde types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEventRow {
    pub id: String,
    pub patient_id: String,
    pub event_date: String,
    pub category: String,
    pub title: String,
    pub description: Option<String>,
    pub source_type: String,
    pub source_id: Option<String>,
    pub severity: Option<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyAlert {
    pub id: String,
    pub rule_name: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub detected_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceReport {
    pub id: String,
    pub patient_id: String,
    pub generated_at: String,
    pub model_used: String,
    pub report_text: String,
    pub anomalies_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationVisit {
    pub id: String,
    pub patient_id: String,
    pub visit_date: String,
    pub city: String,
    pub country: Option<String>,
    pub source: String,
    pub notes: Option<String>,
}

// ── LLM helpers (same pattern as health_extract.rs) ──────────────────────────

fn get_endpoint(conn: &Conn) -> Option<(String, Option<String>, String)> {
    conn.query_row(
        "SELECT base_url, api_key_encrypted, COALESCE(default_model, 'llama3')
         FROM llm_endpoints LIMIT 1",
        [],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?)),
    )
    .ok()
}

async fn call_llm(
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    system: &str,
    user: &str,
) -> Option<String> {
    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user",   "content": user}
        ],
        "stream": false
    });
    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .ok()?;
    let mut req = client.post(&url).json(&body);
    if let Some(key) = api_key {
        if !key.is_empty() {
            req = req.bearer_auth(key);
        }
    }
    let resp = req
        .send()
        .await
        .map_err(|e| tracing::warn!("LLM call failed: {e}"))
        .ok()?;
    if !resp.status().is_success() {
        tracing::warn!("LLM returned {}", resp.status());
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    json["choices"][0]["message"]["content"].as_str().map(|s| s.to_string())
}

// ── Anomaly ID ────────────────────────────────────────────────────────────────

fn anomaly_id(patient_id: &str, rule_name: &str, date: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    format!("{patient_id}:{rule_name}:{date}").hash(&mut h);
    format!("{:016x}", h.finish())
}

// ── Timeline builder ──────────────────────────────────────────────────────────

pub fn build_timeline_events(
    conn: &rusqlite::Connection,
    patient_id: &str,
) -> Result<usize, String> {
    conn.execute(
        "DELETE FROM health_timeline_events WHERE patient_id = ?1",
        params![patient_id],
    )
    .map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();
    let mut total: usize = 0;

    // 1. Lab results
    {
        let mut stmt = conn
            .prepare(
                "SELECT slr.id, slr.report_date, slr.lab_name, slr.location_city,
                    SUM(CASE WHEN slv.flag IN ('CRITICAL','HIGH') THEN 1 ELSE 0 END),
                    SUM(CASE WHEN slv.flag = 'LOW' THEN 1 ELSE 0 END)
             FROM structured_lab_results slr
             LEFT JOIN structured_lab_values slv ON slv.result_id = slr.id
             WHERE slr.patient_id = ?1
             GROUP BY slr.id ORDER BY slr.report_date DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<_> = stmt
            .query_map(params![patient_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        for (src_id, report_date, lab_name, city, crit, low_count) in rows {
            let severity =
                if crit > 0 { "alert" } else if low_count > 0 { "warning" } else { "info" };
            let title = lab_name.unwrap_or_else(|| "Lab Report".to_string());
            let description = city.map(|c| format!("at {c}"));
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO health_timeline_events
                 (id, patient_id, event_date, category, title, description,
                  source_type, source_id, severity, created_at)
                 VALUES (?1,?2,?3,'lab',?4,?5,'structured_lab_result',?6,?7,?8)",
                params![id, patient_id, report_date, title, description, src_id, severity, now],
            )
            .map_err(|e| e.to_string())?;
            total += 1;
        }
    }

    // 2. Prescriptions
    {
        let mut stmt = conn
            .prepare(
                "SELECT p.id, p.prescribed_date, p.prescriber_name, p.diagnosis_text,
                    COUNT(pi.id)
             FROM prescriptions p
             LEFT JOIN prescription_items pi ON pi.prescription_id = p.id
             WHERE p.patient_id = ?1
             GROUP BY p.id ORDER BY p.prescribed_date DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<_> = stmt
            .query_map(params![patient_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        for (src_id, prescribed_date, prescriber, diagnosis, item_count) in rows {
            let title = prescriber
                .map(|p| format!("Prescription by Dr. {p}"))
                .unwrap_or_else(|| "Prescription".to_string());
            let description = diagnosis
                .map(|d| format!("{d} ({item_count} items)"))
                .or_else(|| Some(format!("{item_count} items")));
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO health_timeline_events
                 (id, patient_id, event_date, category, title, description,
                  source_type, source_id, severity, created_at)
                 VALUES (?1,?2,?3,'prescription',?4,?5,'prescription',?6,'info',?7)",
                params![id, patient_id, prescribed_date, title, description, src_id, now],
            )
            .map_err(|e| e.to_string())?;
            total += 1;
        }
    }

    // 3. Fitness — weekly aggregates
    {
        let mut stmt = conn
            .prepare(
                "SELECT strftime('%Y-W%W', date) AS week, MIN(date),
                    AVG(steps), AVG(sleep_hours), AVG(heart_rate_avg), AVG(weight_kg)
             FROM fitness_metrics WHERE date IS NOT NULL
             GROUP BY week ORDER BY week DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<_> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<f64>>(2)?,
                    r.get::<_, Option<f64>>(3)?,
                    r.get::<_, Option<f64>>(4)?,
                    r.get::<_, Option<f64>>(5)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        for (week_start, avg_steps, avg_sleep, avg_hr, avg_weight) in rows {
            let low_steps = avg_steps.map(|s| s < 3000.0).unwrap_or(false);
            let low_sleep = avg_sleep.map(|s| s < 5.0).unwrap_or(false);
            let severity = if low_steps || low_sleep { "warning" } else { "info" };
            let mut parts = Vec::new();
            if let Some(s) = avg_steps {
                parts.push(format!("{:.0} steps/day", s));
            }
            if let Some(s) = avg_sleep {
                parts.push(format!("{:.1}h sleep", s));
            }
            if let Some(h) = avg_hr {
                parts.push(format!("HR {:.0}", h));
            }
            if let Some(w) = avg_weight {
                parts.push(format!("{:.1}kg", w));
            }
            let description = if parts.is_empty() { None } else { Some(parts.join(", ")) };
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO health_timeline_events
                 (id, patient_id, event_date, category, title, description,
                  source_type, source_id, severity, created_at)
                 VALUES (?1,?2,?3,'fitness','Weekly Fitness Summary',?4,'fitness_metrics',NULL,?5,?6)",
                params![id, patient_id, week_start, description, severity, now],
            )
            .map_err(|e| e.to_string())?;
            total += 1;
        }
    }

    // 4. Symptoms — columns: description, first_noticed, severity (INTEGER)
    {
        let mut stmt = conn
            .prepare(
                "SELECT id, description, first_noticed, severity
             FROM symptoms WHERE patient_id = ?1 ORDER BY first_noticed DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<_> = stmt
            .query_map(params![patient_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        for (src_id, description, first_noticed, severity_int) in rows {
            let date =
                first_noticed.unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
            let title: String =
                description.unwrap_or_else(|| "Symptom".to_string()).chars().take(60).collect();
            // Map integer severity (1-10) to text label
            let severity = match severity_int {
                Some(s) if s >= 8 => "alert",
                Some(s) if s >= 5 => "warning",
                _ => "info",
            };
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO health_timeline_events
                 (id, patient_id, event_date, category, title,
                  source_type, source_id, severity, created_at)
                 VALUES (?1,?2,?3,'symptom',?4,'symptom',?5,?6,?7)",
                params![id, patient_id, date, title, src_id, severity, now],
            )
            .map_err(|e| e.to_string())?;
            total += 1;
        }
    }

    // 5. Vitals — EAV schema: measurement_type / value / unit / measured_at
    //    Group readings by date and build a summary description per day.
    {
        let mut stmt = conn
            .prepare(
                "SELECT id, measured_at, measurement_type, value, unit
             FROM vitals WHERE patient_id = ?1 ORDER BY measured_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<_> = stmt
            .query_map(params![patient_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, f64>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        // Collect all readings; emit one timeline event per individual vital entry
        for (src_id, measured_at, mtype, value, unit) in rows {
            let bp_alert = (mtype == "systolic_bp" && value > 140.0)
                || (mtype == "diastolic_bp" && value > 90.0);
            let spo2_alert = mtype == "spo2" && value < 94.0;
            let severity = if bp_alert || spo2_alert { "alert" } else { "info" };
            let unit_str = unit.as_deref().unwrap_or("");
            let description = Some(format!("{mtype}: {value:.1}{unit_str}"));
            let event_date = measured_at.get(..10).unwrap_or(&measured_at).to_string();
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO health_timeline_events
                 (id, patient_id, event_date, category, title, description,
                  source_type, source_id, severity, created_at)
                 VALUES (?1,?2,?3,'vital','Vitals Reading',?4,'vital',?5,?6,?7)",
                params![id, patient_id, event_date, description, src_id, severity, now],
            )
            .map_err(|e| e.to_string())?;
            total += 1;
        }
    }

    // 6. Location visits
    {
        let mut stmt = conn
            .prepare(
                "SELECT id, visit_date, city, country FROM location_visits
             WHERE patient_id = ?1 ORDER BY visit_date DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<_> = stmt
            .query_map(params![patient_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        for (src_id, visit_date, city, country) in rows {
            let title = format!("Visit: {city}");
            let description = country.map(|c| format!("{city}, {c}"));
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO health_timeline_events
                 (id, patient_id, event_date, category, title, description,
                  source_type, source_id, severity, created_at)
                 VALUES (?1,?2,?3,'location',?4,?5,'location_visit',?6,'info',?7)",
                params![id, patient_id, visit_date, title, description, src_id, now],
            )
            .map_err(|e| e.to_string())?;
            total += 1;
        }
    }

    Ok(total)
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn health_rebuild_timeline(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<usize, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    build_timeline_events(&conn, &patient_id)
}

#[tauri::command]
pub async fn health_get_timeline(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    limit: i64,
    offset: i64,
    category_filter: Option<String>,
) -> Result<Vec<TimelineEventRow>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    fn map_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TimelineEventRow> {
        Ok(TimelineEventRow {
            id: r.get(0)?,
            patient_id: r.get(1)?,
            event_date: r.get(2)?,
            category: r.get(3)?,
            title: r.get(4)?,
            description: r.get(5)?,
            source_type: r.get(6)?,
            source_id: r.get(7)?,
            severity: r.get(8)?,
            metadata_json: r.get(9)?,
        })
    }

    let rows: Vec<TimelineEventRow> = if let Some(cat) = &category_filter {
        let mut stmt = conn
            .prepare(
                "SELECT id, patient_id, event_date, category, title, description,
                    source_type, source_id, severity, metadata_json
             FROM health_timeline_events
             WHERE patient_id = ?1 AND category = ?2
             ORDER BY event_date DESC LIMIT ?3 OFFSET ?4",
            )
            .map_err(|e| e.to_string())?;
        let collected: Vec<TimelineEventRow> = stmt
            .query_map(params![patient_id, cat, limit, offset], map_row)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        collected
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, patient_id, event_date, category, title, description,
                    source_type, source_id, severity, metadata_json
             FROM health_timeline_events
             WHERE patient_id = ?1
             ORDER BY event_date DESC LIMIT ?2 OFFSET ?3",
            )
            .map_err(|e| e.to_string())?;
        let collected: Vec<TimelineEventRow> = stmt
            .query_map(params![patient_id, limit, offset], map_row)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        collected
    };
    Ok(rows)
}

#[tauri::command]
pub async fn health_detect_anomalies(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<AnomalyAlert>, String> {
    let db = { state.read().await.db.clone() };
    let mut alerts: Vec<AnomalyAlert> = Vec::new();
    let today = Utc::now().format("%Y-%m-%d").to_string();

    // Rule 1: HbA1c rising — last 3 HbA1c values all > 7.0
    {
        let conn = db.get().map_err(|e| e.to_string())?;
        let vals: Vec<f64> = conn
            .prepare(
                "SELECT v.value_numeric FROM structured_lab_values v
             JOIN structured_lab_results r ON r.id = v.result_id
             WHERE r.patient_id = ?1 AND LOWER(v.test_name) LIKE '%hba1c%'
               AND v.value_numeric IS NOT NULL
             ORDER BY r.report_date DESC LIMIT 3",
            )
            .and_then(|mut s| {
                s.query_map(params![patient_id], |r| r.get::<_, f64>(0))
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default();
        if vals.len() >= 2 && vals.iter().all(|&v| v > 7.0) {
            let id = anomaly_id(&patient_id, "hba1c_rising", &today);
            alerts.push(AnomalyAlert {
                id,
                rule_name: "hba1c_rising".to_string(),
                severity: "alert".to_string(),
                title: "HbA1c elevated across multiple tests".to_string(),
                description: format!(
                    "Last {} HbA1c readings all above 7.0% — review glycaemic control.",
                    vals.len()
                ),
                detected_at: today.clone(),
            });
        }
    }

    // Rule 2: Sleep deficit — last 7 days avg sleep < 6h
    {
        let conn = db.get().map_err(|e| e.to_string())?;
        let avg: Option<f64> = conn
            .query_row(
                "SELECT AVG(sleep_hours) FROM fitness_metrics
             WHERE date >= date('now', '-7 days') AND sleep_hours IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        if avg.map(|a| a < 6.0).unwrap_or(false) {
            let id = anomaly_id(&patient_id, "sleep_deficit", &today);
            alerts.push(AnomalyAlert {
                id,
                rule_name: "sleep_deficit".to_string(),
                severity: "warning".to_string(),
                title: "Sleep below 6h average this week".to_string(),
                description: format!(
                    "7-day average sleep: {:.1}h — below the 6h threshold.",
                    avg.unwrap_or(0.0)
                ),
                detected_at: today.clone(),
            });
        }
    }

    // Rule 3: Critical lab value in last 90 days
    {
        let conn = db.get().map_err(|e| e.to_string())?;
        let critical_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM structured_lab_values v
             JOIN structured_lab_results r ON r.id = v.result_id
             WHERE r.patient_id = ?1 AND v.flag = 'CRITICAL'
               AND r.report_date >= date('now', '-90 days')",
                params![patient_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if critical_count > 0 {
            let id = anomaly_id(&patient_id, "lab_critical", &today);
            alerts.push(AnomalyAlert {
                id,
                rule_name: "lab_critical".to_string(),
                severity: "alert".to_string(),
                title: format!("{} critical lab value(s) in last 90 days", critical_count),
                description: "One or more lab tests are critically outside reference range. \
                               Review with your doctor."
                    .to_string(),
                detected_at: today.clone(),
            });
        }
    }

    // Rule 4: BP elevated (systolic_bp > 140) in last 30 days — EAV schema
    {
        let conn = db.get().map_err(|e| e.to_string())?;
        let bp_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM vitals
             WHERE patient_id = ?1 AND measurement_type = 'systolic_bp' AND value > 140
               AND measured_at >= datetime('now', '-30 days')",
                params![patient_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if bp_count > 0 {
            let id = anomaly_id(&patient_id, "bp_elevated", &today);
            alerts.push(AnomalyAlert {
                id,
                rule_name: "bp_elevated".to_string(),
                severity: "warning".to_string(),
                title: format!("{} high BP reading(s) in last 30 days", bp_count),
                description: "Systolic BP above 140 mmHg detected. \
                               Monitor closely and consult your doctor."
                    .to_string(),
                detected_at: today.clone(),
            });
        }
    }

    // Rule 5: Vitamin supplement gap — prescription > 12 weeks ago with no follow-up lab
    {
        let conn = db.get().map_err(|e| e.to_string())?;
        let gap_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM prescription_items pi
             JOIN prescriptions p ON p.id = pi.prescription_id
             WHERE p.patient_id = ?1
               AND LOWER(pi.drug_name) LIKE '%vitamin%'
               AND p.prescribed_date < date('now', '-84 days')",
                params![patient_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if gap_count > 0 {
            let id = anomaly_id(&patient_id, "vitamin_gap", &today);
            alerts.push(AnomalyAlert {
                id,
                rule_name: "vitamin_gap".to_string(),
                severity: "warning".to_string(),
                title: "Vitamin supplement prescription over 12 weeks old".to_string(),
                description: "Consider getting follow-up lab tests to check supplement levels."
                    .to_string(),
                detected_at: today.clone(),
            });
        }
    }

    Ok(alerts)
}

#[tauri::command]
pub async fn health_generate_report(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    consent_confirmed: bool,
) -> Result<IntelligenceReport, String> {
    if !consent_confirmed {
        return Err("User consent required to generate a health report.".to_string());
    }
    let db = { state.read().await.db.clone() };

    // Get last 180 days of timeline events
    let events: Vec<TimelineEventRow> = {
        let conn = db.get().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, patient_id, event_date, category, title, description,
                    source_type, source_id, severity, metadata_json
             FROM health_timeline_events
             WHERE patient_id = ?1 AND event_date >= date('now', '-180 days')
             ORDER BY event_date DESC LIMIT 100",
            )
            .map_err(|e| e.to_string())?;
        let collected: Vec<TimelineEventRow> = stmt
            .query_map(params![patient_id], |r| {
                Ok(TimelineEventRow {
                    id: r.get(0)?,
                    patient_id: r.get(1)?,
                    event_date: r.get(2)?,
                    category: r.get(3)?,
                    title: r.get(4)?,
                    description: r.get(5)?,
                    source_type: r.get(6)?,
                    source_id: r.get(7)?,
                    severity: r.get(8)?,
                    metadata_json: r.get(9)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        collected
    };

    // Get LLM endpoint — drop conn before async
    let (base_url, api_key, model) = {
        let conn = db.get().map_err(|e| e.to_string())?;
        get_endpoint(&conn)
            .ok_or("No AI endpoint configured. Add one in Settings → AI Endpoints.")?
    };

    // Build summary prompt
    let mut summary = String::new();
    for ev in &events {
        summary.push_str(&format!(
            "[{}] {} — {}",
            ev.event_date,
            ev.category.to_uppercase(),
            ev.title
        ));
        if let Some(desc) = &ev.description {
            summary.push_str(&format!(": {desc}"));
        }
        summary.push('\n');
    }
    let summary = &summary[..summary.len().min(8_000)];

    let system = "You are a medical assistant helping a patient prepare for a doctor visit. \
                  Write a clear, factual 300-500 word health summary suitable to share with a GP. \
                  Focus on trends, anomalies, and what has changed. Do not diagnose. Be concise.";
    let user = format!("Health timeline for the last 6 months:\n\n{summary}");

    let report_text = call_llm(&base_url, api_key.as_deref(), &model, system, &user)
        .await
        .ok_or("AI did not return a response. Check your AI endpoint in Settings.")?;

    let report_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    {
        let conn = db.get().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO health_intelligence_reports
             (id, patient_id, generated_at, model_used, report_text, created_at)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![report_id, patient_id, now, model, report_text, now],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(IntelligenceReport {
        id: report_id,
        patient_id,
        generated_at: now,
        model_used: model,
        report_text,
        anomalies_json: None,
    })
}

#[tauri::command]
pub async fn health_list_reports(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<IntelligenceReport>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, patient_id, generated_at, model_used, report_text, anomalies_json
         FROM health_intelligence_reports
         WHERE patient_id = ?1 ORDER BY generated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<IntelligenceReport> = stmt
        .query_map(params![patient_id], |r| {
            Ok(IntelligenceReport {
                id: r.get(0)?,
                patient_id: r.get(1)?,
                generated_at: r.get(2)?,
                model_used: r.get(3)?,
                report_text: r.get(4)?,
                anomalies_json: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[tauri::command]
pub async fn health_delete_report(
    state: State<'_, AppStateHandle>,
    id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM health_intelligence_reports WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn health_add_location_visit(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    visit_date: String,
    city: String,
    country: Option<String>,
    source: String,
) -> Result<String, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO location_visits (id, patient_id, visit_date, city, country, source, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![id, patient_id, visit_date, city, country, source, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub async fn health_list_location_visits(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<LocationVisit>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, patient_id, visit_date, city, country, source, notes
         FROM location_visits WHERE patient_id = ?1 ORDER BY visit_date DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<LocationVisit> = stmt
        .query_map(params![patient_id], |r| {
            Ok(LocationVisit {
                id: r.get(0)?,
                patient_id: r.get(1)?,
                visit_date: r.get(2)?,
                city: r.get(3)?,
                country: r.get(4)?,
                source: r.get(5)?,
                notes: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}
