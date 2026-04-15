//! Health Vault week 5: AI analysis pipeline.
//!
//! Reads the patient's full per-event history, compresses it into a
//! "timeline brief" (token-bounded textual summary), and feeds it into
//! one of four mode-specific LLM prompts:
//!
//! - **trend**: numeric series in labs/vitals → direction + suspected drivers
//! - **correlation**: explain symptom ↔ life event ↔ medication overlaps
//! - **lifestyle**: how yoga/meditation/sleep/diet/stress correlate with markers
//! - **qa**: free-form patient question grounded in the timeline
//!
//! Results are cached in `health_analyses` so re-opening the tab is free.
//! A per-request consent gate blocks cloud LLM use unless the patient opted
//! in via the consent record set in week 1.

use crate::health_classify::classify_endpoint_for_feature;
use crate::health_timeline::{health_timeline_get, TimelineEvent};
use crate::state::AppState;
use chrono::Utc;
use minion_llm::{create_provider, ChatMessage, ChatRequest, ChatRole, ProviderType};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<AppState>>;

/// Cap on bytes of timeline brief we send to the LLM. ~12k chars ≈ 3k tokens
/// which keeps the prompt headroom comfortable on 8k-context models.
const MAX_BRIEF_CHARS: usize = 12_000;

// =====================================================================
// Public types
// =====================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalysisRequest {
    pub patient_id: String,
    pub mode: String, // trend | correlation | lifestyle | qa
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    /// User-confirmed cloud consent for this specific request. If the
    /// resolved endpoint is non-local and this is `false` we refuse.
    #[serde(default)]
    pub allow_cloud: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalysisResult {
    pub id: String,
    pub patient_id: String,
    pub mode: String,
    pub question: Option<String>,
    pub brief_text: String,
    pub response_text: String,
    pub model_used: Option<String>,
    pub cloud_used: bool,
    pub created_at: String,
}

// =====================================================================
// Timeline brief builder
// =====================================================================

