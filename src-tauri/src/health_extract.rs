//! Health Phase B — structured extraction of prescriptions and lab results.

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

// ── Confirm commands ─────────────────────────────────────────────────────────

/// Save confirmed prescription extraction to DB. Returns new prescription id.
#[tauri::command]
pub async fn health_confirm_prescription(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    source_file_id: Option<String>,
    data: PrescriptionExtraction,
) -> Result<String, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let rx_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO prescriptions
         (id, patient_id, source_file_id, prescribed_date, prescriber_name,
          prescriber_specialty, facility_name, location_city, diagnosis_text, raw_text,
          confirmed, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,1,?11)",
        params![
            rx_id, patient_id, source_file_id,
            data.prescribed_date.as_deref().unwrap_or("unknown"),
            data.prescriber_name, data.prescriber_specialty,
            data.facility_name, data.location_city,
            data.diagnosis_text, data.raw_text, now
        ],
    ).map_err(|e| e.to_string())?;
    for item in &data.medications {
        let item_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO prescription_items
             (id, prescription_id, drug_name, dosage, frequency, duration_days, instructions, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                item_id, rx_id, item.drug_name, item.dosage,
                item.frequency, item.duration_days, item.instructions, now
            ],
        ).map_err(|e| e.to_string())?;
    }
    Ok(rx_id)
}

/// Save confirmed lab result extraction to DB. Returns new lab result id.
#[tauri::command]
pub async fn health_confirm_lab_result(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    source_file_id: Option<String>,
    data: LabResultExtraction,
) -> Result<String, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let result_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO structured_lab_results
         (id, patient_id, source_file_id, lab_name, report_date, location_city, confirmed, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,1,?7)",
        params![
            result_id, patient_id, source_file_id,
            data.lab_name,
            data.report_date.as_deref().unwrap_or("unknown"),
            data.location_city, now
        ],
    ).map_err(|e| e.to_string())?;
    for val in &data.results {
        let val_id = Uuid::new_v4().to_string();
        let flag = compute_flag(val.value_numeric, val.reference_low, val.reference_high);
        conn.execute(
            "INSERT INTO structured_lab_values
             (id, result_id, test_name, value_text, value_numeric, unit,
              reference_low, reference_high, flag, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                val_id, result_id, val.test_name, val.value_text,
                val.value_numeric, val.unit,
                val.reference_low, val.reference_high,
                flag, now
            ],
        ).map_err(|e| e.to_string())?;
    }
    Ok(result_id)
}

// ── List commands ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn health_list_prescriptions(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<PrescriptionWithItems>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, patient_id, source_file_id, prescribed_date, prescriber_name,
                prescriber_specialty, facility_name, location_city, diagnosis_text,
                confirmed, created_at
         FROM prescriptions WHERE patient_id = ?1 ORDER BY prescribed_date DESC"
    ).map_err(|e| e.to_string())?;
    let rows: Vec<PrescriptionWithItems> = stmt.query_map(params![patient_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, Option<String>>(7)?,
            r.get::<_, Option<String>>(8)?,
            r.get::<_, i64>(9)?,
            r.get::<_, String>(10)?,
        ))
    }).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .map(|(id, pid, sfid, date, pname, pspec, fname, lcity, diag, conf, cat)| {
        let items = {
            let mut s = conn.prepare(
                "SELECT id, drug_name, dosage, frequency, duration_days, instructions
                 FROM prescription_items WHERE prescription_id = ?1 ORDER BY rowid"
            ).unwrap();
            s.query_map(params![id], |r| Ok(PrescriptionItemRow {
                id: r.get(0)?,
                drug_name: r.get(1)?,
                dosage: r.get(2)?,
                frequency: r.get(3)?,
                duration_days: r.get(4)?,
                instructions: r.get(5)?,
            })).unwrap().filter_map(|r| r.ok()).collect()
        };
        PrescriptionWithItems {
            id, patient_id: pid, source_file_id: sfid,
            prescribed_date: date, prescriber_name: pname,
            prescriber_specialty: pspec, facility_name: fname,
            location_city: lcity, diagnosis_text: diag,
            confirmed: conf != 0, created_at: cat, items,
        }
    }).collect();
    Ok(rows)
}

#[tauri::command]
pub async fn health_list_lab_results(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<LabResultWithValues>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, patient_id, source_file_id, lab_name, report_date,
                location_city, confirmed, created_at
         FROM structured_lab_results WHERE patient_id = ?1 ORDER BY report_date DESC"
    ).map_err(|e| e.to_string())?;
    let rows: Vec<LabResultWithValues> = stmt.query_map(params![patient_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, i64>(6)?,
            r.get::<_, String>(7)?,
        ))
    }).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .map(|(id, pid, sfid, lname, date, lcity, conf, cat)| {
        let (values, abnormal_count) = {
            let mut s = conn.prepare(
                "SELECT id, test_name, value_text, value_numeric, unit,
                        reference_low, reference_high, flag
                 FROM structured_lab_values WHERE result_id = ?1 ORDER BY rowid"
            ).unwrap();
            let vals: Vec<LabValueRow> = s.query_map(params![id], |r| Ok(LabValueRow {
                id: r.get(0)?,
                test_name: r.get(1)?,
                value_text: r.get(2)?,
                value_numeric: r.get(3)?,
                unit: r.get(4)?,
                reference_low: r.get(5)?,
                reference_high: r.get(6)?,
                flag: r.get(7)?,
            })).unwrap().filter_map(|r| r.ok()).collect();
            let abn = vals.iter().filter(|v| {
                matches!(v.flag.as_deref(), Some("HIGH") | Some("LOW") | Some("CRITICAL"))
            }).count() as i64;
            (vals, abn)
        };
        LabResultWithValues {
            id, patient_id: pid, source_file_id: sfid,
            lab_name: lname, report_date: date,
            location_city: lcity, confirmed: conf != 0,
            created_at: cat, values, abnormal_count,
        }
    }).collect();
    Ok(rows)
}

// ── Delete commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn health_delete_prescription(
    state: State<'_, AppStateHandle>,
    id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM prescriptions WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn health_delete_lab_result(
    state: State<'_, AppStateHandle>,
    id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM structured_lab_results WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Trend query ───────────────────────────────────────────────────────────────

/// Return all numeric values for one test name, ordered by date (for trend charts).
#[tauri::command]
pub async fn health_get_lab_trends(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    test_name: String,
) -> Result<Vec<LabTrendPoint>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT r.report_date, v.value_numeric, v.flag, r.lab_name
         FROM structured_lab_values v
         JOIN structured_lab_results r ON r.id = v.result_id
         WHERE r.patient_id = ?1
           AND LOWER(v.test_name) LIKE LOWER(?2)
           AND v.value_numeric IS NOT NULL
         ORDER BY r.report_date ASC"
    ).map_err(|e| e.to_string())?;
    let rows: Vec<LabTrendPoint> = stmt.query_map(
        params![patient_id, format!("%{}%", test_name)],
        |r| Ok(LabTrendPoint {
            date: r.get(0)?,
            value_numeric: r.get(1)?,
            flag: r.get(2)?,
            lab_name: r.get(3)?,
        }),
    ).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok()).collect();
    Ok(rows)
}
