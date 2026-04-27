# Health Intelligence Design

**Status:** Approved for implementation  
**Date:** 2026-04-27  
**Phases:** A (Fix Fitness) → B (Medical Records) → C (Unified Intelligence)

---

## Goal

Turn MINION into a personal health intelligence platform: real fitness data from Google Fit, structured extraction of medical documents (prescriptions, lab reports, discharge summaries), a unified chronological health timeline, anomaly detection, and AI-generated doctor-visit narratives.

---

## Current State

| Area | Status |
|---|---|
| `fitness_get_dashboard` | Queries real DB — correct |
| Sleep / Heart Rate / Activity tabs | 100% hardcoded mock constants — broken |
| Dashboard fallback | Uses mock values (8432 steps, 72 BPM) when DB empty |
| AI Scores / Recommendations | Always hardcoded — never real |
| Health Vault ingestion | Works for PDF + JPG/PNG; HEIC discovered but extraction fails |
| Health Vault structured data | Free-text only; no per-field extraction for prescriptions or lab values |

---

## Phase A — Fix Fitness Tabs

### Problem
`WEEKLY_STEPS`, `WEEKLY_SLEEP`, `SLEEP_STAGES`, `HEART_RATE_ZONES`, `WEEKLY_HEART_RATE`, `AI_HEALTH_SCORES`, `AI_RECOMMENDATIONS`, `DEFAULT_HABITS`, `DOCTOR_SUGGESTIONS` are all hardcoded constants in `Fitness.tsx`. The Sleep, Heart Rate, and Activity tabs never read from the database.

### Solution

**Backend — no new commands needed.** `fitness_get_metrics(days: 30)` already returns `FitnessMetricResponse[]` with: `date`, `steps`, `heart_rate_avg`, `heart_rate_min`, `heart_rate_max`, `sleep_hours`, `sleep_quality`, `weight_kg`, `distance_m`, `active_minutes`, `spo2_avg`, `calories_in`, `calories_out`, `source`.

**Frontend — `ui/src/pages/Fitness.tsx`:**

1. Delete all hardcoded constant blocks (`WEEKLY_STEPS`, `WEEKLY_SLEEP`, etc.)
2. Wire `SleepTab` to derive data from `metrics()` signal:
   - 7-night bar chart from last 7 `sleep_hours` values
   - Avg / min / max computed client-side
   - `sleep_quality` field (0–100, user-entered or Fit-derived) shown as a quality score badge, not stage percentages. Sleep stage breakdown (Deep/Light/REM/Awake) requires Google Fit sleep session data — not yet in `fitness_metrics`; show "Stage data not available" placeholder for now and leave stage chart for Phase C-2.
3. Wire `HeartTab` to `heart_rate_avg`, `heart_rate_min`, `heart_rate_max` per day
4. Wire `ActivityTab` to `steps`, `distance_m`, `active_minutes`, `calories_out` per day
5. Add **sync status bar** (shared component across all tabs):
   - "Last synced: N min ago" from `gfit_get_sync_status`
   - "↻ Sync now" button calling `gfit_sync`
6. Add **empty state** when no metrics exist:
   - Icon + "No data yet" message
   - "Connect Google Fit →" CTA button (navigates to Settings → Google Fit section)
   - Secondary: "Log manually in the Metrics tab"
7. `hasRealData` signal already exists — use it to decide between real chart and empty state
8. Remove `AI_HEALTH_SCORES`, `AI_RECOMMENDATIONS`, `DOCTOR_SUGGESTIONS` hardcoded blocks; replace with "Connect an AI endpoint to generate recommendations" placeholder

**Data flow:**
```
gfit_sync (every 15 min or manual) → fitness_metrics table
→ fitness_get_metrics() → Fitness.tsx signals
→ SleepTab / HeartTab / ActivityTab read from same signal
```

**Files changed:** `ui/src/pages/Fitness.tsx` only (backend already correct).

---

## Phase B — Medical Records Intelligence

### New Tables (Migration 019)

