# Health Module — Design Specification

**Status:** In development (v1.10.0)
**Scope:** Longitudinal medical records, lab tests, medications, conditions, life events, symptoms, and AI-powered health analysis.

---

## 1. Overview

The Health module is a **longitudinal clinical record system** distinct from the Fitness module:

- **Fitness** = daily active tracking (habits, workouts, steps, live stats). Short-horizon, gamified.
- **Health** = clinical record (labs, diagnoses, meds, family history, imaging, life events). Long-horizon, analytical.

Both modules share the same SQLite store and Fitness data is pulled into Health AI analysis as correlation context, but the UIs are separate.

## 2. Core Requirements

1. **Multi-patient** — primary user plus family members (spouse, child, parent, dependent). Keyed by phone number.
2. **Bulk document ingestion** — PDFs, scanned images, photos spanning years.
3. **Chronological reconstruction** — dates extracted from content, not file metadata.
4. **Entity resolution** — normalize drug names, test names, doctor names.
5. **Episode linking** — group related events (e.g., "Diabetes workup Jan-Mar 2023").
6. **Life events** — user-entered context for correlation (job, diet, yoga, meditation, stress).
7. **Free-text symptoms** — with LLM classification for correlation analysis.
8. **Pluggable LLM providers** — Ollama, llama.cpp, AirLLM, Anthropic, OpenAI, Google.
9. **Local-first with Drive sync** — zero-knowledge encrypted backup via `drive.appdata` scope.
10. **Consent-gated** — explicit acceptance of data policy before first use.

## 3. Privacy Model

- All data stored locally in `~/.minion/health/vault/`, encrypted with AES-256-GCM.
- Original documents copied into the vault on import (not referenced).
- **No PII redaction** by default (user chose this) — but explicit consent required.
- AI analysis defaults to **local LLM only**. Cloud LLM is opt-in per-analysis.
- Google Drive sync uses `drive.appdata` scope (invisible app-data folder).
- Files are encrypted client-side before upload — Drive never sees plaintext.

## 4. Data Model

### Migration 006 — tables

#### Patients (multi-user support)

```sql
CREATE TABLE patients (
    id TEXT PRIMARY KEY,
    phone_number TEXT UNIQUE NOT NULL,   -- E.164 format
    full_name TEXT NOT NULL,
    date_of_birth TEXT,
    sex TEXT,                             -- M, F, other
    blood_group TEXT,                     -- A+, O-, etc.
    relationship TEXT NOT NULL,           -- self, spouse, child, parent,
                                          -- dependent, sibling, other
    is_primary INTEGER DEFAULT 0,         -- exactly one row
    avatar_color TEXT,
    notes TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX idx_primary_patient ON patients(is_primary) WHERE is_primary = 1;
CREATE INDEX idx_patient_phone ON patients(phone_number);
```

#### Consent tracking

```sql
CREATE TABLE health_consent (
    id INTEGER PRIMARY KEY,
    accepted_at TEXT NOT NULL,
    version TEXT NOT NULL,
    local_only_mode INTEGER DEFAULT 1,
    drive_sync_enabled INTEGER DEFAULT 0,
    cloud_llm_allowed INTEGER DEFAULT 0,
    user_signature TEXT
);
```

#### Medical records (generic event)

```sql
CREATE TABLE medical_records (
    id TEXT PRIMARY KEY,
    patient_id TEXT NOT NULL REFERENCES patients(id),
    record_type TEXT NOT NULL,            -- visit, diagnosis, procedure,
                                          -- prescription, imaging, vaccination
    title TEXT NOT NULL,
    description TEXT,
    doctor_id TEXT REFERENCES health_entities(id),
    facility_id TEXT REFERENCES health_entities(id),
    date TEXT NOT NULL,
    tags TEXT,                            -- JSON array
    document_file_id TEXT REFERENCES file_manifest(id),
    episode_id TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_medical_patient_date ON medical_records(patient_id, date DESC);
```

#### Lab tests

