//! Health Vault document classification + structured extraction (week 3).
//!
//! Wires the LLM layer from `minion-llm` into the ingestion pipeline:
//!
//! 1. **Classify** — sends the first ~2 KB of the document to the LLM and
//!    asks it to label the text as `lab_report | prescription | imaging_report
//!    | discharge_summary | consultation_note | vaccination_record | invoice
//!    | other`.
//! 2. **Extract** — dispatches to a schema-specific prompt that returns
//!    strict JSON (tests + values, medication list, imaging findings, …).
//! 3. **Persist** — updates `document_extractions` + `file_manifest.status`
//!    and waits for user review before any downstream entity (lab_test,
//!    medication, medical_record) is created.
//!
//! Entity canonicalization lives in `health_entities`.

use crate::health_entities::{canonicalize_drug, canonicalize_test, resolve_entity};
use crate::state::AppState;
use minion_llm::{
    create_provider, EndpointConfig, JsonExtractRequest, LlmProvider, ProviderType,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<AppState>>;

/// Maximum characters of raw text we send to the classifier.
const CLASSIFY_CHARS: usize = 2000;
/// Maximum characters of raw text we send to an extractor.
const EXTRACT_CHARS: usize = 16_000;

// =====================================================================
// Structured result types
// =====================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub document_type: String,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabTestEntry {
    #[serde(default)]
    pub test_name: Option<String>,
    #[serde(default)]
    pub canonical_name: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub reference_low: Option<f64>,
    #[serde(default)]
    pub reference_high: Option<f64>,
    #[serde(default)]
    pub reference_text: Option<String>,
    #[serde(default)]
    pub flag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LabReportExtract {
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub collected_at: Option<String>,
    #[serde(default)]
    pub reported_at: Option<String>,
    #[serde(default)]
    pub lab_name: Option<String>,
    #[serde(default)]
    pub ordering_doctor: Option<String>,
    #[serde(default)]
    pub tests: Vec<LabTestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationEntry {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub generic_name: Option<String>,
    #[serde(default)]
    pub dose: Option<String>,
    #[serde(default)]
    pub frequency: Option<String>,
    #[serde(default)]
    pub route: Option<String>,
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub indication: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrescriptionExtract {
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub prescribed_at: Option<String>,
    #[serde(default)]
    pub doctor_name: Option<String>,
    #[serde(default)]
    pub facility_name: Option<String>,
    #[serde(default)]
    pub medications: Vec<MedicationEntry>,
    #[serde(default)]
    pub diagnosis: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImagingExtract {
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub study_date: Option<String>,
    #[serde(default)]
    pub modality: Option<String>,
    #[serde(default)]
    pub body_part: Option<String>,
    #[serde(default)]
    pub radiologist: Option<String>,
    #[serde(default)]
    pub facility_name: Option<String>,
    #[serde(default)]
    pub findings: Option<String>,
    #[serde(default)]
    pub impression: Option<String>,
    #[serde(default)]
    pub recommendations: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DischargeExtract {
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub admission_date: Option<String>,
    #[serde(default)]
    pub discharge_date: Option<String>,
    #[serde(default)]
    pub hospital_name: Option<String>,
    #[serde(default)]
    pub attending_doctor: Option<String>,
    #[serde(default)]
    pub primary_diagnosis: Option<String>,
    #[serde(default)]
    pub secondary_diagnoses: Vec<String>,
    #[serde(default)]
    pub procedures: Vec<String>,
    #[serde(default)]
    pub medications: Vec<MedicationEntry>,
    #[serde(default)]
    pub follow_up: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConsultationExtract {
    #[serde(default)]
    pub patient_name: Option<String>,
    #[serde(default)]
    pub visit_date: Option<String>,
    #[serde(default)]
    pub doctor_name: Option<String>,
    #[serde(default)]
    pub facility_name: Option<String>,
    #[serde(default)]
    pub chief_complaint: Option<String>,
    #[serde(default)]
    pub history: Option<String>,
    #[serde(default)]
    pub examination: Option<String>,
    #[serde(default)]
    pub diagnosis: Option<String>,
    #[serde(default)]
    pub plan: Option<String>,
    #[serde(default)]
    pub medications: Vec<MedicationEntry>,
}

// =====================================================================
// Classification helper
// =====================================================================

const CLASSIFY_EXAMPLE: &str =
    r#"{ "document_type": "lab_report", "confidence": 0.92, "reason": "..." }"#;

const CLASSIFY_SYSTEM: &str = "You are a medical document classifier. \
Classify the following text as ONE of:\n\
- lab_report (blood work, urine, stool tests)\n\
- prescription (medication list with dose/frequency)\n\
- imaging_report (X-ray, MRI, CT, ultrasound, sonography)\n\
- discharge_summary (hospital discharge note)\n\
- consultation_note (doctor visit summary)\n\
- vaccination_record\n\
- invoice (bill — IGNORE later)\n\
- other\n\n\
Return JSON: { \"document_type\": \"...\", \"confidence\": 0.0-1.0, \"reason\": \"...\" }";

/// Send up to `CLASSIFY_CHARS` of text to the LLM and get a document type.
pub async fn classify_document(
    provider: &dyn LlmProvider,
    raw_text: &str,
) -> Result<ClassificationResult, String> {
    let snippet = truncate_chars(raw_text, CLASSIFY_CHARS);
    let req = JsonExtractRequest {
        system_prompt: CLASSIFY_SYSTEM.to_string(),
        user_input: snippet,
        example_json: CLASSIFY_EXAMPLE.to_string(),
        model: None,
        temperature: Some(0.0),
    };
    let resp = provider.extract_json(req).await.map_err(|e| e.to_string())?;
    let result: ClassificationResult = serde_json::from_value(resp.parsed)
        .map_err(|e| format!("classifier returned non-conforming JSON: {e}"))?;
    Ok(normalize_doc_type(result))
}

/// Map alternate spellings back to our canonical set.
fn normalize_doc_type(mut r: ClassificationResult) -> ClassificationResult {
    let t = r.document_type.to_lowercase().replace(['-', ' '], "_");
    let canonical = match t.as_str() {
        "lab_report" | "lab" | "lab_results" | "laboratory_report" => "lab_report",
        "prescription" | "rx" | "medication_list" => "prescription",
        "imaging_report" | "imaging" | "radiology" | "radiology_report" | "scan" => {
            "imaging_report"
        }
        "discharge_summary" | "discharge" => "discharge_summary",
        "consultation_note" | "consultation" | "consult" | "visit_note" | "progress_note" => {
            "consultation_note"
        }
        "vaccination_record" | "vaccination" | "immunization" | "vaccine" => {
            "vaccination_record"
        }
        "invoice" | "bill" | "receipt" => "invoice",
        _ => "other",
    };
    r.document_type = canonical.to_string();
    r
}

// =====================================================================
// Extraction helpers
// =====================================================================

const LAB_EXAMPLE: &str = r#"{
  "patient_name": "...",
  "collected_at": "YYYY-MM-DD",
  "reported_at": "YYYY-MM-DD",
  "lab_name": "...",
  "ordering_doctor": "...",
  "tests": [
    {
      "test_name": "...",
      "canonical_name": "...",
      "category": "metabolic|lipid|cbc|thyroid|liver|kidney|hormonal|other",
      "value": 123.45,
      "unit": "...",
      "reference_low": null,
      "reference_high": null,
      "reference_text": "...",
      "flag": "normal|high|low|critical"
    }
  ]
}"#;

const LAB_SYSTEM: &str = "Extract all lab test values from this medical lab report. \
Return JSON matching the shape shown. Use null for missing fields. \
Do NOT invent values. If a reference range is printed as text only, put it in \
`reference_text` and leave the numeric fields null. `flag` should be one of \
normal|high|low|critical based on the printed flag or reference range.";

const RX_EXAMPLE: &str = r#"{
  "patient_name": "...",
  "prescribed_at": "YYYY-MM-DD",
  "doctor_name": "...",
  "facility_name": "...",
  "medications": [
    {
      "name": "...",
      "generic_name": "...",
      "dose": "500 mg",
      "frequency": "twice daily",
      "route": "oral",
      "duration": "7 days",
      "indication": "...",
      "notes": "..."
    }
  ],
  "diagnosis": "...",
  "notes": "..."
}"#;

const RX_SYSTEM: &str = "Extract every medication from this prescription. \
For each drug capture the brand/name as written, its generic name if stated, \
dose, frequency, route, duration, and any indication or notes. \
Return JSON in the shape shown. Use null for missing fields. Do NOT invent \
drugs, doses, or durations.";

const IMG_EXAMPLE: &str = r#"{
  "patient_name": "...",
  "study_date": "YYYY-MM-DD",
  "modality": "X-ray|CT|MRI|Ultrasound|Mammogram|Other",
  "body_part": "...",
  "radiologist": "...",
  "facility_name": "...",
  "findings": "...",
  "impression": "...",
  "recommendations": "..."
}"#;

const IMG_SYSTEM: &str = "Extract the key fields from this imaging / radiology report. \
Return JSON in the shape shown. Preserve the full `findings` and `impression` \
sections verbatim (do not summarize). Use null for anything missing.";

const DIS_EXAMPLE: &str = r#"{
  "patient_name": "...",
  "admission_date": "YYYY-MM-DD",
  "discharge_date": "YYYY-MM-DD",
  "hospital_name": "...",
  "attending_doctor": "...",
  "primary_diagnosis": "...",
  "secondary_diagnoses": ["..."],
  "procedures": ["..."],
  "medications": [
    {
      "name": "...",
      "generic_name": "...",
      "dose": "...",
      "frequency": "...",
      "route": "...",
      "duration": "...",
      "indication": "...",
      "notes": "..."
    }
  ],
  "follow_up": "...",
  "summary": "..."
}"#;

const DIS_SYSTEM: &str = "Extract the structured fields from this hospital \
discharge summary. Return JSON in the shape shown. Preserve diagnoses and \
procedures as separate array entries. Use null or [] for missing fields. \
Do NOT invent diagnoses or procedures.";

const CON_EXAMPLE: &str = r#"{
  "patient_name": "...",
  "visit_date": "YYYY-MM-DD",
  "doctor_name": "...",
  "facility_name": "...",
  "chief_complaint": "...",
  "history": "...",
  "examination": "...",
  "diagnosis": "...",
  "plan": "...",
  "medications": [
    {
      "name": "...",
      "generic_name": "...",
      "dose": "...",
      "frequency": "...",
      "route": "...",
      "duration": "...",
      "indication": "...",
      "notes": "..."
    }
  ]
}"#;

const CON_SYSTEM: &str = "Extract the structured fields from this doctor \
consultation / visit note. Return JSON in the shape shown. Use null or [] \
for anything missing. Do NOT invent findings.";

/// Helper: run one extraction prompt and deserialize the response.
async fn run_extract<T: for<'de> Deserialize<'de>>(
    provider: &dyn LlmProvider,
    raw_text: &str,
    system: &str,
    example: &str,
) -> Result<T, String> {
    let snippet = truncate_chars(raw_text, EXTRACT_CHARS);
    let req = JsonExtractRequest {
        system_prompt: system.to_string(),
        user_input: snippet,
        example_json: example.to_string(),
        model: None,
        temperature: Some(0.0),
    };
    let resp = provider.extract_json(req).await.map_err(|e| e.to_string())?;
    serde_json::from_value::<T>(resp.parsed)
        .map_err(|e| format!("extractor returned non-conforming JSON: {e}"))
}

pub async fn extract_lab_report(
    provider: &dyn LlmProvider,
    raw_text: &str,
) -> Result<LabReportExtract, String> {
    run_extract(provider, raw_text, LAB_SYSTEM, LAB_EXAMPLE).await
}

pub async fn extract_prescription(
    provider: &dyn LlmProvider,
    raw_text: &str,
) -> Result<PrescriptionExtract, String> {
    run_extract(provider, raw_text, RX_SYSTEM, RX_EXAMPLE).await
}

pub async fn extract_imaging(
    provider: &dyn LlmProvider,
    raw_text: &str,
) -> Result<ImagingExtract, String> {
    run_extract(provider, raw_text, IMG_SYSTEM, IMG_EXAMPLE).await
}

pub async fn extract_discharge_summary(
    provider: &dyn LlmProvider,
    raw_text: &str,
) -> Result<DischargeExtract, String> {
    run_extract(provider, raw_text, DIS_SYSTEM, DIS_EXAMPLE).await
}

pub async fn extract_consultation(
    provider: &dyn LlmProvider,
    raw_text: &str,
) -> Result<ConsultationExtract, String> {
    run_extract(provider, raw_text, CON_SYSTEM, CON_EXAMPLE).await
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

// =====================================================================
// Endpoint lookup
// =====================================================================

/// Stored endpoint row (used for provider construction).
#[derive(Debug, Clone)]
struct StoredEndpoint {
    #[allow(dead_code)]
    id: String,
    provider_type: String,
    base_url: String,
    api_key: Option<String>,
    default_model: Option<String>,
    extra_headers: Option<String>,
}

/// Resolve the endpoint to use for the `health_extract` feature:
/// 1. Prefer the binding in `llm_feature_bindings`.
/// 2. Otherwise, fall back to the first enabled row in `llm_endpoints`.
fn get_extract_endpoint(
    conn: &rusqlite::Connection,
    feature: &str,
) -> Option<StoredEndpoint> {
    // Try feature binding first.
    let bound: Option<String> = conn
        .query_row(
            "SELECT endpoint_id FROM llm_feature_bindings WHERE feature = ?1",
            rusqlite::params![feature],
            |row| row.get(0),
        )
        .ok()
        .flatten();
    let query_by_id = |id: &str| -> Option<StoredEndpoint> {
        conn.query_row(
            "SELECT id, provider_type, base_url, api_key_encrypted,
                    default_model, extra_headers
             FROM llm_endpoints WHERE id = ?1 AND enabled = 1",
            rusqlite::params![id],
            |row| {
                Ok(StoredEndpoint {
                    id: row.get(0)?,
                    provider_type: row.get(1)?,
                    base_url: row.get(2)?,
                    api_key: row.get(3)?,
                    default_model: row.get(4)?,
                    extra_headers: row.get(5)?,
                })
            },
        )
        .ok()
    };
    if let Some(id) = bound {
        if let Some(ep) = query_by_id(&id) {
            return Some(ep);
        }
    }
    // Fallback: first enabled endpoint.
    conn.query_row(
        "SELECT id, provider_type, base_url, api_key_encrypted,
                default_model, extra_headers
         FROM llm_endpoints WHERE enabled = 1
         ORDER BY created_at ASC LIMIT 1",
        [],
        |row| {
            Ok(StoredEndpoint {
                id: row.get(0)?,
                provider_type: row.get(1)?,
                base_url: row.get(2)?,
                api_key: row.get(3)?,
                default_model: row.get(4)?,
                extra_headers: row.get(5)?,
            })
        },
    )
    .ok()
}

/// Convert a stored endpoint row into an [`EndpointConfig`] + model label.
fn build_config(stored: &StoredEndpoint) -> Result<EndpointConfig, String> {
    let pt = match stored.provider_type.as_str() {
        "ollama" => ProviderType::Ollama,
        "openai_compatible" => ProviderType::OpenaiCompatible,
        "openai" => ProviderType::Openai,
        "anthropic" => ProviderType::Anthropic,
        "google_gemini" => ProviderType::GoogleGemini,
        "airllm" => ProviderType::Airllm,
        other => return Err(format!("Unknown provider type: {}", other)),
    };
    let extra_headers = stored
        .extra_headers
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    Ok(EndpointConfig {
        provider_type: pt,
        base_url: stored.base_url.clone(),
        api_key: stored.api_key.clone(),
        default_model: stored.default_model.clone().unwrap_or_default(),
        extra_headers,
    })
}

/// Stable handle that other modules (e.g. `health_timeline`) hold onto.
pub type EndpointHandle = EndpointConfig;

/// Look up the endpoint bound to `feature` (or any enabled fallback) and
/// return an [`EndpointConfig`] ready for `create_provider`. Returns `None`
/// when no endpoint exists, which lets callers decide whether to error or
/// degrade silently.
pub async fn classify_endpoint_for_feature(
    state: &AppStateHandle,
    feature: &str,
) -> Result<Option<EndpointConfig>, String> {
    let stored = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        get_extract_endpoint(&conn, feature)
    };
    match stored {
        Some(s) => build_config(&s).map(Some),
        None => Ok(None),
    }
}

/// True if a default extraction endpoint exists and passes a health check.
/// Used by the ingestion loop to decide whether to auto-classify.
pub async fn is_extract_endpoint_healthy(state: &AppStateHandle) -> bool {
    let stored = {
        let st = state.read().await;
        match st.db.get() {
            Ok(conn) => get_extract_endpoint(&conn, "health_extract"),
            Err(_) => None,
        }
    };
    let Some(stored) = stored else { return false };
    let Ok(cfg) = build_config(&stored) else {
        return false;
    };
    let provider = create_provider(cfg);
    provider.health_check().await.unwrap_or(false)
}

// =====================================================================
// Pipeline orchestration
// =====================================================================

/// Classify + extract a single file end-to-end. Updates
/// `document_extractions` and `file_manifest.status` in place.
///
/// Requires an LLM endpoint (looked up via feature binding, then fallback).
pub async fn process_document(
    state: &AppStateHandle,
    file_id: &str,
    raw_text: &str,
    feature: Option<&str>,
) -> Result<(), String> {
    let feature = feature.unwrap_or("health_extract");

    // 1. Resolve endpoint + build provider.
    let stored = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        get_extract_endpoint(&conn, feature)
            .ok_or_else(|| "no LLM endpoint configured".to_string())?
    };
    let cfg = build_config(&stored)?;
    let model_label = cfg.default_model.clone();
    let provider = create_provider(cfg);

    // 2. Classify.
    let classification = classify_document(&*provider, raw_text).await?;
    tracing::info!(
        "classified file {} as {} (confidence {:.2})",
        file_id,
        classification.document_type,
        classification.confidence
    );

    // 3. If invoice/other/vaccination_record, short-circuit — no extractor yet.
    let doc_type = classification.document_type.as_str();
    let structured: Option<serde_json::Value> = match doc_type {
        "lab_report" => Some(
            serde_json::to_value(extract_lab_report(&*provider, raw_text).await?)
                .map_err(|e| e.to_string())?,
        ),
        "prescription" => Some(
            serde_json::to_value(extract_prescription(&*provider, raw_text).await?)
                .map_err(|e| e.to_string())?,
        ),
        "imaging_report" => Some(
            serde_json::to_value(extract_imaging(&*provider, raw_text).await?)
                .map_err(|e| e.to_string())?,
        ),
        "discharge_summary" => Some(
            serde_json::to_value(extract_discharge_summary(&*provider, raw_text).await?)
                .map_err(|e| e.to_string())?,
        ),
        "consultation_note" => Some(
            serde_json::to_value(extract_consultation(&*provider, raw_text).await?)
                .map_err(|e| e.to_string())?,
        ),
        // invoice | vaccination_record | other — just record classification.
        _ => None,
    };

    // 4. Persist classification + extraction JSON.
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let extracted_json = structured
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "null".into()));
    conn.execute(
        "UPDATE document_extractions
         SET document_type = ?1,
             classification_confidence = ?2,
             extracted_json = ?3,
             extraction_model = ?4
         WHERE file_id = ?5",
        rusqlite::params![
            classification.document_type,
            classification.confidence,
            extracted_json,
            model_label,
            file_id,
        ],
    )
    .map_err(|e| e.to_string())?;

    let new_status = match doc_type {
        "invoice" | "other" => "completed",
        _ if structured.is_some() => "extracted_pending_review",
        _ => "extracted",
    };
    conn.execute(
        "UPDATE file_manifest SET status = ?1 WHERE id = ?2",
        rusqlite::params![new_status, file_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

// =====================================================================
// Tauri commands
// =====================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ClassifyBatchResult {
    pub processed: i64,
    pub succeeded: i64,
    pub failed: i64,
    pub skipped: i64,
}

#[tauri::command]
pub async fn health_classify_pending(
    state: State<'_, AppStateHandle>,
    app: tauri::AppHandle,
    feature: Option<String>,
) -> Result<ClassifyBatchResult, String> {
    let feature = feature.unwrap_or_else(|| "health_extract".to_string());

    // Collect all pending extractions (file_id + raw_text).
    let pending: Vec<(String, String, String)> = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT de.id, de.file_id, COALESCE(de.raw_text, '')
                 FROM document_extractions de
                 WHERE de.document_type IS NULL AND de.raw_text IS NOT NULL",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        out
    };

    let total = pending.len() as i64;
    let mut processed = 0i64;
    let mut succeeded = 0i64;
    let mut failed = 0i64;
    let mut skipped = 0i64;

    for (_extraction_id, file_id, raw_text) in pending {
        if raw_text.trim().is_empty() {
            skipped += 1;
            processed += 1;
            continue;
        }
        match process_document(&state, &file_id, &raw_text, Some(&feature)).await {
            Ok(()) => succeeded += 1,
            Err(e) => {
                failed += 1;
                tracing::warn!("classify failed for file {}: {}", file_id, e);
            }
        }
        processed += 1;
        let _ = app.emit(
            "health-classify-progress",
            serde_json::json!({
                "processed": processed,
                "total": total,
                "succeeded": succeeded,
                "failed": failed,
                "skipped": skipped,
                "current_file_id": file_id,
            }),
        );
    }

    Ok(ClassifyBatchResult {
        processed,
        succeeded,
        failed,
        skipped,
    })
}

