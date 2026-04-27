//! Health Phase B — structured extraction of prescriptions and lab results.

use crate::state::AppState;
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
pub struct PrescriptionItem {
    pub drug_name: String,
    pub dosage: Option<String>,
    pub frequency: Option<String>,
    pub duration_days: Option<i64>,
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrescriptionExtraction {
    pub prescribed_date: Option<String>,
    pub prescriber_name: Option<String>,
    pub prescriber_specialty: Option<String>,
    pub facility_name: Option<String>,
    pub location_city: Option<String>,
    pub diagnosis_text: Option<String>,
    pub medications: Vec<PrescriptionItem>,
    pub raw_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabValue {
    pub test_name: String,
    pub value_text: String,
    pub value_numeric: Option<f64>,
    pub unit: Option<String>,
    pub reference_low: Option<f64>,
    pub reference_high: Option<f64>,
    pub flag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabResultExtraction {
    pub lab_name: Option<String>,
    pub report_date: Option<String>,
    pub location_city: Option<String>,
    pub results: Vec<LabValue>,
    pub raw_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrescriptionWithItems {
    pub id: String,
    pub patient_id: String,
    pub source_file_id: Option<String>,
    pub prescribed_date: String,
    pub prescriber_name: Option<String>,
    pub prescriber_specialty: Option<String>,
    pub facility_name: Option<String>,
    pub location_city: Option<String>,
    pub diagnosis_text: Option<String>,
    pub confirmed: bool,
    pub created_at: String,
    pub items: Vec<PrescriptionItemRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrescriptionItemRow {
    pub id: String,
    pub drug_name: String,
    pub dosage: Option<String>,
    pub frequency: Option<String>,
    pub duration_days: Option<i64>,
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabResultWithValues {
    pub id: String,
    pub patient_id: String,
    pub source_file_id: Option<String>,
    pub lab_name: Option<String>,
    pub report_date: String,
    pub location_city: Option<String>,
    pub confirmed: bool,
    pub created_at: String,
    pub values: Vec<LabValueRow>,
    pub abnormal_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabValueRow {
    pub id: String,
    pub test_name: String,
    pub value_text: String,
    pub value_numeric: Option<f64>,
    pub unit: Option<String>,
    pub reference_low: Option<f64>,
    pub reference_high: Option<f64>,
    pub flag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabTrendPoint {
    pub date: String,
    pub value_numeric: f64,
    pub flag: Option<String>,
    pub lab_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ExtractionPreview {
    Prescription(PrescriptionExtraction),
    Lab(LabResultExtraction),
    Unsupported { doc_type: String },
}

// ── LLM helpers ──────────────────────────────────────────────────────────────

fn get_endpoint(conn: &Conn) -> Option<(String, Option<String>, String)> {
    conn.query_row(
        "SELECT base_url, api_key_encrypted, COALESCE(default_model, 'llama3')
         FROM llm_endpoints LIMIT 1",
        [],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?)),
    ).ok()
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
    let resp = req.send().await
        .map_err(|e| tracing::warn!("LLM call failed: {e}"))
        .ok()?;
    if !resp.status().is_success() {
        tracing::warn!("LLM returned {}", resp.status());
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    json["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
}

/// Strip markdown code fences if the LLM wrapped the JSON in ```json ... ```
fn extract_json_block(raw: &str) -> &str {
    let trimmed = raw.trim();
    if trimmed.starts_with("```") {
        let after = trimmed.trim_start_matches('`').trim_start_matches("json").trim_start();
        if let Some(end) = after.rfind("```") {
            return after[..end].trim();
        }
        return after;
    }
    trimmed
}

/// Deterministic flag from numeric value vs reference range.
fn compute_flag(value: Option<f64>, low: Option<f64>, high: Option<f64>) -> Option<String> {
    match (value, low, high) {
        (Some(v), _, Some(h)) if v > h * 1.5 => Some("CRITICAL".to_string()),
        (Some(v), _, Some(h)) if v > h => Some("HIGH".to_string()),
        (Some(v), Some(l), _) if v < l => Some("LOW".to_string()),
        (Some(_), _, _) => Some("NORMAL".to_string()),
        _ => None,
    }
}

// ── Extract command ───────────────────────────────────────────────────────────

/// Run LLM extraction on an already-ingested file. Returns a preview — does NOT write to DB.
/// Returns None if no LLM endpoint is configured.
#[tauri::command]
pub async fn health_extract_document(
    state: State<'_, AppStateHandle>,
    file_id: String,
    patient_id: String,
) -> Result<Option<ExtractionPreview>, String> {
    let db = { state.read().await.db.clone() };

    // Read raw text and doc_type — drop conn before async work
    let (raw_text, doc_type): (String, String) = {
        let conn = db.get().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT COALESCE(extracted_text, ''), COALESCE(doc_type, 'unknown')
             FROM file_manifest WHERE id = ?1",
            params![file_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        ).map_err(|_| format!("File {} not found", file_id))?
    };

    if raw_text.trim().is_empty() {
        return Err("File has no extracted text. Run ingestion first.".to_string());
    }

    // Get LLM endpoint — drop conn before async
    let endpoint = {
        let conn = db.get().map_err(|e| e.to_string())?;
        get_endpoint(&conn)
    };
    let Some((base_url, api_key, model)) = endpoint else {
        return Ok(None);
    };

    // patient_id is part of the public API; used by caller to associate confirmed records
    let _patient_id = patient_id;

    // Truncate to ~12 000 chars to stay within most local model context windows
    let excerpt = &raw_text[..raw_text.len().min(12_000)];

    match doc_type.to_lowercase().as_str() {
        "prescription" | "prescription_note" => {
            let system = "You are a medical document parser. Extract prescription data and return \
                          ONLY valid JSON with no markdown fences, no explanation, no preamble.";
            let user = format!(
                "Extract into JSON matching exactly this schema:\n\
                 {{\"prescribed_date\":\"YYYY-MM-DD or null\",\
                 \"prescriber_name\":\"string or null\",\
                 \"prescriber_specialty\":\"string or null\",\
                 \"facility_name\":\"string or null\",\
                 \"location_city\":\"string or null\",\
                 \"diagnosis_text\":\"string or null\",\
                 \"medications\":[{{\"drug_name\":\"string\",\"dosage\":\"string or null\",\
                 \"frequency\":\"string or null\",\"duration_days\":N or null,\
                 \"instructions\":\"string or null\"}}]}}\n\nPrescription text:\n{}",
                excerpt
            );
            let Some(raw) =
                call_llm(&base_url, api_key.as_deref(), &model, system, &user).await
            else {
                return Ok(None);
            };
            let json_str = extract_json_block(&raw);
            let mut parsed: PrescriptionExtraction = serde_json::from_str(json_str)
                .map_err(|e| format!("LLM returned invalid JSON: {e}\nRaw: {raw}"))?;
            parsed.raw_text = raw_text.clone();
            Ok(Some(ExtractionPreview::Prescription(parsed)))
        }

        "lab_report" | "lab_result" | "blood_test" => {
            let system = "You are a medical document parser. Extract lab report data and return \
                          ONLY valid JSON with no markdown fences, no explanation, no preamble.";
            let user = format!(
                "Extract into JSON matching exactly this schema:\n\
                 {{\"lab_name\":\"string or null\",\
                 \"report_date\":\"YYYY-MM-DD or null\",\
                 \"location_city\":\"string or null\",\
                 \"results\":[{{\"test_name\":\"string\",\"value_text\":\"string\",\
                 \"value_numeric\":N or null,\"unit\":\"string or null\",\
                 \"reference_low\":N or null,\"reference_high\":N or null}}]}}\n\nLab report text:\n{}",
                excerpt
            );
            let Some(raw) =
                call_llm(&base_url, api_key.as_deref(), &model, system, &user).await
            else {
                return Ok(None);
            };
            let json_str = extract_json_block(&raw);
            let mut parsed: LabResultExtraction = serde_json::from_str(json_str)
                .map_err(|e| format!("LLM returned invalid JSON: {e}\nRaw: {raw}"))?;
            // Compute flags deterministically
            for result in &mut parsed.results {
                result.flag =
                    compute_flag(result.value_numeric, result.reference_low, result.reference_high);
            }
            parsed.raw_text = raw_text.clone();
            Ok(Some(ExtractionPreview::Lab(parsed)))
        }

        other => Ok(Some(ExtractionPreview::Unsupported { doc_type: other.to_string() })),
    }
}

// ── suppress dead-code warnings for types used by future Task 4 commands ─────
const _: () = {
    let _ = std::mem::size_of::<PrescriptionWithItems>();
    let _ = std::mem::size_of::<PrescriptionItemRow>();
    let _ = std::mem::size_of::<LabResultWithValues>();
    let _ = std::mem::size_of::<LabValueRow>();
    let _ = std::mem::size_of::<LabTrendPoint>();
    let _ = std::mem::size_of::<Uuid>(); // ensure uuid import is used
};
