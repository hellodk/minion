//! Health Vault week 4: timeline aggregation, episode auto-linking,
//! symptom LLM classification, and temporal correlation graph.
//!
//! The "timeline" is a unified read-side view across all per-patient event
//! tables (medical_records, lab_tests, medications_v2, health_conditions,
//! vitals, life_events, symptoms). It is computed on demand rather than
//! materialized so the data is always live.
//!
//! Episodes group nearby events. Auto-linking uses simple temporal
//! clustering (a user-tunable gap window) so that, for example, a
//! consultation, the lab work it ordered, and the prescription that came
//! out of it all live under one named episode.
//!
//! Correlations connect a "source" event (typically a symptom, life event,
//! or condition) to other events that fall inside a window, with a
//! confidence score that decays linearly with delta-days. Results are
//! cached in `health_correlations` so the UI graph renders instantly.

use crate::health_classify::{classify_endpoint_for_feature, EndpointHandle};
use crate::state::AppState;
use chrono::{Duration, NaiveDate, Utc};
use minion_llm::{create_provider, JsonExtractRequest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<AppState>>;

// =====================================================================
// Timeline aggregation
// =====================================================================

/// One row in the unified timeline. `kind` identifies the source table;
/// `layer` buckets it into the three visualization rows (events / symptoms
/// / labs).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TimelineEvent {
    pub id: String,
    pub kind: String,
    pub layer: String,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub date: String,
    pub end_date: Option<String>,
    pub value: Option<f64>,
    pub unit: Option<String>,
    pub flag: Option<String>,
    pub episode_id: Option<String>,
}