```sql
CREATE TABLE IF NOT EXISTS prescriptions (
    id              TEXT PRIMARY KEY,
    patient_id      TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
    source_file_id  TEXT REFERENCES health_ingestion_files(id) ON DELETE SET NULL,
    prescribed_date TEXT NOT NULL,
    prescriber_name TEXT,
    prescriber_specialty TEXT,
    facility_name   TEXT,
    location_city   TEXT,
    diagnosis_text  TEXT,
    raw_text        TEXT,
    confirmed       INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS prescription_items (
    id              TEXT PRIMARY KEY,
    prescription_id TEXT NOT NULL REFERENCES prescriptions(id) ON DELETE CASCADE,
    drug_name       TEXT NOT NULL,
    dosage          TEXT,
    frequency       TEXT,      -- "1-0-1", "twice daily", etc.
    duration_days   INTEGER,
    instructions    TEXT,
    created_at      TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS structured_lab_results (
    id              TEXT PRIMARY KEY,
    patient_id      TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
    source_file_id  TEXT REFERENCES health_ingestion_files(id) ON DELETE SET NULL,
    lab_name        TEXT,
    report_date     TEXT NOT NULL,
    location_city   TEXT,
    confirmed       INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS structured_lab_values (
    id              TEXT PRIMARY KEY,
    result_id       TEXT NOT NULL REFERENCES structured_lab_results(id) ON DELETE CASCADE,
    test_name       TEXT NOT NULL,
    value_text      TEXT NOT NULL,
    value_numeric   REAL,
    unit            TEXT,
    reference_low   REAL,
    reference_high  REAL,
    flag            TEXT,   -- "HIGH" | "LOW" | "CRITICAL" | "NORMAL"
    created_at      TEXT DEFAULT CURRENT_TIMESTAMP
);
```

### HEIC Fix

In `health_ingestion.rs`, add HEIC → JPEG conversion using `magick convert` (ImageMagick — same external-tool pattern already used for `pdftoppm` + tesseract). Command: `magick convert input.heic output.jpg`, then run tesseract on the JPEG. If ImageMagick is not installed, return a clear error: "HEIC files require ImageMagick: `sudo apt install imagemagick`". Add `heic` to the extraction match arm.

### New Rust Module: `src-tauri/src/health_extract.rs`

**Responsibilities:**
- Receive raw extracted text + document type from classification
- Call local LLM to extract structured JSON matching the schema above
- Parse and validate the JSON response
- Store as unconfirmed records (confirmed = 0)
- Return extraction preview to UI for review

**LLM prompts (local model only — never cloud for extraction):**

*Prescription extraction prompt:*
```
Extract from this medical prescription text and return ONLY valid JSON:
{
  "prescribed_date": "YYYY-MM-DD or null",
  "prescriber_name": "string or null",
  "prescriber_specialty": "string or null",
  "facility_name": "string or null",
  "location_city": "string or null",
  "diagnosis_text": "string or null",
  "medications": [
    {"drug_name": "...", "dosage": "...", "frequency": "...", "duration_days": N or null, "instructions": "..."}
  ]
}
Text: {text}
```

*Lab report extraction prompt:*
```
Extract from this lab report and return ONLY valid JSON:
{
  "lab_name": "string or null",
  "report_date": "YYYY-MM-DD or null",
  "location_city": "string or null",
  "results": [
    {"test_name": "...", "value_text": "...", "value_numeric": N or null, "unit": "...", "reference_low": N or null, "reference_high": N or null}
  ]
}
Text: {text}
```

**Flagging logic (deterministic, no LLM):** After extraction, `flag` is computed in Rust:
- `value_numeric < reference_low` → "LOW"
- `value_numeric > reference_high` → "HIGH"  
- `value_numeric > reference_high * 1.5` → "CRITICAL"
- Otherwise → "NORMAL"

### New Tauri Commands

| Command | Description |
|---|---|
| `health_extract_document(file_id, patient_id)` | Run LLM extraction on an already-ingested file, returns preview |
| `health_confirm_prescription(data)` | Save confirmed prescription + items |
| `health_confirm_lab_result(data)` | Save confirmed lab result + values |
| `health_list_prescriptions(patient_id)` | List all prescriptions |
| `health_list_lab_results(patient_id)` | List structured lab results with values |
| `health_delete_prescription(id)` | Delete prescription + cascade items |
| `health_delete_lab_result(id)` | Delete lab result + cascade values |
| `health_get_lab_trends(patient_id, test_name)` | Return all values for one test over time (for trend chart) |