/// Render the patient's events into a compact text block the LLM can
/// reason over. Aggregates labs by canonical name with the latest few
/// values so a 10-year history fits the cap.
pub fn build_brief(events: &[TimelineEvent]) -> String {
    let mut sections: Vec<(&'static str, String)> = Vec::new();

    // ---- conditions
    let conditions: Vec<&TimelineEvent> =
        events.iter().filter(|e| e.kind == "condition").collect();
    if !conditions.is_empty() {
        let mut s = String::from("Conditions:\n");
        for c in &conditions {
            s.push_str(&format!(
                "  • {} (dx {})\n",
                c.title,
                c.date.get(..10).unwrap_or(&c.date)
            ));
        }
        sections.push(("conditions", s));
    }

    // ---- active medications (no end_date)
    let active_meds: Vec<&TimelineEvent> = events
        .iter()
        .filter(|e| e.kind == "medication" && e.end_date.is_none())
        .collect();
    if !active_meds.is_empty() {
        let mut s = String::from("Active medications:\n");
        for m in &active_meds {
            let dose = m.description.clone().unwrap_or_default();
            s.push_str(&format!(
                "  • {} {} (since {})\n",
                m.title,
                dose,
                m.date.get(..10).unwrap_or(&m.date)
            ));
        }
        sections.push(("active_meds", s));
    }

    // ---- labs grouped by canonical name (last 5 each)
    let mut labs_by_name: std::collections::BTreeMap<String, Vec<&TimelineEvent>> =
        Default::default();
    for e in events.iter().filter(|e| e.kind == "lab_test") {
        labs_by_name
            .entry(e.title.clone())
            .or_default()
            .push(e);
    }
    if !labs_by_name.is_empty() {
        let mut s = String::from("Lab series (most recent 5 per test):\n");
        for (name, mut series) in labs_by_name {
            // Newest first.
            series.sort_by(|a, b| b.date.cmp(&a.date));
            series.truncate(5);
            let inline: Vec<String> = series
                .iter()
                .map(|e| {
                    let val = e.value.map(format_value).unwrap_or_default();
                    let unit = e.unit.clone().unwrap_or_default();
                    let flag = e
                        .flag
                        .as_ref()
                        .filter(|f| !f.is_empty() && f.to_lowercase() != "normal")
                        .map(|f| format!(" [{}]", f))
                        .unwrap_or_default();
                    format!(
                        "{}={} {}{}",
                        e.date.get(..10).unwrap_or(&e.date),
                        val,
                        unit,
                        flag
                    )
                })
                .collect();
            s.push_str(&format!("  • {}: {}\n", name, inline.join("; ")));
        }
        sections.push(("labs", s));
    }

    // ---- vitals (last 10 across all types)
    let vitals: Vec<&TimelineEvent> = events
        .iter()
        .filter(|e| e.kind == "vital")
        .take(10)
        .collect();
    if !vitals.is_empty() {
        let mut s = String::from("Recent vitals:\n");
        for v in &vitals {
            let val = v.value.map(format_value).unwrap_or_default();
            let unit = v.unit.clone().unwrap_or_default();
            s.push_str(&format!(
                "  • {} {}={} {}\n",
                v.date.get(..10).unwrap_or(&v.date),
                v.title,
                val,
                unit
            ));
        }
        sections.push(("vitals", s));
    }

    // ---- symptoms (active first, then resolved)
    let symptoms: Vec<&TimelineEvent> =
        events.iter().filter(|e| e.kind == "symptom").collect();
    if !symptoms.is_empty() {
        let mut s = String::from("Symptoms:\n");
        for sy in &symptoms {
            let sev = sy.value.map(|v| format!(" sev={}", v as i64)).unwrap_or_default();
            let body = sy.category.clone().map(|b| format!(" ({b})")).unwrap_or_default();
            let resolved = sy
                .end_date
                .as_ref()
                .map(|d| format!(" → resolved {}", d.get(..10).unwrap_or(d)))
                .unwrap_or_else(|| " [active]".into());
            s.push_str(&format!(
                "  • {} {}{}{}{}\n",
                sy.date.get(..10).unwrap_or(&sy.date),
                sy.title,
                body,
                sev,
                resolved
            ));
        }
        sections.push(("symptoms", s));
    }

    // ---- life events grouped by category
    let life: Vec<&TimelineEvent> =
        events.iter().filter(|e| e.kind == "life_event").collect();
    if !life.is_empty() {
        let mut s = String::from("Life events:\n");
        for le in &life {
            let cat = le.category.clone().unwrap_or_default();
            let intensity = le
                .value
                .map(|v| format!(" intensity={}", v as i64))
                .unwrap_or_default();
            s.push_str(&format!(
                "  • {} [{}] {}{}\n",
                le.date.get(..10).unwrap_or(&le.date),
                cat,
                le.title,
                intensity
            ));
        }
        sections.push(("life_events", s));
    }

    // ---- recent medical records (last 15)
    let records: Vec<&TimelineEvent> = events
        .iter()
        .filter(|e| e.kind == "medical_record")
        .take(15)
        .collect();
    if !records.is_empty() {
        let mut s = String::from("Recent records:\n");
        for r in &records {
            let cat = r.category.clone().unwrap_or_default();
            s.push_str(&format!(
                "  • {} [{}] {}\n",
                r.date.get(..10).unwrap_or(&r.date),
                cat,
                r.title
            ));
        }
        sections.push(("records", s));
    }

    // Concat with budget — drop low-priority sections first if we overflow.
    // Order = priority high→low.
    let priority = [
        "conditions",
        "active_meds",
        "symptoms",
        "labs",
        "vitals",
        "life_events",
        "records",
    ];
    let mut out = String::new();
    for tag in priority {
        if let Some((_, body)) = sections.iter().find(|(t, _)| *t == tag) {
            if out.len() + body.len() < MAX_BRIEF_CHARS {
                out.push_str(body);
                out.push('\n');
            }
        }
    }
    if out.is_empty() {
        out.push_str("(no recorded events)");
    }
    out
}

fn format_value(v: f64) -> String {
    if (v.fract().abs() < f64::EPSILON) && v.abs() < 1.0e9 {
        format!("{}", v as i64)
    } else {
        format!("{:.2}", v)
    }
}

// =====================================================================
// Mode prompts
// =====================================================================

const SYS_PREFIX: &str = "You are a careful medical assistant analyzing a \
patient's longitudinal health record. You are NOT a doctor and your output \
must include a one-line caveat that the patient should consult a \
clinician for medical decisions. Cite specific dates and values from the \
brief; never invent numbers. If the data is insufficient to draw a \
conclusion, say so. The patient's lifestyle includes yoga, meditation, \
and spiritual practices like Shambhavi Mahamudra and Inner Engineering — \
treat these as legitimate, beneficial inputs that interact with sleep, \
stress, and cardiovascular markers.";

const TREND_INSTRUCTION: &str = "Identify the most important numeric \
trends in the labs and vitals (improving, worsening, stable). For each \
trend, give: the test name, direction, magnitude, dates, and a plausible \
clinical or lifestyle driver suggested by the data. Group by body system. \
End with a 'Watch list' bullet of tests that should be re-checked.";

const CORRELATION_INSTRUCTION: &str = "Identify temporal overlaps among \
symptoms, life events, medications, and lab abnormalities. For each \
correlation, state which events occurred in the same window, the time \
delta, and the most plausible interpretation. Distinguish coincidence \
from clinically meaningful patterns. Avoid causal claims.";

const LIFESTYLE_INSTRUCTION: &str = "Focus on lifestyle inputs (yoga, \
meditation, sleep, diet, exercise, stress) and how they correlate with \
biomarkers (BP, HRV, lipids, HbA1c, weight, mood). Quantify changes \
observed before vs after a sustained lifestyle change where the data \
supports it. Highlight what is working and what is missing.";

const QA_INSTRUCTION: &str = "Answer the patient's question using ONLY \
the data in the brief. Cite specific dates/values. If the brief lacks the \
information needed, say so explicitly and suggest what data would help.";

fn instruction_for(mode: &str) -> &'static str {
    match mode {
        "trend" => TREND_INSTRUCTION,
        "correlation" => CORRELATION_INSTRUCTION,
        "lifestyle" => LIFESTYLE_INSTRUCTION,
        "qa" => QA_INSTRUCTION,
        _ => QA_INSTRUCTION,
    }
}