/// Read every per-patient event table and merge into a single sorted list.
/// `from`/`to` are optional ISO-8601 dates (`YYYY-MM-DD`).
#[tauri::command]
pub async fn health_timeline_get(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    from: Option<String>,
    to: Option<String>,
) -> Result<Vec<TimelineEvent>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut events: Vec<TimelineEvent> = Vec::new();

    // Validate date inputs once. Any non-`YYYY-MM-DD` value is dropped
    // rather than passed through to SQL — this is the boundary where we
    // refuse to interpolate user input. (Inline interpolation is only
    // safe AFTER validation.)
    fn validate_iso_date(s: &str) -> Option<String> {
        if s.len() == 10 && chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok() {
            Some(s.to_string())
        } else if s.len() >= 10 && chrono::NaiveDate::parse_from_str(&s[..10], "%Y-%m-%d").is_ok() {
            Some(s[..10].to_string())
        } else {
            None
        }
    }
    let from = from.as_deref().and_then(validate_iso_date);
    let to = to.as_deref().and_then(validate_iso_date);

    let date_filter = |col: &str| -> String {
        let mut s = String::new();
        if let Some(d) = &from {
            s.push_str(&format!(" AND {} >= '{}'", col, d));
        }
        if let Some(d) = &to {
            s.push_str(&format!(" AND {} <= '{}'", col, d));
        }
        s
    };

    // Medical records — events layer.
    {
        let sql = format!(
            "SELECT id, record_type, title, description, date, episode_id
             FROM medical_records
             WHERE patient_id = ?1 {}
             ORDER BY date DESC",
            date_filter("date")
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([&patient_id], |row| {
                let date: String = row.get(4)?;
                Ok(TimelineEvent {
                    id: row.get(0)?,
                    kind: "medical_record".into(),
                    layer: "events".into(),
                    title: row.get(2)?,
                    description: row.get(3)?,
                    category: row.get(1)?,
                    date,
                    end_date: None,
                    value: None,
                    unit: None,
                    flag: None,
                    episode_id: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            events.push(r.map_err(|e| e.to_string())?);
        }
    }

    // Lab tests — labs layer.
    {
        let sql = format!(
            "SELECT id, test_name, canonical_name, test_category, value, unit,
                    flag, collected_at, episode_id
             FROM lab_tests
             WHERE patient_id = ?1 {}
             ORDER BY collected_at DESC",
            date_filter("collected_at")
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([&patient_id], |row| {
                let canonical: Option<String> = row.get(2)?;
                let raw: String = row.get(1)?;
                Ok(TimelineEvent {
                    id: row.get(0)?,
                    kind: "lab_test".into(),
                    layer: "labs".into(),
                    title: canonical.unwrap_or(raw),
                    description: None,
                    category: row.get(3)?,
                    date: row.get(7)?,
                    end_date: None,
                    value: row.get(4)?,
                    unit: row.get(5)?,
                    flag: row.get(6)?,
                    episode_id: row.get(8)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            events.push(r.map_err(|e| e.to_string())?);
        }
    }

    // Medications — events layer (start date as the event).
    {
        let sql = format!(
            "SELECT id, name, generic_name, dose, frequency, start_date, end_date, episode_id
             FROM medications_v2
             WHERE patient_id = ?1 AND start_date IS NOT NULL {}
             ORDER BY start_date DESC",
            date_filter("start_date")
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([&patient_id], |row| {
                let dose: Option<String> = row.get(3)?;
                let freq: Option<String> = row.get(4)?;
                let mut desc = String::new();
                if let Some(d) = &dose {
                    desc.push_str(d);
                }
                if let Some(f) = &freq {
                    if !desc.is_empty() {
                        desc.push_str(" · ");
                    }
                    desc.push_str(f);
                }
                Ok(TimelineEvent {
                    id: row.get(0)?,
                    kind: "medication".into(),
                    layer: "events".into(),
                    title: row.get(1)?,
                    description: if desc.is_empty() { None } else { Some(desc) },
                    category: row.get(2)?,
                    date: row.get(5)?,
                    end_date: row.get(6)?,
                    value: None,
                    unit: None,
                    flag: None,
                    episode_id: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            events.push(r.map_err(|e| e.to_string())?);
        }
    }

    // Conditions — events layer.
    {
        let sql = format!(
            "SELECT id, name, condition_type, severity, diagnosed_at, resolved_at, episode_id
             FROM health_conditions
             WHERE patient_id = ?1 AND diagnosed_at IS NOT NULL {}
             ORDER BY diagnosed_at DESC",
            date_filter("diagnosed_at")
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([&patient_id], |row| {
                Ok(TimelineEvent {
                    id: row.get(0)?,
                    kind: "condition".into(),
                    layer: "events".into(),
                    title: row.get(1)?,
                    description: row.get(3)?,
                    category: row.get(2)?,
                    date: row.get(4)?,
                    end_date: row.get(5)?,
                    value: None,
                    unit: None,
                    flag: None,
                    episode_id: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            events.push(r.map_err(|e| e.to_string())?);
        }
    }

    // Vitals — labs layer (numeric series).
    {
        let sql = format!(
            "SELECT id, measurement_type, value, unit, measured_at, episode_id
             FROM vitals
             WHERE patient_id = ?1 {}
             ORDER BY measured_at DESC",
            date_filter("measured_at")
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([&patient_id], |row| {
                let mtype: String = row.get(1)?;
                Ok(TimelineEvent {
                    id: row.get(0)?,
                    kind: "vital".into(),
                    layer: "labs".into(),
                    title: mtype.clone(),
                    description: None,
                    category: Some(mtype),
                    date: row.get(4)?,
                    end_date: None,
                    value: row.get(2)?,
                    unit: row.get(3)?,
                    flag: None,
                    episode_id: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            events.push(r.map_err(|e| e.to_string())?);
        }
    }

    // Life events — events layer.
    {
        let sql = format!(
            "SELECT id, category, subcategory, title, description, intensity,
                    started_at, ended_at, episode_id
             FROM life_events
             WHERE patient_id = ?1 {}
             ORDER BY started_at DESC",
            date_filter("started_at")
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([&patient_id], |row| {
                Ok(TimelineEvent {
                    id: row.get(0)?,
                    kind: "life_event".into(),
                    layer: "events".into(),
                    title: row.get(3)?,
                    description: row.get(4)?,
                    category: row.get(1)?,
                    date: row.get(6)?,
                    end_date: row.get(7)?,
                    value: row.get::<_, Option<i64>>(5)?.map(|n| n as f64),
                    unit: None,
                    flag: row.get(2)?, // store subcategory in flag for chip
                    episode_id: row.get(8)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            events.push(r.map_err(|e| e.to_string())?);
        }
    }

    // Symptoms — symptoms layer.
    {
        let sql = format!(
            "SELECT id, description, canonical_name, body_part, severity,
                    first_noticed, resolved_at, episode_id
             FROM symptoms
             WHERE patient_id = ?1 {}
             ORDER BY first_noticed DESC",
            date_filter("first_noticed")
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([&patient_id], |row| {
                let canonical: Option<String> = row.get(2)?;
                let desc: String = row.get(1)?;
                Ok(TimelineEvent {
                    id: row.get(0)?,
                    kind: "symptom".into(),
                    layer: "symptoms".into(),
                    title: canonical.unwrap_or_else(|| desc.chars().take(40).collect::<String>()),
                    description: Some(desc),
                    category: row.get(3)?,
                    date: row.get(5)?,
                    end_date: row.get(6)?,
                    value: row.get::<_, Option<i64>>(4)?.map(|n| n as f64),
                    unit: None,
                    flag: None,
                    episode_id: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            events.push(r.map_err(|e| e.to_string())?);
        }
    }

    // Sort newest first overall.
    events.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(events)
}

// =====================================================================
// Episodes
// =====================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Episode {
    pub id: String,
    pub patient_id: String,
    pub name: String,
    pub description: Option<String>,
    pub start_date: String,
    pub end_date: Option<String>,
    pub primary_condition: Option<String>,
    pub ai_generated: bool,
    pub user_confirmed: bool,
    pub created_at: String,
    pub event_count: i64,
}

#[tauri::command]
pub async fn health_episode_list(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<Episode>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, patient_id, name, description, start_date, end_date,
                    primary_condition, ai_generated, user_confirmed, created_at
             FROM episodes WHERE patient_id = ?1 ORDER BY start_date DESC",
        )
        .map_err(|e| e.to_string())?;
    let raw: Vec<Episode> = stmt
        .query_map([&patient_id], |row| {
            Ok(Episode {
                id: row.get(0)?,
                patient_id: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                start_date: row.get(4)?,
                end_date: row.get(5)?,
                primary_condition: row.get(6)?,
                ai_generated: row.get::<_, i64>(7)? != 0,
                user_confirmed: row.get::<_, i64>(8)? != 0,
                created_at: row.get(9)?,
                event_count: 0,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    // Counts across the six per-event tables.
    let mut out = Vec::with_capacity(raw.len());
    for mut ep in raw {
        ep.event_count = count_episode_events(&conn, &ep.id);
        out.push(ep);
    }
    Ok(out)
}

fn count_episode_events(conn: &rusqlite::Connection, episode_id: &str) -> i64 {
    let tables = [
        "medical_records",
        "lab_tests",
        "medications_v2",
        "health_conditions",
        "vitals",
        "life_events",
        "symptoms",
    ];
    let mut total: i64 = 0;
    for t in tables {
        let sql = format!("SELECT COUNT(*) FROM {} WHERE episode_id = ?1", t);
        if let Ok(n) = conn.query_row(&sql, [episode_id], |r| r.get::<_, i64>(0)) {
            total += n;
        }
    }
    total
}

#[tauri::command]
pub async fn health_episode_create(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    name: String,
    description: Option<String>,
    start_date: String,
    end_date: Option<String>,
    primary_condition: Option<String>,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO episodes (id, patient_id, name, description, start_date,
         end_date, primary_condition, ai_generated, user_confirmed)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 1)",
        rusqlite::params![
            id,
            patient_id,
            name,
            description,
            start_date,
            end_date,
            primary_condition,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

/// Single field-update payload. The frontend sends only the fields that
/// were actually edited; nullable fields use `Some(None)` (after JSON
/// round-trip via `serde_json::Value`) to mean "clear", `None` to mean
/// "leave alone". To keep the wire format simple we use a single struct
/// instead of a wide function signature.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct EpisodeUpdate {
    pub name: Option<String>,
    pub description: Option<serde_json::Value>,
    pub end_date: Option<serde_json::Value>,
    pub primary_condition: Option<serde_json::Value>,
    pub user_confirmed: Option<bool>,
}

fn json_to_opt_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
        serde_json::Value::String(_) => None,
        other => Some(other.to_string()),
    }
}

#[tauri::command]
pub async fn health_episode_update(
    state: State<'_, AppStateHandle>,
    id: String,
    update: EpisodeUpdate,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    if let Some(n) = update.name {
        conn.execute(
            "UPDATE episodes SET name = ?1 WHERE id = ?2",
            rusqlite::params![n, id],
        )
        .map_err(|e| e.to_string())?;
    }
    if let Some(v) = update.description {
        conn.execute(
            "UPDATE episodes SET description = ?1 WHERE id = ?2",
            rusqlite::params![json_to_opt_string(&v), id],
        )
        .map_err(|e| e.to_string())?;
    }
    if let Some(v) = update.end_date {
        conn.execute(
            "UPDATE episodes SET end_date = ?1 WHERE id = ?2",
            rusqlite::params![json_to_opt_string(&v), id],
        )
        .map_err(|e| e.to_string())?;
    }
    if let Some(v) = update.primary_condition {
        conn.execute(
            "UPDATE episodes SET primary_condition = ?1 WHERE id = ?2",
            rusqlite::params![json_to_opt_string(&v), id],
        )
        .map_err(|e| e.to_string())?;
    }
    if let Some(uc) = update.user_confirmed {
        conn.execute(
            "UPDATE episodes SET user_confirmed = ?1 WHERE id = ?2",
            rusqlite::params![uc as i64, id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn health_episode_delete(
    state: State<'_, AppStateHandle>,
    id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    // Detach events first (FK is ON DELETE no-action by default in SQLite).
    let tables = [
        "medical_records",
        "lab_tests",
        "medications_v2",
        "health_conditions",
        "vitals",
        "life_events",
        "symptoms",
    ];
    for t in tables {
        let sql = format!("UPDATE {} SET episode_id = NULL WHERE episode_id = ?1", t);
        let _ = conn.execute(&sql, rusqlite::params![id]);
    }
    conn.execute("DELETE FROM episodes WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn health_episode_attach(
    state: State<'_, AppStateHandle>,
    episode_id: String,
    event_kind: String,
    event_id: String,
) -> Result<(), String> {
    let table = match event_kind.as_str() {
        "medical_record" => "medical_records",
        "lab_test" => "lab_tests",
        "medication" => "medications_v2",
        "condition" => "health_conditions",
        "vital" => "vitals",
        "life_event" => "life_events",
        "symptom" => "symptoms",
        other => return Err(format!("unknown event_kind: {other}")),
    };
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let sql = format!("UPDATE {} SET episode_id = ?1 WHERE id = ?2", table);
    conn.execute(&sql, rusqlite::params![episode_id, event_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EpisodeAutolinkResult {
    pub episodes_created: i64,
    pub events_linked: i64,
}

/// Cluster un-episoded events into auto-named episodes purely by temporal
/// proximity. `gap_days` controls how many days of silence break a cluster
/// (default 14). Only events with `episode_id IS NULL` are touched, so this
/// is safe to run repeatedly and never overrides user assignments.
#[tauri::command]
pub async fn health_episode_autolink(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    gap_days: Option<i64>,
) -> Result<EpisodeAutolinkResult, String> {
    let gap = gap_days.unwrap_or(14);

    // Pull all events that don't yet belong to an episode.
    let mut all = health_timeline_get(state.clone(), patient_id.clone(), None, None).await?;
    all.retain(|e| e.episode_id.is_none());
    if all.is_empty() {
        return Ok(EpisodeAutolinkResult {
            episodes_created: 0,
            events_linked: 0,
        });
    }
    // Sort oldest-first so cluster boundaries are deterministic.
    all.sort_by(|a, b| a.date.cmp(&b.date));

    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut episodes_created = 0i64;
    let mut events_linked = 0i64;

    let mut cluster: Vec<TimelineEvent> = Vec::new();
    let mut last_date: Option<NaiveDate> = None;

    fn parse_date(s: &str) -> Option<NaiveDate> {
        // Accept full RFC3339 or YYYY-MM-DD.
        NaiveDate::parse_from_str(&s[..s.len().min(10)], "%Y-%m-%d").ok()
    }

    let flush = |conn: &rusqlite::Connection,
                 cluster: &[TimelineEvent],
                 patient_id: &str,
                 episodes_created: &mut i64,
                 events_linked: &mut i64|
     -> Result<(), String> {
        if cluster.len() < 2 {
            return Ok(()); // single-event "episodes" are noise
        }
        let start = cluster.first().unwrap().date.clone();
        let end = cluster.last().unwrap().date.clone();
        // Pick a name from the most prominent event in the cluster.
        let name_seed = cluster
            .iter()
            .find(|e| e.kind == "condition")
            .or_else(|| cluster.iter().find(|e| e.kind == "medical_record"))
            .or_else(|| cluster.iter().find(|e| e.kind == "symptom"))
            .unwrap_or(&cluster[0])
            .title
            .clone();
        let name = format!("{} ({})", name_seed, &start[..start.len().min(10)]);
        let primary_condition = cluster
            .iter()
            .find(|e| e.kind == "condition")
            .map(|e| e.title.clone());
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO episodes (id, patient_id, name, description, start_date,
             end_date, primary_condition, ai_generated, user_confirmed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 0)",
            rusqlite::params![
                id,
                patient_id,
                name,
                None::<String>,
                start,
                end,
                primary_condition,
            ],
        )
        .map_err(|e| e.to_string())?;
        *episodes_created += 1;

        for ev in cluster {
            let table = match ev.kind.as_str() {
                "medical_record" => "medical_records",
                "lab_test" => "lab_tests",
                "medication" => "medications_v2",
                "condition" => "health_conditions",
                "vital" => "vitals",
                "life_event" => "life_events",
                "symptom" => "symptoms",
                _ => continue,
            };
            let sql = format!("UPDATE {} SET episode_id = ?1 WHERE id = ?2", table);
            if conn
                .execute(&sql, rusqlite::params![id, ev.id])
                .unwrap_or(0)
                > 0
            {
                *events_linked += 1;
            }
        }
        Ok(())
    };

    for ev in all {
        let date = match parse_date(&ev.date) {
            Some(d) => d,
            None => continue,
        };
        match last_date {
            Some(prev) if (date - prev).num_days() <= gap => {
                cluster.push(ev);
            }
            _ => {
                flush(
                    &conn,
                    &cluster,
                    &patient_id,
                    &mut episodes_created,
                    &mut events_linked,
                )?;
                cluster.clear();
                cluster.push(ev);
            }
        }
        last_date = Some(date);
    }
    flush(
        &conn,
        &cluster,
        &patient_id,
        &mut episodes_created,
        &mut events_linked,
    )?;

    Ok(EpisodeAutolinkResult {
        episodes_created,
        events_linked,
    })
}

// =====================================================================
// Symptom LLM classification
// =====================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct SymptomClassification {
    #[serde(default)]
    pub canonical_name: Option<String>,
    #[serde(default)]
    pub body_part: Option<String>,
    #[serde(default)]
    pub laterality: Option<String>,
    #[serde(default)]
    pub severity: Option<i64>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

const SYMPTOM_SYSTEM: &str = "You are a medical text normalizer. Given a \
free-text symptom description from a patient, extract structured fields. \
Return JSON. canonical_name should be the medical term (e.g. 'cephalalgia' \
or 'headache'). body_part is the affected anatomy in plain English. \
laterality is one of left|right|bilateral|unspecified. severity is 1-10 \
inferred from intensity words. category is one of pain|gi|cardiac|\
respiratory|neuro|skin|mental|fatigue|other. keywords are 3-6 lowercased \
search terms. Do not invent diagnoses.";

const SYMPTOM_EXAMPLE: &str = r#"{
  "canonical_name": "headache",
  "body_part": "head",
  "laterality": "left",
  "severity": 7,
  "category": "neuro",
  "keywords": ["headache", "throbbing", "left side", "migraine"]
}"#;

#[tauri::command]
pub async fn health_classify_symptom(
    state: State<'_, AppStateHandle>,
    text: String,
) -> Result<SymptomClassification, String> {
    let cfg = classify_endpoint_for_feature(&state, "health_extract")
        .await?
        .ok_or_else(|| "no LLM endpoint configured".to_string())?;
    let provider = create_provider(cfg);
    let req = JsonExtractRequest {
        system_prompt: SYMPTOM_SYSTEM.to_string(),
        user_input: text,
        example_json: SYMPTOM_EXAMPLE.to_string(),
        model: None,
        temperature: Some(0.0),
    };
    let resp = provider
        .extract_json(req)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::from_value(resp.parsed)
        .map_err(|e| format!("symptom classifier returned bad JSON: {e}"))
}

/// Update an existing symptom row with classifier output.
#[tauri::command]
pub async fn health_apply_symptom_classification(
    state: State<'_, AppStateHandle>,
    symptom_id: String,
    classification: SymptomClassification,
) -> Result<(), String> {
    let llm_meta = serde_json::to_string(&classification).unwrap_or_else(|_| "null".into());
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE symptoms
         SET canonical_name = COALESCE(?1, canonical_name),
             body_part      = COALESCE(?2, body_part),
             laterality     = COALESCE(?3, laterality),
             severity       = COALESCE(?4, severity),
             llm_metadata   = ?5
         WHERE id = ?6",
        rusqlite::params![
            classification.canonical_name,
            classification.body_part,
            classification.laterality,
            classification.severity,
            llm_meta,
            symptom_id,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
// Correlation graph
// =====================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Correlation {
    pub id: String,
    pub source_kind: String,
    pub source_id: String,
    pub source_title: String,
    pub source_date: String,
    pub target_kind: String,
    pub target_id: String,
    pub target_title: String,
    pub target_date: String,
    pub relation: String,
    pub confidence: f64,
    pub delta_days: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CorrelateResult {
    pub correlations_created: i64,
    pub sources_scanned: i64,
}

/// Recompute the cached correlation graph for a patient. Source events
/// are symptoms, life events, and conditions; targets are any other event.
/// Confidence is `1 - (|delta_days| / window_days)`, clamped to [0, 1].
#[tauri::command]
pub async fn health_correlate(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    window_days: Option<i64>,
) -> Result<CorrelateResult, String> {
    let window = window_days.unwrap_or(30).max(1);
    let all = health_timeline_get(state.clone(), patient_id.clone(), None, None).await?;

    let parse = |s: &str| -> Option<NaiveDate> {
        NaiveDate::parse_from_str(&s[..s.len().min(10)], "%Y-%m-%d").ok()
    };

    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    // Wipe stale rows first so deletions on the input side propagate.
    conn.execute(
        "DELETE FROM health_correlations WHERE patient_id = ?1",
        rusqlite::params![patient_id],
    )
    .map_err(|e| e.to_string())?;

    let mut created = 0i64;
    let mut sources_scanned = 0i64;
    for src in &all {
        if !matches!(src.kind.as_str(), "symptom" | "life_event" | "condition") {
            continue;
        }
        let Some(src_date) = parse(&src.date) else {
            continue;
        };
        sources_scanned += 1;
        for tgt in &all {
            if tgt.id == src.id && tgt.kind == src.kind {
                continue;
            }
            // Skip self-on-self pairs of the same source category (symptom→symptom etc.).
            if tgt.kind == src.kind {
                continue;
            }
            let Some(tgt_date) = parse(&tgt.date) else {
                continue;
            };
            let delta = (tgt_date - src_date).num_days();
            if delta.abs() > window {
                continue;
            }
            let confidence = 1.0 - (delta.abs() as f64 / window as f64);
            if confidence <= 0.0 {
                continue;
            }
            let relation = if delta == 0 {
                "concurrent"
            } else if delta > 0 {
                "precedes" // source precedes target
            } else {
                "follows"
            };
            let id = uuid::Uuid::new_v4().to_string();
            let inserted = conn
                .execute(
                    "INSERT OR IGNORE INTO health_correlations
                     (id, patient_id, source_kind, source_id, target_kind,
                      target_id, relation, confidence, delta_days)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    rusqlite::params![
                        id, patient_id, src.kind, src.id, tgt.kind, tgt.id, relation, confidence,
                        delta,
                    ],
                )
                .map_err(|e| e.to_string())?;
            if inserted > 0 {
                created += 1;
            }
        }
    }
    Ok(CorrelateResult {
        correlations_created: created,
        sources_scanned,
    })
}

#[tauri::command]
pub async fn health_list_correlations(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    source_kind: Option<String>,
    source_id: Option<String>,
    min_confidence: Option<f64>,
) -> Result<Vec<Correlation>, String> {
    let min_conf = min_confidence.unwrap_or(0.1);
    let timeline = health_timeline_get(state.clone(), patient_id.clone(), None, None).await?;

    let lookup: HashMap<(String, String), &TimelineEvent> = timeline
        .iter()
        .map(|e| ((e.kind.clone(), e.id.clone()), e))
        .collect();

    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut sql = String::from(
        "SELECT id, source_kind, source_id, target_kind, target_id, relation,
                confidence, delta_days
         FROM health_correlations
         WHERE patient_id = ?1 AND confidence >= ?2",
    );
    let mut params: Vec<Box<dyn rusqlite::ToSql>> =
        vec![Box::new(patient_id.clone()), Box::new(min_conf)];
    if let Some(sk) = &source_kind {
        sql.push_str(" AND source_kind = ?3");
        params.push(Box::new(sk.clone()));
    }
    if let Some(sid) = &source_id {
        let n = params.len() + 1;
        sql.push_str(&format!(" AND source_id = ?{n}"));
        params.push(Box::new(sid.clone()));
    }
    sql.push_str(" ORDER BY confidence DESC LIMIT 500");

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for r in rows {
        let (id, sk, sid, tk, tid, rel, conf, delta) = r.map_err(|e| e.to_string())?;
        let src = lookup.get(&(sk.clone(), sid.clone()));
        let tgt = lookup.get(&(tk.clone(), tid.clone()));
        let (Some(src), Some(tgt)) = (src, tgt) else {
            continue; // event was deleted after correlate ran
        };
        out.push(Correlation {
            id,
            source_kind: sk,
            source_id: sid,
            source_title: src.title.clone(),
            source_date: src.date.clone(),
            target_kind: tk,
            target_id: tid,
            target_title: tgt.title.clone(),
            target_date: tgt.date.clone(),
            relation: rel,
            confidence: conf,
            delta_days: delta,
        });
    }
    Ok(out)
}

// =====================================================================
// Convenience: today's date (used by UI defaults)
// =====================================================================

#[allow(dead_code)]
fn today() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

#[allow(dead_code)]
fn days_between(a: &str, b: &str) -> i64 {
    let pa = NaiveDate::parse_from_str(&a[..a.len().min(10)], "%Y-%m-%d");
    let pb = NaiveDate::parse_from_str(&b[..b.len().min(10)], "%Y-%m-%d");
    match (pa, pb) {
        (Ok(da), Ok(db)) => (db - da).num_days(),
        _ => 0,
    }
}

#[allow(dead_code)]
fn add_days(date: &str, n: i64) -> String {
    NaiveDate::parse_from_str(&date[..date.len().min(10)], "%Y-%m-%d")
        .map(|d| (d + Duration::days(n)).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|_| date.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_days_works() {
        assert_eq!(add_days("2024-01-01", 5), "2024-01-06");
        assert_eq!(add_days("2024-12-31", 1), "2025-01-01");
    }

    #[test]
    fn days_between_works() {
        assert_eq!(days_between("2024-01-01", "2024-01-08"), 7);
        assert_eq!(days_between("2024-01-08", "2024-01-01"), -7);
    }
}

// Type for the `EndpointHandle` re-export from health_classify so we don't
// have to make it pub(crate); used only inside this module.
#[allow(dead_code)]
type _Hint = EndpointHandle;