### UI Changes

New sub-tab **"Records"** added to the Health Vault page (alongside existing tabs):

- **Prescriptions sub-section:** Card per prescription showing date, prescriber, facility, medication list. Expand to see full details. Delete button.
- **Lab Results sub-section:** Card per report showing date, lab name, abnormal count. Expand to see full value table (colour-coded: red = HIGH/CRITICAL, amber = LOW, green = NORMAL).
- **Extraction flow:** Documents tab gains an "Extract →" button per ingested file. Opens review panel with the LLM-extracted fields editable before confirming.

---

## Phase C — Unified Health Intelligence

### New Tables (Migration 020)

```sql
CREATE TABLE IF NOT EXISTS location_visits (
    id           TEXT PRIMARY KEY,
    patient_id   TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
    visit_date   TEXT NOT NULL,
    city         TEXT NOT NULL,
    country      TEXT,
    source       TEXT NOT NULL,  -- "google_timeline" | "document_text"
    notes        TEXT,
    created_at   TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_location_visits_patient ON location_visits(patient_id, visit_date DESC);

CREATE TABLE IF NOT EXISTS health_timeline_events (
    id           TEXT PRIMARY KEY,
    patient_id   TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
    event_date   TEXT NOT NULL,
    category     TEXT NOT NULL,  -- "lab" | "prescription" | "fitness" | "location" | "symptom" | "condition" | "vital" | "vaccination"
    title        TEXT NOT NULL,
    description  TEXT,
    source_type  TEXT NOT NULL,  -- "structured_lab_result" | "prescription" | "fitness_metrics" | "symptom" | "location" | "manual"
    source_id    TEXT,
    severity     TEXT,           -- "info" | "warning" | "alert"
    metadata_json TEXT,
    created_at   TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_timeline_patient_date ON health_timeline_events(patient_id, event_date DESC);

CREATE TABLE IF NOT EXISTS health_intelligence_reports (
    id           TEXT PRIMARY KEY,
    patient_id   TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
    generated_at TEXT NOT NULL,
    model_used   TEXT NOT NULL,
    report_text  TEXT NOT NULL,
    anomalies_json TEXT,
    created_at   TEXT DEFAULT CURRENT_TIMESTAMP
);
```

### New Rust Module: `src-tauri/src/health_intelligence.rs`

**Timeline builder:** Queries all sources and inserts/refreshes `health_timeline_events`:
- `structured_lab_results` + `structured_lab_values` → lab events (severity based on flag count)
- `prescriptions` + `prescription_items` → prescription events
- `fitness_metrics` (weekly aggregates, not daily noise) → fitness events
- `symptoms` → symptom events
- `vitals` → vital events when outside normal range
- Location: extracted from `prescriptions.location_city` and `structured_lab_results.location_city`

**Anomaly detector (local, rule-based + LLM):**
- Statistical baseline: compute 90-day rolling mean + std-dev for each metric
- Flag when current value deviates > 1.5 std-dev from baseline
- Specific rules: sleep < 6h for 3+ consecutive days, HbA1c > 7%, lab value flagged HIGH/CRITICAL, medication gap > 7 days since prescription end
- LLM (local): given anomaly list, generate a 1-line human-readable description per anomaly