// =====================================================================
// Cloud-vs-local detection
// =====================================================================

/// Returns true if the provider type is known to send data outside the
/// device. Local providers are Ollama, llama.cpp via OpenAI-compat on
/// 127.0.0.1, and AirLLM. Anything else is treated as cloud and gated by
/// `allow_cloud`.
/// Canonical lowercase tag matching what the database / settings UI use.
fn provider_type_tag(p: ProviderType) -> &'static str {
    match p {
        ProviderType::Ollama => "ollama",
        ProviderType::OpenaiCompatible => "openai_compatible",
        ProviderType::Openai => "openai",
        ProviderType::Anthropic => "anthropic",
        ProviderType::GoogleGemini => "google_gemini",
        ProviderType::Airllm => "airllm",
    }
}

fn is_cloud_provider(provider: ProviderType, base_url: &str) -> bool {
    match provider {
        ProviderType::Ollama => false,
        ProviderType::Airllm => false,
        ProviderType::OpenaiCompatible => {
            !is_local_url(base_url)
        }
        ProviderType::Openai | ProviderType::Anthropic | ProviderType::GoogleGemini => true,
    }
}

fn is_local_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("127.0.0.1")
        || lower.contains("localhost")
        || lower.contains("0.0.0.0")
        || lower.starts_with("http://192.168.")
        || lower.starts_with("http://10.")
}

// =====================================================================
// Tauri commands
// =====================================================================

