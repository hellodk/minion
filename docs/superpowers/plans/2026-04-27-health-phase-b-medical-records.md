# Health Intelligence Phase B — Medical Records Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build structured extraction of prescriptions and lab results from ingested health documents. An LLM parses raw text extracted in Phase A (ingestion) into typed Rust structs; the user reviews the preview and confirms before anything is saved. Adds two new DB tables per document class, a new Tauri command module (`health_extract.rs`), a new frontend tab (`StructuredRecordsTab`), and an "Extract →" button on each DocumentsTab row.

**Architecture:**

- `crates/minion-db/src/migrations.rs` — migration 019 adds `prescriptions`, `prescription_items`, `structured_lab_results`, `structured_lab_values`
- `src-tauri/src/health_ingestion.rs` — HEIC support via ImageMagick `magick convert`
- `src-tauri/src/health_extract.rs` — 8 new `#[tauri::command]` functions: extract preview, confirm prescription, confirm lab result, list/delete each type, lab trends
- `src-tauri/src/lib.rs` — `mod health_extract;` + 8 handler registrations
- `ui/src/pages/health/StructuredRecordsTab.tsx` — new component showing Prescription cards + Lab Result cards with expandable item tables, colour-coded flags, delete
- `ui/src/pages/Health.tsx` — new `'structured_records'` tab wired to `<StructuredRecordsTab>`, "Extract →" button added to DocumentsTab rows

**Data flow:** `health_list_files` → user clicks "Extract →" → `health_extract_document` (LLM, no DB write) → frontend shows preview modal → user clicks "Confirm" → `health_confirm_prescription` or `health_confirm_lab_result` (DB write) → `StructuredRecordsTab` reloads.

---

## File Map

| Action | Path | Responsibility |
|---|---|---|
| Modify | `crates/minion-db/src/migrations.rs` | Add migration 019, update count assertion 18 → 19 |
| Modify | `src-tauri/src/health_ingestion.rs` | Add HEIC arm to `extract_text_from_file` |
| Create | `src-tauri/src/health_extract.rs` | Serde types + 8 Tauri commands |
| Modify | `src-tauri/src/lib.rs` | `mod health_extract;` + register 8 commands |
| Create | `ui/src/pages/health/StructuredRecordsTab.tsx` | Prescriptions + Lab Results UI |
| Modify | `ui/src/pages/Health.tsx` | Add `'structured_records'` tab + wire component + "Extract →" button |

---

## Task 1: DB Migration 019

**Files:**
- Modify: `crates/minion-db/src/migrations.rs`

Adds four new tables for structured extraction output and updates the `test_migrations_are_recorded` count assertion from 18 to 19.

- [ ] **Step 1: Verify the baseline test passes**

```bash
cargo test -p minion-db -- test_migrations_are_recorded 2>&1 | tail -5
```

Expected: `test test_migrations_are_recorded ... ok`

- [ ] **Step 2: Add `("019_health_extract", migrate_019_health_extract)` to the MIGRATIONS array**

In `crates/minion-db/src/migrations.rs`, find the line:

```rust
        ("018_blog_llm", migrate_018_blog_llm),
    ];
```

Replace with:

```rust
        ("018_blog_llm", migrate_018_blog_llm),
        ("019_health_extract", migrate_019_health_extract),
    ];
```

- [ ] **Step 3: Add the migration function**

Immediately after the closing `}` of `migrate_018_blog_llm` (around line 1200), insert:

```rust
/// Health Phase B: structured extraction output tables.
/// Prescriptions + items extracted from prescription documents.
/// Structured lab results + individual test values extracted from lab reports.
fn migrate_019_health_extract(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS prescriptions (
            id TEXT PRIMARY KEY,
            patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            source_file_id TEXT REFERENCES health_ingestion_files(id) ON DELETE SET NULL,
            prescribed_date TEXT NOT NULL,
            prescriber_name TEXT,
            prescriber_specialty TEXT,
            facility_name TEXT,
            location_city TEXT,
            diagnosis_text TEXT,
            raw_text TEXT,
            confirmed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS prescription_items (
            id TEXT PRIMARY KEY,
            prescription_id TEXT NOT NULL REFERENCES prescriptions(id) ON DELETE CASCADE,
            drug_name TEXT NOT NULL,
            dosage TEXT,
            frequency TEXT,
            duration_days INTEGER,
            instructions TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS structured_lab_results (
            id TEXT PRIMARY KEY,
            patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            source_file_id TEXT REFERENCES health_ingestion_files(id) ON DELETE SET NULL,
            lab_name TEXT,
            report_date TEXT NOT NULL,
            location_city TEXT,
            confirmed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS structured_lab_values (
            id TEXT PRIMARY KEY,
            result_id TEXT NOT NULL REFERENCES structured_lab_results(id) ON DELETE CASCADE,
            test_name TEXT NOT NULL,
            value_text TEXT NOT NULL,
            value_numeric REAL,
            unit TEXT,
            reference_low REAL,
            reference_high REAL,
            flag TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_prescriptions_patient ON prescriptions(patient_id);
        CREATE INDEX IF NOT EXISTS idx_lab_results_patient ON structured_lab_results(patient_id);
        CREATE INDEX IF NOT EXISTS idx_lab_values_result ON structured_lab_values(result_id);
        ",
    )?;
    Ok(())
}
```

- [ ] **Step 4: Update count assertion from 18 to 19**

In the `test_migrations_are_recorded` test, find:

```rust
        assert_eq!(count, 18);
```

Replace with:

```rust
        assert_eq!(count, 19);
```

- [ ] **Step 5: Add a schema smoke-test for migration 019**

In the `#[cfg(test)]` block, after `test_migration_018_blog_llm_schema`, add:

```rust
    #[test]
    fn test_migration_019_health_extract_schema() {
        let conn = setup_test_db();
        run(&conn).expect("migrations failed");

        for table in &[
            "prescriptions",
            "prescription_items",
            "structured_lab_results",
            "structured_lab_values",
        ] {
            let exists: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
                    [table],
                    |r| r.get(0),
                )
                .unwrap_or(false);
            assert!(exists, "table {} missing after migration 019", table);
        }

        // Verify FK cascade: insert patient → prescription → item, then delete patient
        conn.execute(
            "INSERT INTO patients (id, full_name, date_of_birth, gender, created_at)
             VALUES ('p1', 'Test Patient', '1990-01-01', 'other', '2026-01-01')",
            [],
        )
        .expect("insert patient failed");
        conn.execute(
            "INSERT INTO prescriptions
             (id, patient_id, prescribed_date)
             VALUES ('rx1', 'p1', '2026-01-01')",
            [],
        )
        .expect("insert prescription failed");
        conn.execute(
            "INSERT INTO prescription_items
             (id, prescription_id, drug_name)
             VALUES ('ri1', 'rx1', 'TestDrug')",
            [],
        )
        .expect("insert prescription_item failed");
        conn.execute("DELETE FROM patients WHERE id = 'p1'", [])
            .expect("delete patient failed");
        let rx_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM prescriptions WHERE id = 'rx1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rx_count, 0, "prescription should cascade on patient delete");
    }
```

- [ ] **Step 6: Run the tests**

```bash
cargo test -p minion-db 2>&1 | tail -10
```