**Narrative generator (cloud optional with explicit per-report consent):**
- Collects last 180 days of timeline events
- Summarises: recent lab trends, active medications, fitness averages, notable anomalies
- Sends to cloud LLM (user's configured endpoint — Claude, GPT-4, etc.) with a structured prompt
- Returns doctor-visit narrative (300–500 words)
- User must click "Generate Report" and confirm the consent warning each time

**Google Timeline + Nutrition import:**
- Reuse existing Google OAuth flow (`gfit_*` pattern)
- Pull location history from Google Timeline API (monthly chunks, store city-level only — no GPS coordinates stored)
- Pull nutrition data from Google Fit (`com.google.nutrition` data type)
- Store location in a new `location_visits` table: `(date, city, country, source)`

### New Tauri Commands

| Command | Description |
|---|---|
| `health_rebuild_timeline(patient_id)` | Rebuild all timeline events from all sources |
| `health_get_timeline(patient_id, limit, offset)` | Paginated timeline fetch |
| `health_detect_anomalies(patient_id)` | Run anomaly detection, return list |
| `health_generate_report(patient_id, consent_confirmed)` | Generate narrative report via cloud LLM |
| `health_list_reports(patient_id)` | List past reports |
| `health_import_google_timeline(months)` | Import location history from Google |
| `health_import_google_nutrition(days)` | Import nutrition from Google Fit |
| `health_get_lab_correlation(patient_id, test_name_a, test_name_b)` | Compute correlation between two lab series |

### UI Changes

New **"Intelligence"** tab in Health Vault (last tab in the tab bar):

**Layout (three sections, stacked):**

1. **Anomaly Alerts** — collapsible card at top. Each alert: icon + severity colour + title + 1-line description. Empty state: "✓ No anomalies detected — run detection to refresh."

2. **Unified Timeline** — chronological scroll with month headers. Each event: coloured dot (by category) + title + description + date + source. Paginated (load 50 at a time). Filter chips: All / Labs / Prescriptions / Fitness / Location / Symptoms.

3. **AI Analysis Panel** — right-side panel (or bottom section on narrow):
   - Shows which model handles what (local vs cloud)
   - "Generate Doctor Report" button with consent warning
   - Last generated report (truncated, with "Copy for doctor" button)
   - Lab trend sparklines for key tests (HbA1c, LDL, glucose — if data exists)

---

## Data Flow Summary

```
Google Fit (auto-sync 15min) ──────────────────┐
Google Timeline (manual import) ───────────────┤
Nutrition from Google Fit (manual import) ─────┤
PDF / HEIC / JPG documents ────────────────────┤
  └→ OCR + LLM extraction (local)              │
     └→ prescriptions / structured_lab_results ┤
Manual entry (vitals, symptoms, conditions) ────┤
                                                ↓
                                health_timeline_events (rebuilt on demand)
                                                ↓
                    Anomaly detector (local LLM) → alerts
                    Narrative generator (cloud, opt-in) → doctor report
```

---

## AI Privacy Model

| Operation | Model | Data leaves device? |
|---|---|---|
| Document classification | Local Ollama | No |
| Structured extraction (prescriptions, labs) | Local Ollama | No |
| Anomaly detection labelling | Local Ollama | No |
| Doctor visit narrative | Cloud LLM (user choice) | Yes — explicit consent per report |

---

## Migration Plan

| Migration | Contents |
|---|---|
| 019 | `prescriptions`, `prescription_items`, `structured_lab_results`, `structured_lab_values` |
| 020 | `health_timeline_events`, `health_intelligence_reports`, `location_visits` |

---

## File Map

| Action | Path | Responsibility |
|---|---|---|
| Modify | `ui/src/pages/Fitness.tsx` | Phase A: wire real data, delete mocks |
| Create | `src-tauri/src/health_extract.rs` | Phase B: LLM extraction, confirm commands |
| Modify | `crates/minion-db/src/migrations.rs` | Migrations 019 + 020 |
| Modify | `src-tauri/src/health_ingestion.rs` | Phase B: HEIC support fix |
| Modify | `ui/src/pages/Health.tsx` | Phase B: Records tab; Phase C: Intelligence tab |
| Create | `ui/src/pages/health/RecordsTab.tsx` | Phase B: prescriptions + lab results UI |
| Create | `ui/src/pages/health/IntelligenceTab.tsx` | Phase C: timeline + anomalies + AI panel |
| Create | `src-tauri/src/health_intelligence.rs` | Phase C: timeline builder, anomaly detector, narrative |
| Modify | `src-tauri/src/lib.rs` | Register all new commands |

---

## Out of Scope (Phase C-2, future)

- Bluetooth wearable streaming
- Apple Health import (requires macOS companion)
- Medication reminder notifications
- Automated doctor appointment booking
- Insurance claim document parsing