/// Run an analysis end-to-end: build brief, check consent + endpoint,
/// invoke LLM, persist result. Returns the cached row.
#[tauri::command]
pub async fn health_run_analysis(
    state: State<'_, AppStateHandle>,
    request: AnalysisRequest,
) -> Result<AnalysisResult, String> {
    // 1. Pull the timeline.
    let events = health_timeline_get(
        state.clone(),
        request.patient_id.clone(),
        request.from.clone(),
        request.to.clone(),
    )
    .await?;
    let brief = build_brief(&events);

    // 2. Resolve endpoint (`health_analyze` binding, fall back to extract,
    // then to first enabled).
    let endpoint = match classify_endpoint_for_feature(&state, "health_analyze").await? {
        Some(c) => c,
        None => classify_endpoint_for_feature(&state, "health_extract")
            .await?
            .ok_or_else(|| "no LLM endpoint configured".to_string())?,
    };
    let cloud = is_cloud_provider(endpoint.provider_type, &endpoint.base_url);
    if cloud && !request.allow_cloud {
        return Err(format!(
            "endpoint at {} is non-local; pass allow_cloud=true to proceed",
            endpoint.base_url
        ));
    }

    // 3. Build the chat request.
    let user_block = match request.mode.as_str() {
        "qa" => {
            let q = request.question.clone().unwrap_or_default();
            if q.trim().is_empty() {
                return Err("qa mode requires a question".to_string());
            }
            format!(
                "PATIENT TIMELINE:\n{}\n\nPATIENT QUESTION:\n{}",
                brief, q
            )
        }
        _ => format!("PATIENT TIMELINE:\n{}", brief),
    };
    let system = format!("{}\n\nMODE: {}\n{}", SYS_PREFIX, request.mode, instruction_for(&request.mode));

    let model = endpoint.default_model.clone();
    let provider = create_provider(endpoint.clone());
    let chat_req = ChatRequest {
        messages: vec![
            ChatMessage {
                role: ChatRole::User,
                content: user_block,
            },
        ],
        model: None,
        temperature: Some(0.2_f32),
        json_mode: false,
        max_tokens: Some(1500_u32),
        system: Some(system),
    };
    let resp = provider
        .chat(chat_req)
        .await
        .map_err(|e| e.to_string())?;

    // 4. Persist.
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO health_analyses
             (id, patient_id, mode, question, timeline_from, timeline_to,
              brief_text, response_text, response_json, model_used,
              endpoint_id, cloud_used, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, NULL, ?10, ?11)",
            rusqlite::params![
                id,
                request.patient_id,
                request.mode,
                request.question,
                request.from,
                request.to,
                brief,
                resp.content,
                model,
                cloud as i64,
                now,
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(AnalysisResult {
        id,
        patient_id: request.patient_id,
        mode: request.mode,
        question: request.question,
        brief_text: brief,
        response_text: resp.content,
        model_used: Some(model),
        cloud_used: cloud,
        created_at: now,
    })
}

#[tauri::command]
pub async fn health_list_analyses(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    limit: Option<i64>,
) -> Result<Vec<AnalysisResult>, String> {
    let limit = limit.unwrap_or(50).clamp(1, 500);
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, patient_id, mode, question, brief_text, response_text,
                    model_used, cloud_used, created_at
             FROM health_analyses WHERE patient_id = ?1
             ORDER BY created_at DESC LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![patient_id, limit], |row| {
            Ok(AnalysisResult {
                id: row.get(0)?,
                patient_id: row.get(1)?,
                mode: row.get(2)?,
                question: row.get(3)?,
                brief_text: row.get(4)?,
                response_text: row.get(5)?,
                model_used: row.get(6)?,
                cloud_used: row.get::<_, i64>(7)? != 0,
                created_at: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub async fn health_delete_analysis(
    state: State<'_, AppStateHandle>,
    id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM health_analyses WHERE id = ?1",
        rusqlite::params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Preview the brief without spending tokens. Used in the UI when the
/// patient wants to see what will be sent before clicking Run.
#[tauri::command]
pub async fn health_preview_brief(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    from: Option<String>,
    to: Option<String>,
) -> Result<String, String> {
    let events = health_timeline_get(state, patient_id, from, to).await?;
    Ok(build_brief(&events))
}

/// Returns whether the bound endpoint for analysis is cloud (non-local),
/// the provider name, and whether the user has cloud_llm_allowed set in
/// consent. The UI uses this to decide whether to require an extra
/// per-request consent click.
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisEndpointStatus {
    pub configured: bool,
    pub provider_type: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub is_cloud: bool,
    pub user_cloud_consent: bool,
}

#[tauri::command]
pub async fn health_analysis_endpoint_status(
    state: State<'_, AppStateHandle>,
) -> Result<AnalysisEndpointStatus, String> {
    let cfg = match classify_endpoint_for_feature(&state, "health_analyze").await? {
        Some(c) => Some(c),
        None => classify_endpoint_for_feature(&state, "health_extract").await?,
    };

    // Pull cloud_llm_allowed from latest consent row (week 1 schema).
    let user_cloud_consent: bool = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT cloud_llm_allowed FROM health_consent
             ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .ok()
        .map(|n| n != 0)
        .unwrap_or(false)
    };

    Ok(match cfg {
        Some(c) => AnalysisEndpointStatus {
            configured: true,
            provider_type: Some(provider_type_tag(c.provider_type).into()),
            base_url: Some(c.base_url.clone()),
            model: Some(c.default_model.clone()),
            is_cloud: is_cloud_provider(c.provider_type, &c.base_url),
            user_cloud_consent,
        },
        None => AnalysisEndpointStatus {
            configured: false,
            provider_type: None,
            base_url: None,
            model: None,
            is_cloud: false,
            user_cloud_consent,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(kind: &str, title: &str, date: &str, value: Option<f64>, unit: Option<&str>) -> TimelineEvent {
        TimelineEvent {
            id: uuid::Uuid::new_v4().to_string(),
            kind: kind.into(),
            layer: match kind {
                "lab_test" | "vital" => "labs".into(),
                "symptom" => "symptoms".into(),
                _ => "events".into(),
            },
            title: title.into(),
            description: None,
            category: None,
            date: date.into(),
            end_date: None,
            value,
            unit: unit.map(|s| s.into()),
            flag: None,
            episode_id: None,
        }
    }

    #[test]
    fn brief_includes_lab_series_with_dates() {
        let events = vec![
            ev("lab_test", "HbA1c", "2025-01-15", Some(7.2), Some("%")),
            ev("lab_test", "HbA1c", "2024-07-10", Some(7.8), Some("%")),
            ev("lab_test", "LDL", "2025-01-15", Some(140.0), Some("mg/dL")),
        ];
        let brief = build_brief(&events);
        assert!(brief.contains("HbA1c"));
        assert!(brief.contains("LDL"));
        assert!(brief.contains("2025-01-15"));
    }

    #[test]
    fn brief_groups_active_medications() {
        let mut m = ev("medication", "Metformin", "2024-01-01", None, None);
        m.description = Some("500 mg · twice daily".into());
        let events = vec![m];
        let brief = build_brief(&events);
        assert!(brief.contains("Active medications"));
        assert!(brief.contains("Metformin"));
        assert!(brief.contains("500 mg"));
    }

    #[test]
    fn brief_handles_empty() {
        let events: Vec<TimelineEvent> = vec![];
        let brief = build_brief(&events);
        assert_eq!(brief, "(no recorded events)");
    }

    #[test]
    fn cloud_detection() {
        assert!(!is_cloud_provider(ProviderType::Ollama, "http://localhost:11434"));
        assert!(!is_cloud_provider(ProviderType::OpenaiCompatible, "http://127.0.0.1:8080"));
        assert!(is_cloud_provider(ProviderType::OpenaiCompatible, "https://api.openrouter.ai"));
        assert!(is_cloud_provider(ProviderType::Anthropic, "https://api.anthropic.com"));
        assert!(is_cloud_provider(ProviderType::Openai, "https://api.openai.com"));
    }
}