```sql
CREATE TABLE lab_tests (
    id TEXT PRIMARY KEY,
    patient_id TEXT NOT NULL REFERENCES patients(id),
    record_id TEXT REFERENCES medical_records(id),
    test_name TEXT NOT NULL,              -- as printed
    canonical_name TEXT,                  -- normalized (HbA1c, LDL, etc.)
    test_category TEXT,                   -- metabolic, lipid, cbc, thyroid,
                                          -- liver, kidney, hormonal, other
    value REAL NOT NULL,
    unit TEXT,
    reference_low REAL,
    reference_high REAL,
    reference_text TEXT,
    flag TEXT,                            -- H, L, HH, LL, normal, critical
    lab_entity_id TEXT REFERENCES health_entities(id),
    collected_at TEXT NOT NULL,
    reported_at TEXT,
    source TEXT,                          -- manual, pdf_import, photo_ocr
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_lab_patient_name_date ON lab_tests(patient_id, canonical_name, collected_at);
CREATE INDEX idx_lab_category ON lab_tests(patient_id, test_category);
```

#### Medications

```sql
CREATE TABLE medications (
    id TEXT PRIMARY KEY,
    patient_id TEXT NOT NULL REFERENCES patients(id),
    name TEXT NOT NULL,
    generic_name TEXT,
    dose TEXT,
    frequency TEXT,
    route TEXT,
    start_date TEXT,
    end_date TEXT,                        -- null = currently taking
    prescribing_doctor_id TEXT REFERENCES health_entities(id),
    indication TEXT,
    notes TEXT,
    record_id TEXT REFERENCES medical_records(id),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_meds_patient_active ON medications(patient_id) WHERE end_date IS NULL;
```

#### Conditions

```sql
CREATE TABLE conditions (
    id TEXT PRIMARY KEY,
    patient_id TEXT NOT NULL REFERENCES patients(id),
    name TEXT NOT NULL,
    condition_type TEXT,                  -- chronic, allergy, surgery, past
    severity TEXT,                        -- mild, moderate, severe
    diagnosed_at TEXT,
    resolved_at TEXT,
    notes TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
```

#### Vitals

```sql
CREATE TABLE vitals (
    id TEXT PRIMARY KEY,
    patient_id TEXT NOT NULL REFERENCES patients(id),
    measurement_type TEXT NOT NULL,       -- bp_systolic, bp_diastolic,
                                          -- glucose, temperature, spo2
    value REAL NOT NULL,
    unit TEXT,
    measured_at TEXT NOT NULL,
    context TEXT,                         -- "after meal", "fasting", "morning"
    notes TEXT
);
CREATE INDEX idx_vitals_patient_type_date ON vitals(patient_id, measurement_type, measured_at);
```

#### Family history

```sql
CREATE TABLE family_history (
    id TEXT PRIMARY KEY,
    patient_id TEXT NOT NULL REFERENCES patients(id),
    relation TEXT NOT NULL,               -- father, mother, sibling, grandparent
    condition TEXT NOT NULL,
    age_at_diagnosis INTEGER,
    notes TEXT
);
```

#### Life events (correlation layer)

```sql
CREATE TABLE life_events (
    id TEXT PRIMARY KEY,
    patient_id TEXT NOT NULL REFERENCES patients(id),
    category TEXT NOT NULL,
    subcategory TEXT,
    title TEXT NOT NULL,
    description TEXT,
    intensity INTEGER,                    -- 1-10
    started_at TEXT NOT NULL,
    ended_at TEXT,
    tags TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_life_patient_dates ON life_events(patient_id, started_at, ended_at);
```

**Categories:** `work`, `diet`, `exercise`, `yoga`, `meditation`, `spiritual`, `travel`, `relationship`, `stress`, `injury`, `illness`, `sleep`, `habit`, `environment`, `other`

**Meditation subcategories include:** vipassana, mindfulness, inner_engineering, shambhavi_mahamudra, isha_kriya, sadhana_intensive, retreat

**Yoga subcategories include:** asana_practice, pranayama, surya_namaskar, yoga_teacher_training, daily_practice

#### Symptoms (free-text with LLM classification)