Expected: all DB tests pass including `test_migrations_are_recorded` (count=19) and `test_migration_019_health_extract_schema`.

**Commit message:** `feat(db): migration 019 — prescriptions, prescription_items, structured_lab_results, structured_lab_values`

---

## Task 2: HEIC Support in health_ingestion.rs

**Files:**
- Modify: `src-tauri/src/health_ingestion.rs`

The `extract_text_from_file` match block currently falls through to the `_ =>` error arm for `.heic` files, even though `discover_files` already collects them. This task adds a `"heic"` arm that shell-outs to `magick convert` (ImageMagick 7), OCRs the converted JPEG, and cleans up the temp file.

- [ ] **Step 1: Verify HEIC currently returns an error**

The existing match at line ~143 ends with:

```rust
            _ => Err(format!("Unsupported extension: {}", ext)),
```

Confirm `"heic"` falls through to this arm by reading lines 143–158.

- [ ] **Step 2: Add the HEIC arm**

In `src-tauri/src/health_ingestion.rs`, find:

```rust
            "png" | "jpg" | "jpeg" | "tiff" | "tif" | "webp" => ocr_image(&p),
            _ => Err(format!("Unsupported extension: {}", ext)),
```

Replace with:

```rust
            "png" | "jpg" | "jpeg" | "tiff" | "tif" | "webp" => ocr_image(&p),
            "heic" => {
                let jpg_path = p.with_extension("jpg");
                let output = std::process::Command::new("magick")
                    .args(["convert", p.to_str().unwrap_or(""), jpg_path.to_str().unwrap_or("")])
                    .output()
                    .map_err(|_| {
                        "HEIC files require ImageMagick: sudo apt install imagemagick".to_string()
                    })?;
                if !output.status.success() {
                    return Err(format!(
                        "HEIC conversion failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ));
                }
                let text = ocr_image(&jpg_path)?;
                let _ = std::fs::remove_file(&jpg_path);
                Ok(text)
            }
            _ => Err(format!("Unsupported extension: {}", ext)),
```

- [ ] **Step 3: Build to check for compilation errors**

```bash
cargo build -p minion-app 2>&1 | grep -E "^error" | head -20
```

Expected: no errors.

- [ ] **Step 4: Run workspace tests**

```bash
cargo test --workspace 2>&1 | tail -15
```

Expected: all tests pass.

**Commit message:** `feat(health): HEIC extraction via ImageMagick magick convert`

---

## Task 3: health_extract.rs — Serde types and LLM extraction

**Files:**
- Create: `src-tauri/src/health_extract.rs`

This is the largest task. It defines all Serde structs, the deterministic flag computation, the LLM helpers (reusing the pattern from `sysmon_analysis.rs`), and the two LLM-powered extraction commands. The confirm/list/delete/trends commands are in Task 4 to keep this task focused.

Note the critical Send-safety rule: the DB connection (`conn`) must be fully dropped before `call_llm` is awaited, since `PooledConnection` is not `Send`. Acquire endpoint data in a scoped block, then call the async LLM function outside it.

- [ ] **Step 1: Create the file with types and LLM helpers**

Create `/home/dk/Documents/git/minion/src-tauri/src/health_extract.rs` with the following content:

```rust
//! Health Phase B — structured extraction of prescriptions and lab results.
//!
//! Provides:
//!   - LLM-powered extraction preview (no DB write)
//!   - Confirm commands that persist previewed data to DB
//!   - List / delete commands for each record class
//!   - Lab trend query for a single test name

use crate::state::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;
use uuid::Uuid;

type AppStateHandle = Arc<RwLock<AppState>>;
type Conn = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

// =====================================================================
// Serde types (shared with frontend via IPC)
// =====================================================================

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
#[serde(tag = "type", content = "data")]
pub enum ExtractionPreview {
    Prescription(PrescriptionExtraction),
    Lab(LabResultExtraction),
    Unsupported { doc_type: String },
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
    pub items: Vec<PrescriptionItem>,
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
    pub values: Vec<LabValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabTrendPoint {
    pub date: String,
    pub value_numeric: f64,
    pub flag: Option<String>,
}

// =====================================================================
// Flag computation (deterministic, no LLM)
// =====================================================================

fn compute_flag(value: Option<f64>, low: Option<f64>, high: Option<f64>) -> Option<String> {
    match (value, low, high) {
        (Some(v), _, Some(h)) if v > h * 1.5 => Some("CRITICAL".to_string()),
        (Some(v), _, Some(h)) if v > h => Some("HIGH".to_string()),
        (Some(v), Some(l), _) if v < l => Some("LOW".to_string()),
        (Some(_), _, _) => Some("NORMAL".to_string()),
        _ => None,
    }
}

// =====================================================================
// LLM helpers (same pattern as sysmon_analysis.rs)
// =====================================================================

fn get_endpoint(conn: &Conn) -> Option<(String, Option<String>, String)> {
    conn.query_row(
        "SELECT base_url, api_key_encrypted, COALESCE(default_model, 'llama3') \
         FROM llm_endpoints LIMIT 1",
        [],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
            ))
        },
    )
    .ok()
}

async fn call_llm(
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    system_content: &str,
    user_content: &str,
) -> Option<String> {
    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_content},
            {"role": "user",   "content": user_content}
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
        .map_err(|e| tracing::warn!("LLM request error: {e}"))
        .ok()?;
    if !resp.status().is_success() {
        tracing::warn!("LLM non-success status: {}", resp.status());
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    json["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
}

fn extract_json_block(raw: &str) -> &str {
    // Strip optional ```json ... ``` fences the model may emit despite instructions
    let s = raw.trim();
    if let Some(inner) = s.strip_prefix("```json") {
        let inner = inner.trim_start();
        if let Some(i) = inner.rfind("```") {
            return inner[..i].trim();
        }
        return inner.trim();
    }
    if let Some(inner) = s.strip_prefix("```") {
        let inner = inner.trim_start();
        if let Some(i) = inner.rfind("```") {
            return inner[..i].trim();
        }
        return inner.trim();
    }
    s
}

// =====================================================================
// Command: extract preview (LLM, no DB write)
// =====================================================================

