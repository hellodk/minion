//! LLM-powered RCA for system events.
//!
//! All public functions return Ok(()) when no LLM endpoint is configured —
//! callers never see an error from a missing endpoint.

use crate::sysmon_collect::SystemSnapshot;
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::warn;
use uuid::Uuid;

type Conn = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct SysmonAnalysis {
    pub id: String,
    pub created_at: String,
    pub trigger: String,
    pub alert_id: Option<String>,
    pub question: Option<String>,
    pub response: String,
}

fn build_prompt(
    snapshots: &[SystemSnapshot],
    alerts: &[(String, String, f64, f64, String)],
    question: Option<&str>,
) -> String {
    let mut lines = vec![
        "## Recent Metrics Summary".to_string(),
        "| Time | CPU% | RAM% | Disk% (max) |".to_string(),
        "|------|------|------|-------------|".to_string(),
    ];
    for (i, s) in snapshots.iter().enumerate() {
        if i % 6 == 0 || i == snapshots.len().saturating_sub(1) {
            let ram_pct = if s.ram_total_mb > 0 {
                (s.ram_used_mb as f64 / s.ram_total_mb as f64 * 100.0) as u64
            } else { 0 };
            let disk_pct = s.disks.iter().map(|d| {
                if d.total_gb > 0.0 { (d.used_gb / d.total_gb * 100.0) as u64 } else { 0 }
            }).max().unwrap_or(0);
            lines.push(format!("| t-{}s | {:.0} | {} | {} |",
                (snapshots.len().saturating_sub(1 + i)) * 5,
                s.cpu_pct, ram_pct, disk_pct));
        }
    }
    if !alerts.is_empty() {
        lines.push("\n## Active Alerts".to_string());
        for (metric, detail, value, threshold, severity) in alerts {
            lines.push(format!("- **{metric}** [{severity}]: {metric} = {value:.1} (threshold {threshold:.1}). {detail}"));
        }
    }
    if let Some(q) = question {
        lines.push(format!("\n## User Question\n{}", q));
    }
    lines.join("\n")
}

fn get_endpoint(conn: &Conn) -> Option<(String, Option<String>, String)> {
    conn.query_row(
        "SELECT base_url, api_key_encrypted, COALESCE(default_model, 'llama3') FROM llm_endpoints LIMIT 1",
        [],
        |r| Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, String>(2)?,
        )),
    ).ok()
}

async fn call_llm(
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    user_content: &str,
) -> Option<String> {
    let system = "You are a system reliability expert. Analyse the metrics below and provide a \
                  concise root cause analysis. Focus on correlations between CPU, RAM, disk I/O, \
                  and process events. Be specific: name the likely cause, its effect, and one \
                  actionable fix if warranted. If no issue is present, say so briefly.";

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user_content}
        ],
        "stream": false
    });

    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .ok()?;

    let mut req = client.post(&url).json(&body);
    if let Some(key) = api_key {
        if !key.is_empty() {
            req = req.bearer_auth(key);
        }
    }

    let resp = req.send().await.map_err(|e| warn!("LLM call failed: {e}")).ok()?;
    if !resp.status().is_success() {
        warn!("LLM returned {}", resp.status());
        return None;
    }

    let json: serde_json::Value = resp.json().await.ok()?;
    json["choices"][0]["message"]["content"].as_str().map(|s| s.to_string())
}

pub async fn run_analysis(
    db: &minion_db::Database,
    trigger: &str,
    alert_id: Option<&str>,
    snapshots: Vec<SystemSnapshot>,
    alerts: Vec<(String, String, f64, f64, String)>,
    question: Option<&str>,
) -> Result<Option<String>, String> {
    // Acquire a connection only for the sync endpoint lookup, then drop it
    // before the async LLM call so the future remains Send.
    let (base_url, api_key, model) = {
        let conn = db.get().map_err(|e| e.to_string())?;
        match get_endpoint(&conn) {
            Some(ep) => ep,
            None => return Ok(None),
        }
    };

    let user_content = build_prompt(&snapshots, &alerts, question);
    let context_json = serde_json::to_string(&snapshots).unwrap_or_default();

    // conn is dropped — this await is now Send-safe.
    let Some(response) = call_llm(&base_url, api_key.as_deref(), &model, &user_content).await else {
        return Ok(None);
    };

    // Re-acquire connection for the INSERT.
    let conn = db.get().map_err(|e| e.to_string())?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO sysmon_analyses (id, created_at, trigger, alert_id, question, context_json, response)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, now, trigger, alert_id, question, context_json, response],
    ).map_err(|e: rusqlite::Error| e.to_string())?;

    Ok(Some(id))
}

pub fn auto_analysis_eligible(conn: &Conn) -> bool {
    let last: Option<String> = conn.query_row(
        "SELECT created_at FROM sysmon_analyses WHERE trigger='auto' ORDER BY created_at DESC LIMIT 1",
        [],
        |r| r.get::<_, String>(0),
    ).ok();

    let Some(last_str) = last else { return true };
    let Ok(last_dt) = chrono::DateTime::parse_from_rfc3339(&last_str) else { return true };
    Utc::now().signed_duration_since(last_dt).num_seconds() > 120
}