```sql
CREATE TABLE symptoms (
    id TEXT PRIMARY KEY,
    patient_id TEXT NOT NULL REFERENCES patients(id),
    description TEXT NOT NULL,            -- free text
    canonical_name TEXT,                  -- LLM-extracted
    body_part TEXT,                       -- LLM-extracted
    laterality TEXT,                      -- left, right, bilateral
    severity INTEGER,                     -- 1-10
    first_noticed TEXT NOT NULL,
    resolved_at TEXT,
    frequency TEXT,                       -- constant, daily, weekly, intermittent
    triggers TEXT,                        -- JSON array, user's guesses
    llm_metadata TEXT,                    -- JSON: LLM extraction output
    notes TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_symptoms_patient_date ON symptoms(patient_id, first_noticed);
```

#### Episodes (grouped events)

```sql
CREATE TABLE episodes (
    id TEXT PRIMARY KEY,
    patient_id TEXT NOT NULL REFERENCES patients(id),
    name TEXT NOT NULL,
    description TEXT,
    start_date TEXT NOT NULL,
    end_date TEXT,
    primary_condition TEXT,
    ai_generated INTEGER DEFAULT 0,
    user_confirmed INTEGER DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
```

#### Health entities (normalized doctors, labs, medications, tests)

```sql
CREATE TABLE health_entities (
    id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,            -- doctor, facility, lab, medication, test
    canonical_name TEXT NOT NULL,
    aliases TEXT,                         -- JSON array
    metadata TEXT,                        -- JSON
    first_seen_at TEXT,
    UNIQUE(entity_type, canonical_name)
);
```

### Migration 007 — ingestion pipeline (week 2)

```sql
CREATE TABLE ingestion_jobs (
    id TEXT PRIMARY KEY,
    patient_id TEXT REFERENCES patients(id),
    source_folder TEXT,
    started_at TEXT DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT,
    status TEXT DEFAULT 'running',        -- running, paused, completed, failed
    total_files INTEGER DEFAULT 0,
    processed_files INTEGER DEFAULT 0,
    skipped_files INTEGER DEFAULT 0,
    failed_files INTEGER DEFAULT 0,
    model_used TEXT,
    error TEXT
);

CREATE TABLE file_manifest (
    id TEXT PRIMARY KEY,
    sha256 TEXT NOT NULL UNIQUE,
    original_path TEXT NOT NULL,
    stored_path TEXT,                     -- encrypted copy in vault
    mime_type TEXT,
    size_bytes INTEGER,
    status TEXT DEFAULT 'pending',
    patient_id TEXT REFERENCES patients(id),
    job_id TEXT REFERENCES ingestion_jobs(id),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    error TEXT
);
CREATE INDEX idx_file_status ON file_manifest(status);

CREATE TABLE document_extractions (
    id TEXT PRIMARY KEY,
    file_id TEXT NOT NULL REFERENCES file_manifest(id),
    document_type TEXT,
    classification_confidence REAL,
    raw_text TEXT,
    extracted_json TEXT,
    extraction_model TEXT,
    extraction_prompt_version TEXT,
    extracted_at TEXT DEFAULT CURRENT_TIMESTAMP,
    user_reviewed INTEGER DEFAULT 0,
    user_corrections TEXT
);
```

### Migration 008 — LLM provider abstraction (week 2)

```sql
CREATE TABLE llm_endpoints (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,                   -- "Local Ollama", "Claude"
    provider_type TEXT NOT NULL,          -- ollama, openai_compatible,
                                          -- anthropic, openai, google_gemini, airllm
    base_url TEXT NOT NULL,
    api_key_encrypted TEXT,
    default_model TEXT,
    headers_json TEXT,                    -- extra headers (JSON)
    enabled INTEGER DEFAULT 1,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE llm_feature_bindings (
    feature TEXT PRIMARY KEY,             -- health_extract, health_analyze,
                                          -- fitness_analyze, blog_seo, book_summary
    endpoint_id TEXT REFERENCES llm_endpoints(id),
    model_override TEXT
);
```

### Migration 009 — event timeline + Drive sync state (week 4/5)