// ---------------------------------------------------------------------
// Pending review listing
// ---------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct PendingReview {
    pub extraction_id: String,
    pub file_id: String,
    pub original_path: String,
    pub mime_type: Option<String>,
    pub document_type: Option<String>,
    pub confidence: Option<f64>,
    pub raw_text: String,
    pub extracted_json: serde_json::Value,
}

#[tauri::command]
pub async fn health_list_pending_review(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<PendingReview>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT de.id, de.file_id, fm.original_path, fm.mime_type,
                    de.document_type, de.classification_confidence,
                    COALESCE(de.raw_text, ''), de.extracted_json
             FROM document_extractions de
             JOIN file_manifest fm ON fm.id = de.file_id
             WHERE fm.patient_id = ?1
               AND de.user_reviewed = 0
               AND de.extracted_json IS NOT NULL
             ORDER BY de.extracted_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![patient_id], |row| {
            let raw_text: String = row.get(6)?;
            let truncated: String = raw_text.chars().take(1000).collect();
            let extracted_str: Option<String> = row.get(7)?;
            let extracted_json: serde_json::Value = extracted_str
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(serde_json::Value::Null);
            Ok(PendingReview {
                extraction_id: row.get(0)?,
                file_id: row.get(1)?,
                original_path: row.get(2)?,
                mime_type: row.get(3)?,
                document_type: row.get(4)?,
                confidence: row.get(5)?,
                raw_text: truncated,
                extracted_json,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------
// Save review
// ---------------------------------------------------------------------

#[tauri::command]
pub async fn health_save_review(
    state: State<'_, AppStateHandle>,
    extraction_id: String,
    corrections: serde_json::Value,
    accept: bool,
) -> Result<(), String> {
    // Gather everything we need: patient_id, file_id, document_type.
    let (file_id, patient_id, document_type): (String, Option<String>, Option<String>) = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT de.file_id, fm.patient_id, de.document_type
             FROM document_extractions de
             JOIN file_manifest fm ON fm.id = de.file_id
             WHERE de.id = ?1",
            rusqlite::params![extraction_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| e.to_string())?
    };

    if !accept {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE document_extractions
             SET user_reviewed = 1, user_corrections = NULL
             WHERE id = ?1",
            rusqlite::params![extraction_id],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE file_manifest SET status = 'rejected' WHERE id = ?1",
            rusqlite::params![file_id],
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Accept path — persist the corrections blob and fan out into tables.
    {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        let encoded =
            serde_json::to_string(&corrections).unwrap_or_else(|_| "null".into());
        conn.execute(
            "UPDATE document_extractions
             SET user_reviewed = 1, user_corrections = ?1
             WHERE id = ?2",
            rusqlite::params![encoded, extraction_id],
        )
        .map_err(|e| e.to_string())?;
    }

    let patient_id = patient_id.ok_or_else(|| {
        "document is not associated with a patient; cannot persist entities".to_string()
    })?;
    let doc_type = document_type.unwrap_or_default();

    match doc_type.as_str() {
        "lab_report" => persist_lab_report(state.inner(), &patient_id, &file_id, &corrections).await?,
        "prescription" => {
            persist_prescription(state.inner(), &patient_id, &file_id, &corrections).await?
        }
        "imaging_report" => {
            persist_imaging(state.inner(), &patient_id, &file_id, &corrections).await?
        }
        "discharge_summary" => {
            persist_discharge(state.inner(), &patient_id, &file_id, &corrections).await?
        }
        "consultation_note" => {
            persist_consultation(state.inner(), &patient_id, &file_id, &corrections).await?
        }
        _ => {
            tracing::info!(
                "review accepted for doc_type '{}' — nothing to fan out",
                doc_type
            );
        }
    }

    // Mark manifest completed.
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE file_manifest SET status = 'completed' WHERE id = ?1",
        rusqlite::params![file_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

// =====================================================================
// Entity fan-out (accept path)
// =====================================================================

/// Shared helper: create a `medical_records` row that carries the link to
/// the file, doctor, facility, and lab. Returns the new record id.
#[allow(clippy::too_many_arguments)]
async fn create_medical_record(
    state: &AppStateHandle,
    patient_id: &str,
    file_id: &str,
    record_type: &str,
    title: &str,
    description: Option<&str>,
    date: &str,
    doctor_id: Option<&str>,
    facility_id: Option<&str>,
) -> Result<String, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO medical_records (id, patient_id, record_type, title, description,
         doctor_id, facility_id, date, document_file_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            id,
            patient_id,
            record_type,
            title,
            description,
            doctor_id,
            facility_id,
            date,
            file_id,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

fn j_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key).and_then(|x| x.as_str()).map(|s| s.to_string())
}
fn j_f64(v: &serde_json::Value, key: &str) -> Option<f64> {
    v.get(key).and_then(|x| x.as_f64())
}
fn j_array<'a>(v: &'a serde_json::Value, key: &str) -> &'a [serde_json::Value] {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

async fn resolve_opt(
    state: &AppStateHandle,
    entity_type: &str,
    name: Option<&str>,
) -> Option<String> {
    let n = name?.trim();
    if n.is_empty() {
        return None;
    }
    match resolve_entity(state, entity_type, n, 0.85).await {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::warn!("resolve_entity({entity_type}, {n}) failed: {e}");
            None
        }
    }
}

async fn persist_lab_report(
    state: &AppStateHandle,
    patient_id: &str,
    file_id: &str,
    data: &serde_json::Value,
) -> Result<(), String> {
    let lab_name = j_str(data, "lab_name");
    let doctor_name = j_str(data, "ordering_doctor");
    let collected_at = j_str(data, "collected_at")
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let reported_at = j_str(data, "reported_at");

    let doctor_id = resolve_opt(state, "doctor", doctor_name.as_deref()).await;
    let lab_id = resolve_opt(state, "lab", lab_name.as_deref()).await;

    let record_id = create_medical_record(
        state,
        patient_id,
        file_id,
        "lab_report",
        &format!(
            "Lab Report — {}",
            lab_name.as_deref().unwrap_or("unknown lab")
        ),
        None,
        &collected_at,
        doctor_id.as_deref(),
        lab_id.as_deref(),
    )
    .await?;

    let tests = j_array(data, "tests");
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    for t in tests {
        let raw_name = match j_str(t, "test_name") {
            Some(n) if !n.trim().is_empty() => n,
            _ => continue,
        };
        let Some(value) = j_f64(t, "value") else {
            continue; // skip tests with no numeric value
        };
        let canonical = j_str(t, "canonical_name")
            .map(|s| canonicalize_test(&s))
            .unwrap_or_else(|| canonicalize_test(&raw_name));
        let category = j_str(t, "category");
        let unit = j_str(t, "unit");
        let reference_low = j_f64(t, "reference_low");
        let reference_high = j_f64(t, "reference_high");
        let reference_text = j_str(t, "reference_text");
        let flag = j_str(t, "flag");

        let lab_test_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO lab_tests (id, patient_id, record_id, test_name,
             canonical_name, test_category, value, unit, reference_low,
             reference_high, reference_text, flag, lab_entity_id, collected_at,
             reported_at, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'ingested')",
            rusqlite::params![
                lab_test_id,
                patient_id,
                record_id,
                raw_name,
                canonical,
                category,
                value,
                unit,
                reference_low,
                reference_high,
                reference_text,
                flag,
                lab_id,
                collected_at,
                reported_at,
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn persist_prescription(
    state: &AppStateHandle,
    patient_id: &str,
    file_id: &str,
    data: &serde_json::Value,
) -> Result<(), String> {
    let doctor_name = j_str(data, "doctor_name");
    let facility_name = j_str(data, "facility_name");
    let prescribed_at = j_str(data, "prescribed_at")
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

    let doctor_id = resolve_opt(state, "doctor", doctor_name.as_deref()).await;
    let facility_id = resolve_opt(state, "facility", facility_name.as_deref()).await;

    let record_id = create_medical_record(
        state,
        patient_id,
        file_id,
        "prescription",
        &format!(
            "Prescription — {}",
            doctor_name.as_deref().unwrap_or("unknown doctor")
        ),
        j_str(data, "diagnosis").as_deref(),
        &prescribed_at,
        doctor_id.as_deref(),
        facility_id.as_deref(),
    )
    .await?;

    let meds = j_array(data, "medications");
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    for m in meds {
        let name = match j_str(m, "name") {
            Some(n) if !n.trim().is_empty() => n,
            _ => continue,
        };
        let generic = j_str(m, "generic_name")
            .map(|g| canonicalize_drug(&g))
            .unwrap_or_else(|| canonicalize_drug(&name));
        let dose = j_str(m, "dose");
        let frequency = j_str(m, "frequency");
        let route = j_str(m, "route");
        let indication = j_str(m, "indication").or_else(|| j_str(data, "diagnosis"));
        let notes = j_str(m, "notes");
        let med_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO medications_v2 (id, patient_id, name, generic_name, dose,
             frequency, route, start_date, end_date, prescribing_doctor_id,
             indication, notes, record_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                med_id,
                patient_id,
                name,
                generic,
                dose,
                frequency,
                route,
                prescribed_at,
                doctor_id,
                indication,
                notes,
                record_id,
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn persist_imaging(
    state: &AppStateHandle,
    patient_id: &str,
    file_id: &str,
    data: &serde_json::Value,
) -> Result<(), String> {
    let radiologist = j_str(data, "radiologist");
    let facility = j_str(data, "facility_name");
    let study_date = j_str(data, "study_date")
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let modality = j_str(data, "modality").unwrap_or_else(|| "Imaging".into());
    let body_part = j_str(data, "body_part").unwrap_or_default();
    let impression = j_str(data, "impression");
    let findings = j_str(data, "findings");
    let description = match (&findings, &impression) {
        (Some(f), Some(i)) => Some(format!("Findings:\n{}\n\nImpression:\n{}", f, i)),
        (Some(f), None) => Some(f.clone()),
        (None, Some(i)) => Some(i.clone()),
        _ => None,
    };

    let doctor_id = resolve_opt(state, "doctor", radiologist.as_deref()).await;
    let facility_id = resolve_opt(state, "facility", facility.as_deref()).await;

    let title = if body_part.is_empty() {
        format!("{} Imaging", modality)
    } else {
        format!("{} — {}", modality, body_part)
    };
    let _ = create_medical_record(
        state,
        patient_id,
        file_id,
        "imaging",
        &title,
        description.as_deref(),
        &study_date,
        doctor_id.as_deref(),
        facility_id.as_deref(),
    )
    .await?;
    Ok(())
}

async fn persist_discharge(
    state: &AppStateHandle,
    patient_id: &str,
    file_id: &str,
    data: &serde_json::Value,
) -> Result<(), String> {
    let hospital = j_str(data, "hospital_name");
    let doctor_name = j_str(data, "attending_doctor");
    let discharge_date = j_str(data, "discharge_date")
        .or_else(|| j_str(data, "admission_date"))
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let primary = j_str(data, "primary_diagnosis").unwrap_or_default();
    let summary = j_str(data, "summary");

    let doctor_id = resolve_opt(state, "doctor", doctor_name.as_deref()).await;
    let facility_id = resolve_opt(state, "facility", hospital.as_deref()).await;

    let record_id = create_medical_record(
        state,
        patient_id,
        file_id,
        "discharge",
        &format!(
            "Discharge — {}",
            if primary.is_empty() {
                "summary".to_string()
            } else {
                primary.clone()
            }
        ),
        summary.as_deref(),
        &discharge_date,
        doctor_id.as_deref(),
        facility_id.as_deref(),
    )
    .await?;

    // Add primary diagnosis as a condition if present.
    if !primary.is_empty() {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        let cond_id = uuid::Uuid::new_v4().to_string();
        let _ = conn.execute(
            "INSERT INTO health_conditions (id, patient_id, name, diagnosed_at, notes)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                cond_id,
                patient_id,
                primary,
                discharge_date,
                "imported from discharge summary",
            ],
        );
    }

    // Fan out discharge medications.
    let meds = j_array(data, "medications");
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    for m in meds {
        let name = match j_str(m, "name") {
            Some(n) if !n.trim().is_empty() => n,
            _ => continue,
        };
        let generic = j_str(m, "generic_name")
            .map(|g| canonicalize_drug(&g))
            .unwrap_or_else(|| canonicalize_drug(&name));
        let med_id = uuid::Uuid::new_v4().to_string();
        let _ = conn.execute(
            "INSERT INTO medications_v2 (id, patient_id, name, generic_name, dose,
             frequency, route, start_date, prescribing_doctor_id, indication,
             notes, record_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                med_id,
                patient_id,
                name,
                generic,
                j_str(m, "dose"),
                j_str(m, "frequency"),
                j_str(m, "route"),
                discharge_date,
                doctor_id,
                j_str(m, "indication"),
                j_str(m, "notes"),
                record_id,
            ],
        );
    }
    Ok(())
}

async fn persist_consultation(
    state: &AppStateHandle,
    patient_id: &str,
    file_id: &str,
    data: &serde_json::Value,
) -> Result<(), String> {
    let doctor_name = j_str(data, "doctor_name");
    let facility = j_str(data, "facility_name");
    let visit_date = j_str(data, "visit_date")
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let complaint = j_str(data, "chief_complaint").unwrap_or_default();
    let diagnosis = j_str(data, "diagnosis");
    let plan = j_str(data, "plan");
    let mut description = String::new();
    if let Some(d) = &diagnosis {
        description.push_str(&format!("Diagnosis: {}\n", d));
    }
    if let Some(p) = &plan {
        description.push_str(&format!("Plan: {}", p));
    }

    let doctor_id = resolve_opt(state, "doctor", doctor_name.as_deref()).await;
    let facility_id = resolve_opt(state, "facility", facility.as_deref()).await;

    let record_id = create_medical_record(
        state,
        patient_id,
        file_id,
        "consultation",
        &format!(
            "Consultation — {}",
            if complaint.is_empty() {
                "visit".to_string()
            } else {
                complaint
            }
        ),
        if description.is_empty() {
            None
        } else {
            Some(&description)
        },
        &visit_date,
        doctor_id.as_deref(),
        facility_id.as_deref(),
    )
    .await?;

    let meds = j_array(data, "medications");
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    for m in meds {
        let name = match j_str(m, "name") {
            Some(n) if !n.trim().is_empty() => n,
            _ => continue,
        };
        let generic = j_str(m, "generic_name")
            .map(|g| canonicalize_drug(&g))
            .unwrap_or_else(|| canonicalize_drug(&name));
        let med_id = uuid::Uuid::new_v4().to_string();
        let _ = conn.execute(
            "INSERT INTO medications_v2 (id, patient_id, name, generic_name, dose,
             frequency, route, start_date, prescribing_doctor_id, indication,
             notes, record_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                med_id,
                patient_id,
                name,
                generic,
                j_str(m, "dose"),
                j_str(m, "frequency"),
                j_str(m, "route"),
                visit_date,
                doctor_id,
                j_str(m, "indication").or_else(|| diagnosis.clone()),
                j_str(m, "notes"),
                record_id,
            ],
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_doc_type_maps_aliases() {
        let mk = |t: &str| ClassificationResult {
            document_type: t.to_string(),
            confidence: 0.9,
            reason: "".into(),
        };
        assert_eq!(normalize_doc_type(mk("Lab-Report")).document_type, "lab_report");
        assert_eq!(normalize_doc_type(mk("rx")).document_type, "prescription");
        assert_eq!(
            normalize_doc_type(mk("Radiology Report")).document_type,
            "imaging_report"
        );
        assert_eq!(
            normalize_doc_type(mk("visit note")).document_type,
            "consultation_note"
        );
        assert_eq!(normalize_doc_type(mk("bill")).document_type, "invoice");
        assert_eq!(normalize_doc_type(mk("garbage")).document_type, "other");
    }

    #[test]
    fn truncate_limits_characters() {
        let s: String = "a".repeat(5000);
        assert_eq!(truncate_chars(&s, 100).len(), 100);
        assert_eq!(truncate_chars("abc", 100), "abc");
    }
}