#[tauri::command]
pub async fn health_extract_document(
    state: State<'_, AppStateHandle>,
    file_id: String,
    patient_id: String,
) -> Result<Option<ExtractionPreview>, String> {
    // 1. Load raw_text + doc_type from DB — drop conn before async work.
    let (raw_text, doc_type, endpoint) = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;

        // raw_text is stored in the extractions table (health_ingestion_files contains
        // file metadata; the extraction row from health_classify contains raw_text).
        let row: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT e.raw_text, e.document_type
                 FROM health_ingestion_files f
                 JOIN health_extractions e ON e.file_id = f.id
                 WHERE f.id = ?1 AND f.patient_id = ?2
                 LIMIT 1",
                params![file_id, patient_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
            )
            .ok();

        let (raw_text, doc_type) = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        if raw_text.trim().is_empty() {
            return Ok(None);
        }

        let ep = get_endpoint(&conn);
        (raw_text, doc_type.unwrap_or_else(|| "unknown".to_string()), ep)
        // conn drops here
    };

    let (base_url, api_key, model) = match endpoint {
        Some(ep) => ep,
        None => return Err("No LLM endpoint configured. Add one in Settings → AI.".to_string()),
    };

    match doc_type.as_str() {
        "prescription" => {
            let system = "You are a medical document parser. Extract prescription data and \
                          return ONLY valid JSON with no markdown, no explanation.";
            let schema = r#"{
  "prescribed_date": "YYYY-MM-DD or null",
  "prescriber_name": "string or null",
  "prescriber_specialty": "string or null",
  "facility_name": "string or null",
  "location_city": "string or null",
  "diagnosis_text": "string or null",
  "medications": [
    {
      "drug_name": "string",
      "dosage": "string or null",
      "frequency": "string or null",
      "duration_days": "integer or null",
      "instructions": "string or null"
    }
  ]
}"#;
            let user = format!(
                "Extract prescription data from the text below into this JSON schema:\n{schema}\n\nPrescription text:\n{raw_text}"
            );

            let llm_resp = call_llm(&base_url, api_key.as_deref(), &model, system, &user)
                .await
                .ok_or_else(|| "LLM returned no response".to_string())?;

            let clean = extract_json_block(&llm_resp);
            let mut parsed: PrescriptionExtraction =
                serde_json::from_str(clean).map_err(|e| format!("JSON parse error: {e}"))?;
            parsed.raw_text = raw_text;
            Ok(Some(ExtractionPreview::Prescription(parsed)))
        }
        "lab_report" => {
            let system = "You are a medical document parser. Extract lab report data and \
                          return ONLY valid JSON with no markdown, no explanation.";
            let schema = r#"{
  "lab_name": "string or null",
  "report_date": "YYYY-MM-DD or null",
  "location_city": "string or null",
  "results": [
    {
      "test_name": "string",
      "value_text": "string",
      "value_numeric": "float or null",
      "unit": "string or null",
      "reference_low": "float or null",
      "reference_high": "float or null"
    }
  ]
}"#;
            let user = format!(
                "Extract lab report data from the text below into this JSON schema:\n{schema}\n\nLab report text:\n{raw_text}"
            );

            let llm_resp = call_llm(&base_url, api_key.as_deref(), &model, system, &user)
                .await
                .ok_or_else(|| "LLM returned no response".to_string())?;

            let clean = extract_json_block(&llm_resp);
            let mut parsed: LabResultExtraction =
                serde_json::from_str(clean).map_err(|e| format!("JSON parse error: {e}"))?;
            // Compute flags server-side (deterministic, no LLM)
            for v in &mut parsed.results {
                v.flag = compute_flag(v.value_numeric, v.reference_low, v.reference_high);
            }
            parsed.raw_text = raw_text;
            Ok(Some(ExtractionPreview::Lab(parsed)))
        }
        other => Ok(Some(ExtractionPreview::Unsupported {
            doc_type: other.to_string(),
        })),
    }
}
```

- [ ] **Step 2: Build to verify the file compiles**

```bash
cargo build -p minion-app 2>&1 | grep -E "^error" | head -20
```

Note: this will fail until `lib.rs` is updated in Task 4. The build error should only be "unused item" or "unresolved module", not type errors. If there are type errors, fix them before proceeding.

**Commit message:** (deferred — commit after Task 4 when the file compiles cleanly as part of the full module)

---

## Task 4: health_extract.rs — Confirm, List, Delete, Trends commands

**Files:**
- Modify: `src-tauri/src/health_extract.rs` (append to the file created in Task 3)
- Modify: `src-tauri/src/lib.rs`

Appends the remaining 6 commands to `health_extract.rs` and registers all 8 commands in `lib.rs`.

- [ ] **Step 1: Append confirm, list, delete, trends commands to health_extract.rs**

Append to the end of `/home/dk/Documents/git/minion/src-tauri/src/health_extract.rs`:

```rust
// =====================================================================
// Command: confirm prescription (DB write)
// =====================================================================

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
    let prescribed_date = data
        .prescribed_date
        .clone()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

    conn.execute(
        "INSERT INTO prescriptions
         (id, patient_id, source_file_id, prescribed_date, prescriber_name,
          prescriber_specialty, facility_name, location_city, diagnosis_text,
          raw_text, confirmed)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,1)",
        params![
            rx_id,
            patient_id,
            source_file_id,
            prescribed_date,
            data.prescriber_name,
            data.prescriber_specialty,
            data.facility_name,
            data.location_city,
            data.diagnosis_text,
            data.raw_text,
        ],
    )
    .map_err(|e| e.to_string())?;

    for item in &data.medications {
        let item_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO prescription_items
             (id, prescription_id, drug_name, dosage, frequency, duration_days, instructions)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                item_id,
                rx_id,
                item.drug_name,
                item.dosage,
                item.frequency,
                item.duration_days,
                item.instructions,
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(rx_id)
}