```sql
CREATE TABLE event_timeline (
    id TEXT PRIMARY KEY,
    patient_id TEXT NOT NULL REFERENCES patients(id),
    event_type TEXT NOT NULL,             -- lab, visit, prescription, imaging,
                                          -- diagnosis, symptom, life_event,
                                          -- vital, vaccination
    event_date TEXT NOT NULL,
    date_confidence REAL,
    title TEXT NOT NULL,
    summary TEXT,
    entity_table TEXT,
    entity_id TEXT,
    source_file_id TEXT REFERENCES file_manifest(id),
    episode_id TEXT,
    importance INTEGER DEFAULT 3,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_timeline_patient_date ON event_timeline(patient_id, event_date DESC);
CREATE INDEX idx_timeline_type ON event_timeline(patient_id, event_type, event_date DESC);

CREATE TABLE drive_sync_state (
    id INTEGER PRIMARY KEY,
    oauth_token_encrypted TEXT,
    refresh_token_encrypted TEXT,
    token_expires_at TEXT,
    user_email TEXT,
    last_sync_at TEXT,
    last_change_page_token TEXT,
    sync_enabled INTEGER DEFAULT 0
);
```

## 5. LLM Provider System

### Supported providers

| Type | Purpose | Example config |
|------|---------|----------------|
| `ollama` | Ollama native API | `http://localhost:11434` |
| `openai_compatible` | Generic /v1/chat/completions | `http://localhost:8080/v1` (llama.cpp, LM Studio, vLLM) |
| `anthropic` | Claude API | `https://api.anthropic.com/v1` |
| `openai` | OpenAI API | `https://api.openai.com/v1` |
| `google_gemini` | Gemini API | `https://generativelanguage.googleapis.com/v1beta` |
| `airllm` | AirLLM wrapper (via OpenAI-compatible) | `http://localhost:8081/v1` |

### Integration notes

- **llama.cpp:** `llama-server` has built-in `/v1/chat/completions` — configure as `openai_compatible`.
- **AirLLM:** Python library, not HTTP by default. MINION ships `tools/minion-airllm-server.py` that wraps it in a FastAPI OpenAI-compatible server. User runs it, points MINION at `http://localhost:8081/v1`.
- **LM Studio / Jan.ai / oobabooga / KoboldCpp / vLLM / GPT4All:** All expose OpenAI-compatible endpoints — configure as `openai_compatible`.

### Per-feature binding

Each feature in MINION can use a different endpoint:

- `health_extract` — document classification and structured extraction
- `health_analyze` — timeline analysis with correlation
- `health_symptom_classify` — symptom free-text classification
- `fitness_analyze` — fitness data analysis
- `blog_seo` — blog SEO analysis
- `book_summary` — book chapter summaries

User can bind each feature to any endpoint in Settings → LLM.

## 6. Google Drive Sync

### Scope

Uses `https://www.googleapis.com/auth/drive.appdata` — a restricted scope that only allows access to a hidden, per-app data folder. Benefits:

- Files invisible in the user's normal Drive UI
- Auto-cleanup on uninstall
- No broad Drive access requested
- Reduced scary consent screen

### Sync algorithm

1. **Initial sync:** Upload all vault files, store last `startPageToken`.
2. **Incremental sync (every 15 min + on launch):**
   - Fetch `changes.list` since last `startPageToken`.
   - Diff local manifest vs Drive manifest by SHA-256:
     - Local-only → upload
     - Drive-only → download
     - Same hash → no-op
     - Different hash → conflict (keep both, user resolves)
3. Update `last_change_page_token`.
4. Emit `health-sync-status` Tauri event for UI progress.

### Encryption

Files are encrypted client-side *before* upload using a key derived from the user's master password via Argon2id. Drive never sees plaintext. Zero-knowledge pattern — Google cannot decrypt even if compromised.

### Conflict resolution

When the same file has different hashes locally and remotely:
1. Keep both versions locally in `conflicts/{timestamp}/`
2. Newer `updated_at` becomes canonical
3. Show notification: "3 conflicts from cloud sync — review now?"
4. User resolves each conflict via UI

### Offline tolerance

Sync is best-effort. If Drive unreachable, app works normally from local vault. Changes queue up and sync when connection returns.

## 7. Consent Modal (first-use)

Shown before first health document import or patient creation:

```
┌─ Health Vault — First-time Setup ────────────────────┐
│                                                       │
│  Welcome to MINION Health.                            │
│                                                       │
│  Your medical records will be stored locally on your │
│  device, encrypted with AES-256-GCM.                  │
│                                                       │
│  1. WHO ARE YOU?                                      │
│     Your name: [____________________]                 │
│     Phone:     [____________________]                 │
│     DOB:       [____________]  Sex: [M ▼]            │
│                                                       │
│  2. PRIVACY MODE                                      │
│     ● Local only — no data leaves your device        │
│     ○ Local + Google Drive backup (encrypted)        │
│                                                       │
│  3. AI ANALYSIS                                       │
│     ● Local LLM only (Ollama/llama.cpp/AirLLM)       │
│     ○ Allow cloud LLM with per-analysis consent      │
│                                                       │
│  ⚠ MINION's AI is EDUCATIONAL ONLY, not medical     │
│  advice. Always consult a licensed physician.         │
│                                                       │
│  ☐ I understand and accept                           │
│                                                       │
│  [Cancel]                         [Create Vault]     │
└───────────────────────────────────────────────────────┘
```

Stored in `health_consent` table with version string so future policy changes can re-prompt.

## 8. UI Architecture

### Sidebar
New top-level item: **Health** (Heart-in-Circle icon). Separate from Fitness.

### Patient switcher
Small dropdown in Health page header showing current patient's avatar + name. Clicking opens a panel with all patients, "+ Add patient" option, and quick-switch shortcuts.

### Tabs inside Health module

- **Dashboard** — health score (weekly), recent labs, active meds, upcoming reminders
- **Records** — all medical records sorted by date, filterable by type
- **Labs** — lab results with timeline chart per test (multi-year line graph)
- **Medications** — active + historical, schedule view
- **Conditions** — chronic conditions, allergies, surgeries
- **Life Events** — timeline of user-entered context
- **Symptoms** — free-text log with correlation hints
- **Family History** — per-relation conditions
- **AI Analysis** — prompt box + analysis history
- **Documents** — file browser for imported PDFs/images
- **Settings** — privacy, consent, Drive sync, LLM bindings

## 9. Shipping Plan

### Week 1 — Foundation (no AI)
- Migration 006 (patient tables + all core health tables)
- `minion-health` Rust crate with types and CRUD logic
- 15+ Tauri commands for CRUD
- Consent modal
- Patient switcher component
- New /health route + sidebar item
- Dashboard, Records, Labs, Medications, Conditions, Life Events, Symptoms, Family History tabs
- Manual entry forms
- Labs timeline chart (multi-year line graph per test)

### Week 2 — Ingestion Pipeline
- Migrations 007 + 008 (file_manifest, document_extractions, llm_endpoints)
- PDF text extraction (pdf_extract reuse)
- Tesseract OCR integration
- LLM provider abstraction (`minion-llm` crate)
- Classification + structured extraction
- Background job runner with progress events
- Vault encryption for document storage

### Week 3 — Review + Entity Resolution
- Review queue UI (side-by-side PDF + extracted fields)
- User corrections tracking
- health_entities normalization
- Fuzzy matching for drugs/tests/doctors
- Deduplication workflow

### Week 4 — Episodes + Timeline
- Migration 009 (event_timeline, drive_sync_state)
- Life events expanded categories (yoga, meditation, spiritual)
- Symptom LLM classification pipeline
- Episode auto-linking
- Timeline visualization (3-layer: events/symptoms/labs)
- Correlation graph with detail panel

### Week 5 — AI Analysis + Drive Sync
- Timeline-brief builder
- 4 analysis modes (trend, correlation, lifestyle, Q&A)
- Health-specific prompt templates with yoga/meditation context
- Cached analysis results
- Per-analysis cloud consent workflow
- Google Drive sync implementation (OAuth + encrypted upload/download)

## 10. Open questions for future iterations

- DICOM imaging viewer (out of scope for v1 — we only import report text)
- Family history graph visualization
- Medication interaction warnings (requires drug DB)
- Immunization schedule tracking
- Share-with-doctor export (encrypted PDF summary)
- Apple Health import (XML archive)
- Mobile companion app (read-only viewer of encrypted vault)