// =====================================================================
// Command: confirm lab result (DB write)
// =====================================================================

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
    let report_date = data
        .report_date
        .clone()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

    conn.execute(
        "INSERT INTO structured_lab_results
         (id, patient_id, source_file_id, lab_name, report_date, location_city, confirmed)
         VALUES (?1,?2,?3,?4,?5,?6,1)",
        params![
            result_id,
            patient_id,
            source_file_id,
            data.lab_name,
            report_date,
            data.location_city,
        ],
    )
    .map_err(|e| e.to_string())?;

    for v in &data.results {
        let val_id = Uuid::new_v4().to_string();
        let flag = compute_flag(v.value_numeric, v.reference_low, v.reference_high);
        conn.execute(
            "INSERT INTO structured_lab_values
             (id, result_id, test_name, value_text, value_numeric, unit,
              reference_low, reference_high, flag)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                val_id,
                result_id,
                v.test_name,
                v.value_text,
                v.value_numeric,
                v.unit,
                v.reference_low,
                v.reference_high,
                flag,
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(result_id)
}

// =====================================================================
// Command: list prescriptions
// =====================================================================

#[tauri::command]
pub async fn health_list_prescriptions(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<PrescriptionWithItems>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, patient_id, source_file_id, prescribed_date, prescriber_name,
                    prescriber_specialty, facility_name, location_city, diagnosis_text,
                    confirmed, created_at
             FROM prescriptions
             WHERE patient_id = ?1
             ORDER BY prescribed_date DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows: Vec<PrescriptionWithItems> = stmt
        .query_map(params![patient_id], |r| {
            Ok(PrescriptionWithItems {
                id: r.get(0)?,
                patient_id: r.get(1)?,
                source_file_id: r.get(2)?,
                prescribed_date: r.get(3)?,
                prescriber_name: r.get(4)?,
                prescriber_specialty: r.get(5)?,
                facility_name: r.get(6)?,
                location_city: r.get(7)?,
                diagnosis_text: r.get(8)?,
                confirmed: r.get::<_, i64>(9)? != 0,
                created_at: r.get(10)?,
                items: vec![],
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(rows.len());
    for mut rx in rows {
        let mut istmt = conn
            .prepare(
                "SELECT drug_name, dosage, frequency, duration_days, instructions
                 FROM prescription_items
                 WHERE prescription_id = ?1
                 ORDER BY created_at",
            )
            .map_err(|e| e.to_string())?;
        rx.items = istmt
            .query_map(params![rx.id], |r| {
                Ok(PrescriptionItem {
                    drug_name: r.get(0)?,
                    dosage: r.get(1)?,
                    frequency: r.get(2)?,
                    duration_days: r.get(3)?,
                    instructions: r.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        out.push(rx);
    }
    Ok(out)
}

// =====================================================================
// Command: list lab results
// =====================================================================

#[tauri::command]
pub async fn health_list_lab_results(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<LabResultWithValues>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, patient_id, source_file_id, lab_name, report_date,
                    location_city, confirmed, created_at
             FROM structured_lab_results
             WHERE patient_id = ?1
             ORDER BY report_date DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows: Vec<LabResultWithValues> = stmt
        .query_map(params![patient_id], |r| {
            Ok(LabResultWithValues {
                id: r.get(0)?,
                patient_id: r.get(1)?,
                source_file_id: r.get(2)?,
                lab_name: r.get(3)?,
                report_date: r.get(4)?,
                location_city: r.get(5)?,
                confirmed: r.get::<_, i64>(6)? != 0,
                created_at: r.get(7)?,
                values: vec![],
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(rows.len());
    for mut lr in rows {
        let mut vstmt = conn
            .prepare(
                "SELECT test_name, value_text, value_numeric, unit,
                        reference_low, reference_high, flag
                 FROM structured_lab_values
                 WHERE result_id = ?1
                 ORDER BY created_at",
            )
            .map_err(|e| e.to_string())?;
        lr.values = vstmt
            .query_map(params![lr.id], |r| {
                Ok(LabValue {
                    test_name: r.get(0)?,
                    value_text: r.get(1)?,
                    value_numeric: r.get(2)?,
                    unit: r.get(3)?,
                    reference_low: r.get(4)?,
                    reference_high: r.get(5)?,
                    flag: r.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        out.push(lr);
    }
    Ok(out)
}

// =====================================================================
// Command: delete prescription
// =====================================================================

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

// =====================================================================
// Command: delete lab result
// =====================================================================

#[tauri::command]
pub async fn health_delete_lab_result(
    state: State<'_, AppStateHandle>,
    id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM structured_lab_results WHERE id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
// Command: lab trends for a single test name
// =====================================================================

#[tauri::command]
pub async fn health_get_lab_trends(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    test_name: String,
) -> Result<Vec<LabTrendPoint>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT r.report_date, v.value_numeric, v.flag
             FROM structured_lab_values v
             JOIN structured_lab_results r ON r.id = v.result_id
             WHERE r.patient_id = ?1
               AND v.test_name = ?2
               AND v.value_numeric IS NOT NULL
             ORDER BY r.report_date ASC",
        )
        .map_err(|e| e.to_string())?;

    let points = stmt
        .query_map(params![patient_id, test_name], |r| {
            Ok(LabTrendPoint {
                date: r.get(0)?,
                value_numeric: r.get(1)?,
                flag: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(points)
}
```

- [ ] **Step 2: Add `mod health_extract;` to lib.rs**

In `src-tauri/src/lib.rs`, find the block of `mod` declarations (around line 5–24). After:

```rust
mod health_ingestion;
```

Add:

```rust
mod health_extract;
```

- [ ] **Step 3: Register the 8 new commands in invoke_handler**

In `src-tauri/src/lib.rs`, find the comment:

```rust
            // Health Vault Google Drive sync (week 5)
```

Just before it (after the last `health_analysis::` registration), add:

```rust
            // Health Phase B — structured extraction (prescriptions + lab results)
            health_extract::health_extract_document,
            health_extract::health_confirm_prescription,
            health_extract::health_confirm_lab_result,
            health_extract::health_list_prescriptions,
            health_extract::health_list_lab_results,
            health_extract::health_delete_prescription,
            health_extract::health_delete_lab_result,
            health_extract::health_get_lab_trends,
```

- [ ] **Step 4: Build the full workspace**

```bash
cargo build -p minion-app 2>&1 | grep -E "^error" | head -30
```

Expected: zero errors. Fix any type mismatches before continuing.

- [ ] **Step 5: Run workspace tests**

```bash
cargo test --workspace 2>&1 | tail -15
```

Expected: all tests pass.

**Commit message:** `feat(health): health_extract.rs — LLM extraction preview, confirm, list, delete, trends commands`

---

## Task 5: StructuredRecordsTab.tsx

**Files:**
- Create: `ui/src/pages/health/StructuredRecordsTab.tsx`

A SolidJS component that shows two sub-sections: Prescriptions and Lab Results. Each section loads data from the corresponding Tauri command on mount and whenever `patientId` changes. Cards are expandable. Lab values are colour-coded by flag. Deletes prompt for confirmation.

- [ ] **Step 1: Create the file**

Create `/home/dk/Documents/git/minion/ui/src/pages/health/StructuredRecordsTab.tsx` with:

```tsx
import {
  Component,
  createSignal,
  createEffect,
  For,
  Show,
  onMount,
} from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

// =====================================================================
// Types (must mirror health_extract.rs Serde structs)
// =====================================================================

interface PrescriptionItem {
  drug_name: string;
  dosage: string | null;
  frequency: string | null;
  duration_days: number | null;
  instructions: string | null;
}

interface PrescriptionWithItems {
  id: string;
  patient_id: string;
  source_file_id: string | null;
  prescribed_date: string;
  prescriber_name: string | null;
  prescriber_specialty: string | null;
  facility_name: string | null;
  location_city: string | null;
  diagnosis_text: string | null;
  confirmed: boolean;
  created_at: string;
  items: PrescriptionItem[];
}

interface LabValue {
  test_name: string;
  value_text: string;
  value_numeric: number | null;
  unit: string | null;
  reference_low: number | null;
  reference_high: number | null;
  flag: string | null;
}

interface LabResultWithValues {
  id: string;
  patient_id: string;
  source_file_id: string | null;
  lab_name: string | null;
  report_date: string;
  location_city: string | null;
  confirmed: boolean;
  created_at: string;
  values: LabValue[];
}

// =====================================================================
// Helpers
// =====================================================================

function flagClass(flag: string | null): string {
  switch (flag) {
    case 'CRITICAL':
      return 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 font-bold';
    case 'HIGH':
      return 'bg-orange-100 dark:bg-orange-900/30 text-orange-700 dark:text-orange-300';
    case 'LOW':
      return 'bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-300';
    case 'NORMAL':
      return 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300';
    default:
      return 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400';
  }
}

function countAbnormal(values: LabValue[]): number {
  return values.filter(
    (v) => v.flag === 'HIGH' || v.flag === 'LOW' || v.flag === 'CRITICAL',
  ).length;
}

// =====================================================================
// PrescriptionCard
// =====================================================================

const PrescriptionCard: Component<{
  rx: PrescriptionWithItems;
  onDelete: (id: string) => void;
}> = (props) => {
  const [expanded, setExpanded] = createSignal(false);

  return (
    <div class="card p-4 mb-3">
      <div class="flex items-start justify-between gap-2">
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-2 flex-wrap">
            <span class="font-semibold text-sm">{props.rx.prescribed_date}</span>
            <Show when={props.rx.prescriber_name}>
              <span class="text-xs text-gray-500">Dr. {props.rx.prescriber_name}</span>
            </Show>
            <Show when={props.rx.prescriber_specialty}>
              <span class="text-xs text-gray-400">({props.rx.prescriber_specialty})</span>
            </Show>
          </div>
          <div class="flex items-center gap-2 mt-1 flex-wrap">
            <Show when={props.rx.facility_name}>
              <span class="text-xs text-gray-500">{props.rx.facility_name}</span>
            </Show>
            <Show when={props.rx.location_city}>
              <span class="text-xs text-gray-400">{props.rx.location_city}</span>
            </Show>
            <span class="text-xs text-minion-600 dark:text-minion-400">
              {props.rx.items.length} medication{props.rx.items.length !== 1 ? 's' : ''}
            </span>
          </div>
          <Show when={props.rx.diagnosis_text}>
            <p class="text-xs text-gray-600 dark:text-gray-400 mt-1 italic truncate">
              {props.rx.diagnosis_text}
            </p>
          </Show>
        </div>
        <div class="flex gap-2 shrink-0">
          <button
            class="text-xs text-minion-600 hover:underline"
            onClick={() => setExpanded((v) => !v)}
          >
            {expanded() ? 'Collapse' : 'Expand'}
          </button>
          <button
            class="text-xs text-red-500 hover:underline"
            onClick={() => {
              if (confirm('Delete this prescription and all its items?')) {
                props.onDelete(props.rx.id);
              }
            }}
          >
            Delete
          </button>
        </div>
      </div>

      <Show when={expanded() && props.rx.items.length > 0}>
        <div class="mt-3 overflow-x-auto">
          <table class="w-full text-xs">
            <thead class="bg-gray-50 dark:bg-gray-800">
              <tr>
                <th class="text-left p-2">Drug</th>
                <th class="text-left p-2">Dosage</th>
                <th class="text-left p-2">Frequency</th>
                <th class="text-right p-2">Duration (days)</th>
                <th class="text-left p-2">Instructions</th>
              </tr>
            </thead>
            <tbody>
              <For each={props.rx.items}>
                {(item) => (
                  <tr class="border-t border-gray-100 dark:border-gray-800">
                    <td class="p-2 font-medium">{item.drug_name}</td>
                    <td class="p-2 text-gray-600 dark:text-gray-400">{item.dosage ?? '—'}</td>
                    <td class="p-2 text-gray-600 dark:text-gray-400">{item.frequency ?? '—'}</td>
                    <td class="p-2 text-right text-gray-600 dark:text-gray-400">
                      {item.duration_days != null ? String(item.duration_days) : '—'}
                    </td>
                    <td class="p-2 text-gray-600 dark:text-gray-400">
                      {item.instructions ?? '—'}
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </div>
      </Show>
    </div>
  );
};

// =====================================================================
// LabResultCard
// =====================================================================

const LabResultCard: Component<{
  result: LabResultWithValues;
  onDelete: (id: string) => void;
}> = (props) => {
  const [expanded, setExpanded] = createSignal(false);
  const abnormal = () => countAbnormal(props.result.values);

  return (
    <div class="card p-4 mb-3">
      <div class="flex items-start justify-between gap-2">
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-2 flex-wrap">
            <span class="font-semibold text-sm">{props.result.report_date}</span>
            <Show when={props.result.lab_name}>
              <span class="text-xs text-gray-500">{props.result.lab_name}</span>
            </Show>
            <Show when={props.result.location_city}>
              <span class="text-xs text-gray-400">{props.result.location_city}</span>
            </Show>
          </div>
          <div class="flex items-center gap-2 mt-1">
            <span class="text-xs text-gray-500">
              {props.result.values.length} test{props.result.values.length !== 1 ? 's' : ''}
            </span>
            <Show when={abnormal() > 0}>
              <span class="px-1.5 py-0.5 rounded text-xs bg-orange-100 dark:bg-orange-900/30 text-orange-700 dark:text-orange-300">
                {abnormal()} abnormal
              </span>
            </Show>
          </div>
        </div>
        <div class="flex gap-2 shrink-0">
          <button
            class="text-xs text-minion-600 hover:underline"
            onClick={() => setExpanded((v) => !v)}
          >
            {expanded() ? 'Collapse' : 'Expand'}
          </button>
          <button
            class="text-xs text-red-500 hover:underline"
            onClick={() => {
              if (confirm('Delete this lab result and all its values?')) {
                props.onDelete(props.result.id);
              }
            }}
          >
            Delete
          </button>
        </div>
      </div>

      <Show when={expanded() && props.result.values.length > 0}>
        <div class="mt-3 overflow-x-auto">
          <table class="w-full text-xs">
            <thead class="bg-gray-50 dark:bg-gray-800">
              <tr>
                <th class="text-left p-2">Test</th>
                <th class="text-right p-2">Value</th>
                <th class="text-left p-2">Unit</th>
                <th class="text-left p-2">Reference range</th>
                <th class="text-left p-2">Flag</th>
              </tr>
            </thead>
            <tbody>
              <For each={props.result.values}>
                {(v) => (
                  <tr class="border-t border-gray-100 dark:border-gray-800">
                    <td class="p-2 font-medium">{v.test_name}</td>
                    <td class="p-2 text-right font-mono">{v.value_text}</td>
                    <td class="p-2 text-gray-500">{v.unit ?? '—'}</td>
                    <td class="p-2 text-gray-500">
                      <Show
                        when={v.reference_low != null || v.reference_high != null}
                        fallback={<span>—</span>}
                      >
                        {v.reference_low != null ? String(v.reference_low) : '?'}
                        {' – '}
                        {v.reference_high != null ? String(v.reference_high) : '?'}
                      </Show>
                    </td>
                    <td class="p-2">
                      <Show when={v.flag} fallback={<span class="text-gray-400">—</span>}>
                        <span class={`px-1.5 py-0.5 rounded text-xs ${flagClass(v.flag)}`}>
                          {v.flag}
                        </span>
                      </Show>
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </div>
      </Show>
    </div>
  );
};

// =====================================================================
// StructuredRecordsTab
// =====================================================================

const StructuredRecordsTab: Component<{ patientId: string }> = (props) => {
  const [prescriptions, setPrescriptions] = createSignal<PrescriptionWithItems[]>([]);
  const [labResults, setLabResults] = createSignal<LabResultWithValues[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const [rxList, labList] = await Promise.all([
        invoke<PrescriptionWithItems[]>('health_list_prescriptions', {
          patientId: props.patientId,
        }),
        invoke<LabResultWithValues[]>('health_list_lab_results', {
          patientId: props.patientId,
        }),
      ]);
      setPrescriptions(rxList);
      setLabResults(labList);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  onMount(() => void load());

  createEffect(() => {
    if (props.patientId) void load();
  });

  const deletePrescription = async (id: string) => {
    try {
      await invoke('health_delete_prescription', { id });
      await load();
    } catch (e) {
      alert(String(e));
    }
  };

  const deleteLabResult = async (id: string) => {
    try {
      await invoke('health_delete_lab_result', { id });
      await load();
    } catch (e) {
      alert(String(e));
    }
  };

  return (
    <div>
      <div class="flex items-center justify-between mb-4">
        <div>
          <h2 class="text-lg font-semibold">Structured Records</h2>
          <p class="text-xs text-gray-500">
            Prescriptions and lab results extracted from imported documents.
          </p>
        </div>
        <button class="btn btn-secondary text-sm" onClick={load} disabled={loading()}>
          {loading() ? 'Loading…' : 'Refresh'}
        </button>
      </div>

      <Show when={error()}>
        <div class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
          <p class="text-sm text-red-700 dark:text-red-300">{error()}</p>
        </div>
      </Show>

      {/* Prescriptions */}
      <div class="mb-6">
        <h3 class="text-sm font-semibold mb-3 text-gray-700 dark:text-gray-300">
          Prescriptions ({prescriptions().length})
        </h3>
        <Show
          when={prescriptions().length > 0}
          fallback={
            <div class="card p-6 text-center text-sm text-gray-500">
              No prescriptions extracted yet. Go to Documents, click "Extract →" on a
              prescription file, then confirm the preview.
            </div>
          }
        >
          <For each={prescriptions()}>
            {(rx) => <PrescriptionCard rx={rx} onDelete={deletePrescription} />}
          </For>
        </Show>
      </div>

      {/* Lab Results */}
      <div>
        <h3 class="text-sm font-semibold mb-3 text-gray-700 dark:text-gray-300">
          Lab Results ({labResults().length})
        </h3>
        <Show
          when={labResults().length > 0}
          fallback={
            <div class="card p-6 text-center text-sm text-gray-500">
              No lab results extracted yet. Go to Documents, click "Extract →" on a lab report
              file, then confirm the preview.
            </div>
          }
        >
          <For each={labResults()}>
            {(lr) => <LabResultCard result={lr} onDelete={deleteLabResult} />}
          </For>
        </Show>
      </div>
    </div>
  );
};

export default StructuredRecordsTab;
```

- [ ] **Step 2: Run typecheck**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | tail -20
```

Expected: zero errors on the new file (other pre-existing errors, if any, are acceptable).

**Commit message:** `feat(health): StructuredRecordsTab — prescriptions and lab results with expandable cards`

---

## Task 6: Wire StructuredRecordsTab into Health.tsx

**Files:**
- Modify: `ui/src/pages/Health.tsx`

Adds the `'structured_records'` tab to the union type, the tab bar, and the tab body `<Show>` block.

- [ ] **Step 1: Import the new component**

In `ui/src/pages/Health.tsx`, find the import block near the top (around lines 3–9):

```tsx
import ImportTab from './health/ImportTab';
import ReviewTab from './health/ReviewTab';
import DocumentsTab from './health/DocumentsTab';
import TimelineTab from './health/TimelineTab';
import EpisodesTab from './health/EpisodesTab';
import AnalysisTab from './health/AnalysisTab';
import CloudBackupTab from './health/CloudBackupTab';
```

Replace with:

```tsx
import ImportTab from './health/ImportTab';
import ReviewTab from './health/ReviewTab';
import DocumentsTab from './health/DocumentsTab';
import StructuredRecordsTab from './health/StructuredRecordsTab';
import TimelineTab from './health/TimelineTab';
import EpisodesTab from './health/EpisodesTab';
import AnalysisTab from './health/AnalysisTab';
import CloudBackupTab from './health/CloudBackupTab';
```

- [ ] **Step 2: Add `'structured_records'` to the HealthTab union type**

Find the `type HealthTab` declaration (around line 137):

```tsx
type HealthTab =
  | 'dashboard'
  | 'records'
  | 'labs'
  | 'medications'
  | 'conditions'
  | 'vitals'
  | 'life_events'
  | 'symptoms'
  | 'timeline'
  | 'episodes'
  | 'analysis'
  | 'family'
  | 'import'
  | 'review'
  | 'documents'
  | 'cloud_backup';
```

Replace with:

```tsx
type HealthTab =
  | 'dashboard'
  | 'records'
  | 'labs'
  | 'medications'
  | 'conditions'
  | 'vitals'
  | 'life_events'
  | 'symptoms'
  | 'timeline'
  | 'episodes'
  | 'analysis'
  | 'family'
  | 'import'
  | 'review'
  | 'documents'
  | 'structured_records'
  | 'cloud_backup';
```

- [ ] **Step 3: Add entry to the tab bar array**

Find the tab bar array (around line 788). After:

```tsx
              ['documents', 'Documents'],
```

Add:

```tsx
              ['structured_records', 'Extracted Records'],
```

- [ ] **Step 4: Add the Show block for the new tab**

Find the documents Show block (around line 977):

```tsx
        {/* ============== DOCUMENTS ============== */}
        <Show when={activeTab() === 'documents'}>
          <DocumentsTab activePatient={activePatient()!} />
        </Show>
```

After it, add:

```tsx
        {/* ============== STRUCTURED RECORDS ============== */}
        <Show when={activeTab() === 'structured_records'}>
          <StructuredRecordsTab patientId={activePatient()!.id} />
        </Show>
```

- [ ] **Step 5: Run typecheck**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | tail -20
```

Expected: zero errors.

**Commit message:** `feat(health): add Extracted Records tab to Health.tsx`

---

## Task 7: "Extract →" button in DocumentsTab

**Files:**
- Modify: `ui/src/pages/health/DocumentsTab.tsx`

Adds the extraction preview UX directly into DocumentsTab: an "Extract →" button per row (only shown for `prescription` and `lab_report` doc types), an extraction-in-progress state, a preview modal with Prescription or Lab result display, and Confirm/Cancel actions that call `health_confirm_prescription` or `health_confirm_lab_result`.

- [ ] **Step 1: Add ExtractionPreview types and signals**

In `ui/src/pages/health/DocumentsTab.tsx`, after the existing `ExtractionEntry` interface add:

```tsx
interface PrescriptionItem {
  drug_name: string;
  dosage: string | null;
  frequency: string | null;
  duration_days: number | null;
  instructions: string | null;
}

interface PrescriptionExtraction {
  prescribed_date: string | null;
  prescriber_name: string | null;
  prescriber_specialty: string | null;
  facility_name: string | null;
  location_city: string | null;
  diagnosis_text: string | null;
  medications: PrescriptionItem[];
  raw_text: string;
}

interface LabValue {
  test_name: string;
  value_text: string;
  value_numeric: number | null;
  unit: string | null;
  reference_low: number | null;
  reference_high: number | null;
  flag: string | null;
}

interface LabResultExtraction {
  lab_name: string | null;
  report_date: string | null;
  location_city: string | null;
  results: LabValue[];
  raw_text: string;
}

type ExtractionPreview =
  | { type: 'Prescription'; data: PrescriptionExtraction }
  | { type: 'Lab'; data: LabResultExtraction }
  | { type: 'Unsupported'; data: { doc_type: string } };
```

- [ ] **Step 2: Add signals and handlers inside the DocumentsTab component**

In the `DocumentsTab` component body, after the existing signals (`loading`, `error`, etc.) add:

```tsx
  const [extractingId, setExtractingId] = createSignal<string | null>(null);
  const [extractError, setExtractError] = createSignal<string | null>(null);
  const [preview, setPreview] = createSignal<ExtractionPreview | null>(null);
  const [previewFileId, setPreviewFileId] = createSignal<string | null>(null);
  const [confirming, setConfirming] = createSignal(false);

  const extractDocument = async (file: FileEntry) => {
    setExtractingId(file.id);
    setExtractError(null);
    try {
      const result = await invoke<ExtractionPreview | null>('health_extract_document', {
        fileId: file.id,
        patientId: props.activePatient.id,
      });
      if (!result) {
        setExtractError('No extraction result returned. Check that the file has been classified.');
        return;
      }
      setPreview(result);
      setPreviewFileId(file.id);
    } catch (e) {
      setExtractError(String(e));
    } finally {
      setExtractingId(null);
    }
  };

  const confirmExtraction = async () => {
    const p = preview();
    if (!p) return;
    setConfirming(true);
    try {
      if (p.type === 'Prescription') {
        await invoke('health_confirm_prescription', {
          patientId: props.activePatient.id,
          sourceFileId: previewFileId(),
          data: p.data,
        });
      } else if (p.type === 'Lab') {
        await invoke('health_confirm_lab_result', {
          patientId: props.activePatient.id,
          sourceFileId: previewFileId(),
          data: p.data,
        });
      }
      setPreview(null);
      setPreviewFileId(null);
      alert('Saved. Switch to the Extracted Records tab to view.');
    } catch (e) {
      alert(String(e));
    } finally {
      setConfirming(false);
    }
  };
```

- [ ] **Step 3: Add "Extract →" button in the table row actions**

Find the actions cell in the row (around line 306):

```tsx
                    <td class="p-2 text-right">
                      <div class="flex justify-end gap-1">
                        <button
                          class="text-xs text-minion-600 hover:underline"
                          onClick={() => setViewFile(f)}
                          title="View raw text"
                        >
                          View
                        </button>
                        <button
                          class="text-xs text-minion-600 hover:underline disabled:opacity-50"
                          onClick={() => reclassify(f)}
                          disabled={reclassifyingId() === f.id}
                          title="Re-classify this document"
                        >
                          {reclassifyingId() === f.id ? '…' : 'Re-classify'}
                        </button>
                        <button
                          class="text-xs text-red-500 hover:underline"
                          onClick={() => remove(f)}
                          title="Delete file"
                        >
                          Delete
                        </button>
                      </div>
                    </td>
```

Replace with:

```tsx
                    <td class="p-2 text-right">
                      <div class="flex justify-end gap-1">
                        <button
                          class="text-xs text-minion-600 hover:underline"
                          onClick={() => setViewFile(f)}
                          title="View raw text"
                        >
                          View
                        </button>
                        <Show
                          when={
                            ex()?.document_type === 'prescription' ||
                            ex()?.document_type === 'lab_report'
                          }
                        >
                          <button
                            class="text-xs text-emerald-600 hover:underline disabled:opacity-50"
                            onClick={() => extractDocument(f)}
                            disabled={extractingId() === f.id}
                            title="Extract structured data via LLM"
                          >
                            {extractingId() === f.id ? 'Extracting…' : 'Extract →'}
                          </button>
                        </Show>
                        <button
                          class="text-xs text-minion-600 hover:underline disabled:opacity-50"
                          onClick={() => reclassify(f)}
                          disabled={reclassifyingId() === f.id}
                          title="Re-classify this document"
                        >
                          {reclassifyingId() === f.id ? '…' : 'Re-classify'}
                        </button>
                        <button
                          class="text-xs text-red-500 hover:underline"
                          onClick={() => remove(f)}
                          title="Delete file"
                        >
                          Delete
                        </button>
                      </div>
                    </td>
```

- [ ] **Step 4: Add extraction error banner and preview modal**

After the existing `{/* View modal */}` block (after the `</Show>` that closes it, around line 383), add:

```tsx
      {/* Extraction error banner */}
      <Show when={extractError()}>
        <div class="fixed bottom-4 right-4 z-50 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg shadow-lg max-w-sm">
          <div class="flex items-start justify-between gap-2">
            <p class="text-sm text-red-700 dark:text-red-300">{extractError()}</p>
            <button
              class="text-red-500 text-xs hover:underline shrink-0"
              onClick={() => setExtractError(null)}
            >
              Dismiss
            </button>
          </div>
        </div>
      </Show>

      {/* Extraction preview modal */}
      <Show when={preview()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
          <div class="card w-full max-w-3xl max-h-[90vh] overflow-hidden shadow-2xl flex flex-col">
            <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
              <div>
                <div class="text-base font-semibold">
                  {preview()!.type === 'Prescription'
                    ? 'Prescription Preview'
                    : preview()!.type === 'Lab'
                      ? 'Lab Result Preview'
                      : 'Unsupported Document Type'}
                </div>
                <p class="text-xs text-gray-500 mt-0.5">
                  Review the extracted data before confirming. You can cancel and re-classify if
                  the extraction looks wrong.
                </p>
              </div>
              <button class="btn btn-secondary text-sm" onClick={() => setPreview(null)}>
                Cancel
              </button>
            </div>

            <div class="p-4 overflow-y-auto flex-1">
              {/* Prescription preview */}
              <Show when={preview()!.type === 'Prescription'}>
                {(() => {
                  const d = (preview() as { type: 'Prescription'; data: PrescriptionExtraction })
                    .data;
                  return (
                    <div class="space-y-3">
                      <div class="grid grid-cols-2 gap-3 text-sm">
                        <div>
                          <span class="text-xs text-gray-500 block">Date</span>
                          <span>{d.prescribed_date ?? '—'}</span>
                        </div>
                        <div>
                          <span class="text-xs text-gray-500 block">Prescriber</span>
                          <span>{d.prescriber_name ?? '—'}</span>
                        </div>
                        <div>
                          <span class="text-xs text-gray-500 block">Specialty</span>
                          <span>{d.prescriber_specialty ?? '—'}</span>
                        </div>
                        <div>
                          <span class="text-xs text-gray-500 block">Facility</span>
                          <span>{d.facility_name ?? '—'}</span>
                        </div>
                        <div>
                          <span class="text-xs text-gray-500 block">City</span>
                          <span>{d.location_city ?? '—'}</span>
                        </div>
                        <div>
                          <span class="text-xs text-gray-500 block">Diagnosis</span>
                          <span>{d.diagnosis_text ?? '—'}</span>
                        </div>
                      </div>
                      <div class="overflow-x-auto mt-2">
                        <table class="w-full text-xs">
                          <thead class="bg-gray-50 dark:bg-gray-800">
                            <tr>
                              <th class="text-left p-2">Drug</th>
                              <th class="text-left p-2">Dosage</th>
                              <th class="text-left p-2">Frequency</th>
                              <th class="text-right p-2">Days</th>
                              <th class="text-left p-2">Instructions</th>
                            </tr>
                          </thead>
                          <tbody>
                            <For each={d.medications}>
                              {(item) => (
                                <tr class="border-t border-gray-100 dark:border-gray-800">
                                  <td class="p-2 font-medium">{item.drug_name}</td>
                                  <td class="p-2">{item.dosage ?? '—'}</td>
                                  <td class="p-2">{item.frequency ?? '—'}</td>
                                  <td class="p-2 text-right">
                                    {item.duration_days != null ? String(item.duration_days) : '—'}
                                  </td>
                                  <td class="p-2">{item.instructions ?? '—'}</td>
                                </tr>
                              )}
                            </For>
                          </tbody>
                        </table>
                      </div>
                    </div>
                  );
                })()}
              </Show>

              {/* Lab result preview */}
              <Show when={preview()!.type === 'Lab'}>
                {(() => {
                  const d = (preview() as { type: 'Lab'; data: LabResultExtraction }).data;
                  return (
                    <div class="space-y-3">
                      <div class="grid grid-cols-2 gap-3 text-sm">
                        <div>
                          <span class="text-xs text-gray-500 block">Report Date</span>
                          <span>{d.report_date ?? '—'}</span>
                        </div>
                        <div>
                          <span class="text-xs text-gray-500 block">Lab Name</span>
                          <span>{d.lab_name ?? '—'}</span>
                        </div>
                        <div>
                          <span class="text-xs text-gray-500 block">City</span>
                          <span>{d.location_city ?? '—'}</span>
                        </div>
                      </div>
                      <div class="overflow-x-auto mt-2">
                        <table class="w-full text-xs">
                          <thead class="bg-gray-50 dark:bg-gray-800">
                            <tr>
                              <th class="text-left p-2">Test</th>
                              <th class="text-right p-2">Value</th>
                              <th class="text-left p-2">Unit</th>
                              <th class="text-left p-2">Reference</th>
                              <th class="text-left p-2">Flag</th>
                            </tr>
                          </thead>
                          <tbody>
                            <For each={d.results}>
                              {(v) => (
                                <tr class="border-t border-gray-100 dark:border-gray-800">
                                  <td class="p-2 font-medium">{v.test_name}</td>
                                  <td class="p-2 text-right font-mono">{v.value_text}</td>
                                  <td class="p-2 text-gray-500">{v.unit ?? '—'}</td>
                                  <td class="p-2 text-gray-500">
                                    <Show
                                      when={
                                        v.reference_low != null || v.reference_high != null
                                      }
                                      fallback={<span>—</span>}
                                    >
                                      {v.reference_low != null ? String(v.reference_low) : '?'}
                                      {' – '}
                                      {v.reference_high != null
                                        ? String(v.reference_high)
                                        : '?'}
                                    </Show>
                                  </td>
                                  <td class="p-2">
                                    <Show
                                      when={v.flag}
                                      fallback={<span class="text-gray-400">—</span>}
                                    >
                                      <span
                                        class={`px-1.5 py-0.5 rounded text-xs ${
                                          v.flag === 'CRITICAL'
                                            ? 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 font-bold'
                                            : v.flag === 'HIGH'
                                              ? 'bg-orange-100 dark:bg-orange-900/30 text-orange-700 dark:text-orange-300'
                                              : v.flag === 'LOW'
                                                ? 'bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-300'
                                                : 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
                                        }`}
                                      >
                                        {v.flag}
                                      </span>
                                    </Show>
                                  </td>
                                </tr>
                              )}
                            </For>
                          </tbody>
                        </table>
                      </div>
                    </div>
                  );
                })()}
              </Show>

              {/* Unsupported */}
              <Show when={preview()!.type === 'Unsupported'}>
                <p class="text-sm text-gray-600 dark:text-gray-400">
                  This document type (
                  {(preview() as { type: 'Unsupported'; data: { doc_type: string } }).data
                    .doc_type}
                  ) is not yet supported for structured extraction. Only prescriptions and lab
                  reports can be extracted.
                </p>
              </Show>
            </div>

            <div class="p-4 border-t border-gray-200 dark:border-gray-700 flex justify-end gap-2">
              <button class="btn btn-secondary text-sm" onClick={() => setPreview(null)}>
                Cancel
              </button>
              <Show when={preview()!.type !== 'Unsupported'}>
                <button
                  class="btn btn-primary text-sm disabled:opacity-50"
                  onClick={confirmExtraction}
                  disabled={confirming()}
                >
                  {confirming() ? 'Saving…' : 'Confirm & Save'}
                </button>
              </Show>
            </div>
          </div>
        </div>
      </Show>
```

- [ ] **Step 5: Run typecheck**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | tail -20
```

Expected: zero errors.

**Commit message:** `feat(health): Extract → button in DocumentsTab with LLM preview modal and confirm flow`

---

## Task 8: Final verification

**Files:** (read-only verification, no changes)

Confirms the full stack compiles, all existing tests pass, and TypeScript has no new errors.

- [ ] **Step 1: Full Rust build**

```bash
cargo build --workspace 2>&1 | grep -E "^error" | head -20
```

Expected: zero errors.

- [ ] **Step 2: Full Rust test suite**

```bash
cargo test --workspace 2>&1 | tail -20
```

Expected: all tests pass (623+ tests). Any failures must be investigated and fixed.

- [ ] **Step 3: Clippy check**

```bash
cargo clippy --workspace -- -D warnings 2>&1 | grep -E "^error" | head -20
```

Expected: zero errors. Fix any clippy warnings before marking the task complete.

- [ ] **Step 4: Frontend typecheck**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | tail -20
```

Expected: zero errors.

- [ ] **Step 5: Frontend lint**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm lint 2>&1 | tail -20
```

Expected: zero errors. Fix any lint issues before marking complete.

- [ ] **Step 6: Final commit**

```bash
# Verify no unintentional leftover uncommitted files
git status
```

All changes from Tasks 1–7 should already be committed. If any are unstaged, commit them now.

**Commit message:** `chore(health-phase-b): final verification pass — build, tests, clippy, typecheck all green`

---

## Implementation Order

Tasks are sequentially dependent in the following order due to compilation constraints:

1. **Task 1** (DB migration) — standalone, no dependencies
2. **Task 2** (HEIC fix) — standalone, compile-checks health_ingestion.rs
3. **Task 3** (health_extract.rs types + extract command) — creates new file
4. **Task 4** (health_extract.rs confirm/list/delete + lib.rs wiring) — completes the Rust module; do NOT commit Task 3 until Task 4 passes `cargo build`
5. **Task 5** (StructuredRecordsTab) — frontend only, can be done in parallel with Tasks 3–4 if run in a separate worktree
6. **Task 6** (wire tab in Health.tsx) — depends on Task 5
7. **Task 7** (Extract → button in DocumentsTab) — depends on Task 5 types being defined (or copy the interfaces locally as shown)
8. **Task 8** (final verification) — depends on all prior tasks

## Key Decisions

- **No DB write on extract preview:** The `health_extract_document` command is intentionally read-only. The user sees the LLM output first and must click "Confirm & Save" — this avoids polluting the DB with bad extractions.
- **Flags are computed in Rust, not by LLM:** `compute_flag` is deterministic and not subject to LLM hallucination. The LLM is only asked for `reference_low`/`reference_high` numeric values; Rust derives the flag from those.
- **`health_ingestion_files` join:** The `health_extract_document` command joins `health_ingestion_files` and `health_extractions` (the table written by `health_classify`) to get `raw_text`. This is the actual table name used in the existing schema — verify via `migrations.rs` if the join fails.
- **Send safety:** The DB `PooledConnection` is `!Send`. The `endpoint` tuple is extracted in a scoped block that drops `conn` before the `call_llm` future is awaited. This matches the pattern in `sysmon_analysis.rs`.
- **JSON fence stripping:** The `extract_json_block` helper strips `` ```json `` fences that many models emit even when instructed not to, making the JSON parse more robust without increasing prompt complexity.
