# Health Intelligence Phase C — Unified Timeline, Anomaly Detection, AI Narrative

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a unified materialized timeline across all health data sources, a rule-based anomaly detector with optional LLM descriptions, a location-visit log, and a "Generate Doctor Report" AI narrative — all surfaced in a new "Intelligence" tab in Health.tsx.

**Architecture:**

- `crates/minion-db/src/migrations.rs` — migration 020 adds `location_visits`, `health_timeline_events`, `health_intelligence_reports`
- `src-tauri/src/health_intelligence.rs` — timeline builder, anomaly detector, report generator, 8 Tauri commands
- `src-tauri/src/lib.rs` — `mod health_intelligence;` + 8 handler registrations
- `ui/src/pages/health/IntelligenceTab.tsx` — Anomaly Alerts + Unified Timeline + AI Analysis Panel
- `ui/src/pages/Health.tsx` — new `'intelligence'` tab wired to `<IntelligenceTab>`

**Data flow:**
`health_rebuild_timeline` → materialized `health_timeline_events` → `health_get_timeline` feeds list.
`health_detect_anomalies` → rule engine + optional per-alert LLM description → in-memory `Vec<AnomalyAlert>` (no DB).
`health_generate_report` → builds prompt from last-180-days timeline events → LLM chat → persisted in `health_intelligence_reports`.

---

## File Map

| Action | Path | Responsibility |
|---|---|---|
| Modify | `crates/minion-db/src/migrations.rs` | Add migration 020, update count assertion 18 → 20 (019 is Phase B) |
| Create | `src-tauri/src/health_intelligence.rs` | Serde types + timeline builder + anomaly detector + report generator + 8 Tauri commands |
| Modify | `src-tauri/src/lib.rs` | `mod health_intelligence;` + register 8 commands |
| Create | `ui/src/pages/health/IntelligenceTab.tsx` | Anomaly Alerts + Timeline + AI Analysis Panel |
| Modify | `ui/src/pages/Health.tsx` | Add `'intelligence'` to HealthTab union + tab bar + `<Show>` render |

---

## Task 1: DB Migration 020

**Files:**
- Modify: `crates/minion-db/src/migrations.rs`

Adds `location_visits`, `health_timeline_events`, and `health_intelligence_reports` tables. Updates the `test_migrations_are_recorded` assertion from 18 to 20 (assumes Phase B migration 019 is already applied; if Phase B has NOT been applied yet, the count goes 18 → 19 first; in that scenario this task only adds migration 020 and bumps the assertion by the number of new migrations present at the time).

- [ ] **Step 1: Verify the baseline test passes**

```bash
cargo test -p minion-db -- test_migrations_are_recorded 2>&1 | tail -5
```

Expected: `test test_migrations_are_recorded ... ok`

- [ ] **Step 2: Add `("020_health_intelligence", migrate_020_health_intelligence)` to the MIGRATIONS array**

In `crates/minion-db/src/migrations.rs`, find:

```rust
        ("018_blog_llm", migrate_018_blog_llm),
    ];
```

Replace with:

```rust
        ("018_blog_llm", migrate_018_blog_llm),
        ("020_health_intelligence", migrate_020_health_intelligence),
    ];
```

> NOTE: If Phase B (019) is also being landed in the same branch, add both entries in numeric order: `("019_health_extract", ...)` then `("020_health_intelligence", ...)`.

- [ ] **Step 3: Add the migration function**

Immediately after the closing `}` of `migrate_018_blog_llm` (around line 1200, before `#[cfg(test)]`), insert:

```rust
/// Health Phase C: materialized timeline, location visits, intelligence reports.
fn migrate_020_health_intelligence(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS location_visits (
            id         TEXT PRIMARY KEY,
            patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            visit_date TEXT NOT NULL,
            city       TEXT NOT NULL,
            country    TEXT,
            source     TEXT NOT NULL,
            notes      TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_location_visits_patient
            ON location_visits(patient_id, visit_date DESC);

        CREATE TABLE IF NOT EXISTS health_timeline_events (
            id            TEXT PRIMARY KEY,
            patient_id    TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            event_date    TEXT NOT NULL,
            category      TEXT NOT NULL,
            title         TEXT NOT NULL,
            description   TEXT,
            source_type   TEXT NOT NULL,
            source_id     TEXT,
            severity      TEXT,
            metadata_json TEXT,
            created_at    TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_timeline_patient_date
            ON health_timeline_events(patient_id, event_date DESC);

        CREATE TABLE IF NOT EXISTS health_intelligence_reports (
            id             TEXT PRIMARY KEY,
            patient_id     TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            generated_at   TEXT NOT NULL,
            model_used     TEXT NOT NULL,
            report_text    TEXT NOT NULL,
            anomalies_json TEXT,
            created_at     TEXT DEFAULT CURRENT_TIMESTAMP
        );
        ",
    )?;
    Ok(())
}
```

- [ ] **Step 4: Update the migration count assertion**

In `test_migrations_are_recorded`, change:

```rust
        assert_eq!(count, 18);
```

To (adjust final number to reflect whichever migrations are present — 19 if only Phase C, 20 if both Phase B and Phase C):

```rust
        assert_eq!(count, 20);
```

- [ ] **Step 5: Verify**

```bash
cargo test -p minion-db 2>&1 | tail -10
```

Expected: all tests pass including `test_migrations_are_recorded`.

**Commit:**
```
feat(health): migration 020 — location_visits, health_timeline_events, health_intelligence_reports
```

---

## Task 2: Create `src-tauri/src/health_intelligence.rs` — Serde Types and DB Helpers

**Files:**
- Create: `src-tauri/src/health_intelligence.rs`

This task writes the file header, all `#[derive]` types, and two pure helper functions (`build_timeline_events` and the `get_endpoint` / `call_llm` stubs). The async commands are added in Task 3.

- [ ] **Step 1: Create the file**

Create `/home/dk/Documents/git/minion/src-tauri/src/health_intelligence.rs` with the following content:

```rust
//! Health Intelligence Phase C.
//!
//! Provides three capabilities:
//!   1. Unified materialized timeline — aggregates structured_lab_results,
//!      prescriptions, fitness_metrics, symptoms, vitals, and location_visits
//!      into health_timeline_events for fast paginated reads.
//!   2. Rule-based anomaly detector — five deterministic rules with optional
//!      one-sentence LLM descriptions (gracefully degrades without LLM).
//!   3. AI narrative report — summarises last 180 days into a doctor-friendly
//!      letter stored in health_intelligence_reports.

use crate::health_classify::classify_endpoint_for_feature;
use crate::state::AppState;
use chrono::Utc;
use minion_llm::{create_provider, ChatMessage, ChatRequest, ChatRole};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<AppState>>;

// =====================================================================
// Public serde types
// =====================================================================

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
    /// Stable hash: sha256-hex of "{patient_id}:{rule_name}:{date}" truncated to 16 chars.
    pub id: String,
    pub rule_name: String,
    /// "warning" | "alert"
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

// =====================================================================
// Timeline builder
// =====================================================================

/// Rebuild the materialized timeline for one patient.
///
/// Deletes all existing rows for `patient_id` then inserts fresh rows
/// sourced from: structured_lab_results, prescriptions, fitness_metrics
/// (weekly buckets), symptoms, vitals, and location_visits.
///
/// Returns the count of newly inserted rows.
pub fn build_timeline_events(
    conn: &rusqlite::Connection,
    patient_id: &str,
) -> Result<usize, String> {
    // Wipe stale rows.
    conn.execute(
        "DELETE FROM health_timeline_events WHERE patient_id = ?1",
        rusqlite::params![patient_id],
    )
    .map_err(|e| e.to_string())?;

    let now = Utc::now().to_rfc3339();
    let mut total: usize = 0;

    // ------------------------------------------------------------------
    // 1. structured_lab_results
    // ------------------------------------------------------------------
    {
        let mut stmt = conn
            .prepare(
                "SELECT slr.id, slr.report_date, slr.lab_name, slr.location_city,
                        COUNT(slv.id) FILTER (WHERE slv.flag IN ('CRITICAL','HIGH')) AS crit,
                        COUNT(slv.id) FILTER (WHERE slv.flag = 'LOW') AS low_count
                 FROM structured_lab_results slr
                 LEFT JOIN structured_lab_values slv ON slv.result_id = slr.id
                 WHERE slr.patient_id = ?1
                 GROUP BY slr.id
                 ORDER BY slr.report_date DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![patient_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            let (src_id, report_date, lab_name, city, crit, low_count) =
                r.map_err(|e| e.to_string())?;
            let severity = if crit > 0 {
                "alert"
            } else if low_count > 0 {
                "warning"
            } else {
                "info"
            };
            let title = lab_name.unwrap_or_else(|| "Lab Report".to_string());
            let description = city.map(|c| format!("at {c}"));
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO health_timeline_events
                 (id, patient_id, event_date, category, title, description,
                  source_type, source_id, severity, created_at)
                 VALUES (?1,?2,?3,'lab',?4,?5,'structured_lab_result',?6,?7,?8)",
                rusqlite::params![
                    id, patient_id, report_date, title, description, src_id, severity, now,
                ],
            )
            .map_err(|e| e.to_string())?;
            total += 1;
        }
    }

    // ------------------------------------------------------------------
    // 2. prescriptions
    // ------------------------------------------------------------------
    {
        let mut stmt = conn
            .prepare(
                "SELECT p.id, p.prescribed_date, p.prescriber_name, p.diagnosis_text,
                        COUNT(pi.id) AS item_count
                 FROM prescriptions p
                 LEFT JOIN prescription_items pi ON pi.prescription_id = p.id
                 WHERE p.patient_id = ?1
                 GROUP BY p.id
                 ORDER BY p.prescribed_date DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![patient_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            let (src_id, prescribed_date, prescriber, diagnosis, item_count) =
                r.map_err(|e| e.to_string())?;
            let title = prescriber
                .map(|p| format!("Prescription by Dr. {p}"))
                .unwrap_or_else(|| "Prescription".to_string());
            let description = diagnosis.map(|d| format!("{d} ({item_count} items)"))
                .or_else(|| Some(format!("{item_count} items")));
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO health_timeline_events
                 (id, patient_id, event_date, category, title, description,
                  source_type, source_id, severity, created_at)
                 VALUES (?1,?2,?3,'prescription',?4,?5,'prescription',?6,'info',?7)",
                rusqlite::params![
                    id, patient_id, prescribed_date, title, description, src_id, now,
                ],
            )
            .map_err(|e| e.to_string())?;
            total += 1;
        }
    }

    // ------------------------------------------------------------------
    // 3. fitness_metrics — weekly aggregates
    // ------------------------------------------------------------------
    {
        let mut stmt = conn
            .prepare(
                "SELECT strftime('%Y-W%W', date) AS week,
                        MIN(date)                AS week_start,
                        AVG(steps)               AS avg_steps,
                        AVG(sleep_hours)         AS avg_sleep,
                        AVG(heart_rate_avg)      AS avg_hr,
                        AVG(weight_kg)           AS avg_weight
                 FROM fitness_metrics
                 WHERE date IS NOT NULL
                 GROUP BY week
                 ORDER BY week DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(1)?, // week_start
                    row.get::<_, Option<f64>>(2)?,
                    row.get::<_, Option<f64>>(3)?,
                    row.get::<_, Option<f64>>(4)?,
                    row.get::<_, Option<f64>>(5)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            let (week_start, avg_steps, avg_sleep, avg_hr, avg_weight) =
                r.map_err(|e| e.to_string())?;
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
            let description = if parts.is_empty() {
                None
            } else {
                Some(parts.join(", "))
            };
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO health_timeline_events
                 (id, patient_id, event_date, category, title, description,
                  source_type, source_id, severity, created_at)
                 VALUES (?1,?2,?3,'fitness','Weekly Fitness Summary',?4,'fitness_metrics',NULL,?5,?6)",
                rusqlite::params![id, patient_id, week_start, description, severity, now],
            )
            .map_err(|e| e.to_string())?;
            total += 1;
        }
    }

    // ------------------------------------------------------------------
    // 4. symptoms
    // ------------------------------------------------------------------
    {
        let mut stmt = conn
            .prepare(
                "SELECT id, description, first_noticed, severity
                 FROM symptoms
                 WHERE patient_id = ?1
                 ORDER BY first_noticed DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![patient_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            let (src_id, description, first_noticed, severity_num) =
                r.map_err(|e| e.to_string())?;
            let title: String = description.chars().take(60).collect();
            let severity = if severity_num.map(|s| s >= 7).unwrap_or(false) {
                "warning"
            } else {
                "info"
            };
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO health_timeline_events
                 (id, patient_id, event_date, category, title, description,
                  source_type, source_id, severity, created_at)
                 VALUES (?1,?2,?3,'symptom',?4,?5,'symptom',?6,?7,?8)",
                rusqlite::params![
                    id, patient_id, first_noticed, title,
                    Some(description), src_id, severity, now
                ],
            )
            .map_err(|e| e.to_string())?;
            total += 1;
        }
    }

    // ------------------------------------------------------------------
    // 5. vitals
    // ------------------------------------------------------------------
    {
        let mut stmt = conn
            .prepare(
                "SELECT id, recorded_at, systolic_bp, diastolic_bp, heart_rate,
                        spo2_pct, weight_kg
                 FROM vitals
                 WHERE patient_id = ?1
                 ORDER BY recorded_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![patient_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<f64>>(2)?,
                    row.get::<_, Option<f64>>(3)?,
                    row.get::<_, Option<f64>>(4)?,
                    row.get::<_, Option<f64>>(5)?,
                    row.get::<_, Option<f64>>(6)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            let (src_id, recorded_at, systolic, diastolic, hr, spo2, weight) =
                r.map_err(|e| e.to_string())?;
            let bp_alert = systolic.map(|s| s > 140.0).unwrap_or(false)
                || diastolic.map(|d| d > 90.0).unwrap_or(false);
            let spo2_alert = spo2.map(|o| o < 94.0).unwrap_or(false);
            let severity = if bp_alert || spo2_alert { "alert" } else { "info" };
            let mut parts = Vec::new();
            if let (Some(s), Some(d)) = (systolic, diastolic) {
                parts.push(format!("BP {:.0}/{:.0}", s, d));
            }
            if let Some(h) = hr {
                parts.push(format!("HR {:.0}", h));
            }
            if let Some(o) = spo2 {
                parts.push(format!("SpO2 {:.0}%", o));
            }
            if let Some(w) = weight {
                parts.push(format!("{:.1}kg", w));
            }
            let description = if parts.is_empty() {
                None
            } else {
                Some(parts.join(", "))
            };
            let event_date = recorded_at.get(..10).unwrap_or(&recorded_at).to_string();
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO health_timeline_events
                 (id, patient_id, event_date, category, title, description,
                  source_type, source_id, severity, created_at)
                 VALUES (?1,?2,?3,'vital','Vitals Reading',?4,'vital',?5,?6,?7)",
                rusqlite::params![
                    id, patient_id, event_date, description, src_id, severity, now
                ],
            )
            .map_err(|e| e.to_string())?;
            total += 1;
        }
    }

    // ------------------------------------------------------------------
    // 6. location_visits
    // ------------------------------------------------------------------
    {
        let mut stmt = conn
            .prepare(
                "SELECT id, visit_date, city, country
                 FROM location_visits
                 WHERE patient_id = ?1
                 ORDER BY visit_date DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![patient_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            let (src_id, visit_date, city, country) = r.map_err(|e| e.to_string())?;
            let title = format!("Visit: {city}");
            let description = country.as_ref().map(|c| format!("{city}, {c}"));
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO health_timeline_events
                 (id, patient_id, event_date, category, title, description,
                  source_type, source_id, severity, created_at)
                 VALUES (?1,?2,?3,'location',?4,?5,'location_visit',?6,'info',?7)",
                rusqlite::params![id, patient_id, visit_date, title, description, src_id, now],
            )
            .map_err(|e| e.to_string())?;
            total += 1;
        }
    }

    Ok(total)
}

// =====================================================================
// Stable anomaly ID helper
// =====================================================================

/// Build a deterministic 16-char hex ID for an anomaly alert so the UI
/// can deduplicate across successive `health_detect_anomalies` calls.
fn anomaly_id(patient_id: &str, rule_name: &str, date: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    format!("{patient_id}:{rule_name}:{date}").hash(&mut h);
    format!("{:016x}", h.finish())
}
```

- [ ] **Step 2: Verify it compiles (no commands yet)**

```bash
cargo check -p minion-app 2>&1 | grep "health_intelligence" | head -10
```

Expected: no errors (the module isn't imported yet so this will not error on missing mod declaration; add `mod health_intelligence;` only in Task 4).

**Commit:** (defer — commit after Task 3 when the file is complete)

---

## Task 3: Add Async Commands to `health_intelligence.rs`

**Files:**
- Modify: `src-tauri/src/health_intelligence.rs`

Append all 8 `#[tauri::command]` functions plus the anomaly detector and report generator to the file created in Task 2.

- [ ] **Step 1: Append the Tauri commands to the file**

Append the following to `/home/dk/Documents/git/minion/src-tauri/src/health_intelligence.rs`:

```rust
// =====================================================================
// Tauri commands — Timeline
// =====================================================================

/// Rebuild the materialized timeline for a patient from all source tables.
/// Returns the count of inserted events.
#[tauri::command]
pub async fn health_rebuild_timeline(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<usize, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    build_timeline_events(&conn, &patient_id)
}

/// Paginated read of the materialized timeline.
/// `category_filter` accepts one of: lab | prescription | fitness | location | symptom | vital
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

    let (sql, params_cat): (String, bool) = if let Some(ref cat) = category_filter {
        (
            format!(
                "SELECT id, patient_id, event_date, category, title, description,
                        source_type, source_id, severity, metadata_json
                 FROM health_timeline_events
                 WHERE patient_id = ?1 AND category = ?2
                 ORDER BY event_date DESC
                 LIMIT ?3 OFFSET ?4"
            ),
            true,
        )
    } else {
        (
            "SELECT id, patient_id, event_date, category, title, description,
                    source_type, source_id, severity, metadata_json
             FROM health_timeline_events
             WHERE patient_id = ?1
             ORDER BY event_date DESC
             LIMIT ?2 OFFSET ?3"
                .to_string(),
            false,
        )
    };

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<TimelineEventRow> {
        Ok(TimelineEventRow {
            id: row.get(0)?,
            patient_id: row.get(1)?,
            event_date: row.get(2)?,
            category: row.get(3)?,
            title: row.get(4)?,
            description: row.get(5)?,
            source_type: row.get(6)?,
            source_id: row.get(7)?,
            severity: row.get(8)?,
            metadata_json: row.get(9)?,
        })
    };

    let rows: Result<Vec<TimelineEventRow>, rusqlite::Error> = if params_cat {
        stmt.query_map(
            rusqlite::params![patient_id, category_filter, limit, offset],
            map_row,
        )
        .and_then(|r| r.collect())
    } else {
        stmt.query_map(rusqlite::params![patient_id, limit, offset], map_row)
            .and_then(|r| r.collect())
    };
    rows.map_err(|e| e.to_string())
}

// =====================================================================
// Anomaly detector
// =====================================================================

/// Run five deterministic anomaly rules for a patient.
/// For each triggered rule, attempts a one-sentence LLM description;
/// falls back to the built-in title + description gracefully.
#[tauri::command]
pub async fn health_detect_anomalies(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<AnomalyAlert>, String> {
    let mut alerts: Vec<AnomalyAlert> = Vec::new();
    let now = Utc::now().format("%Y-%m-%d").to_string();

    // Collect all rule checks (synchronous DB part) before any .await.
    let triggered: Vec<(String, String, String, String)> = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        collect_anomaly_rules(&conn, &patient_id, &now)?
    };

    // Resolve LLM endpoint once (optional — graceful no-LLM fallback).
    let maybe_endpoint = {
        let ep = classify_endpoint_for_feature(&state, "health_analyze").await;
        match ep {
            Ok(Some(e)) => Some(e),
            _ => {
                // Also try the extract endpoint as fallback.
                classify_endpoint_for_feature(&state, "health_extract")
                    .await
                    .ok()
                    .flatten()
            }
        }
    };

    for (rule_name, severity, title, base_description) in triggered {
        let description = if let Some(ref ep) = maybe_endpoint {
            let provider = create_provider(ep.clone());
            let req = ChatRequest {
                messages: vec![ChatMessage {
                    role: ChatRole::User,
                    content: format!(
                        "Write exactly one plain-English sentence (no markdown, no lists) \
                         summarising this health anomaly for a patient: {base_description}"
                    ),
                }],
                model: None,
                temperature: Some(0.2_f32),
                json_mode: false,
                max_tokens: Some(80_u32),
                system: Some(
                    "You are a concise medical assistant. \
                     Return exactly one sentence without markdown."
                        .to_string(),
                ),
            };
            match provider.chat(req).await {
                Ok(r) if !r.content.trim().is_empty() => r.content.trim().to_string(),
                _ => base_description.clone(),
            }
        } else {
            base_description.clone()
        };

        alerts.push(AnomalyAlert {
            id: anomaly_id(&patient_id, &rule_name, &now),
            rule_name,
            severity,
            title,
            description,
            detected_at: now.clone(),
        });
    }

    Ok(alerts)
}

/// Pure synchronous rule evaluations.
/// Returns Vec of (rule_name, severity, title, base_description).
fn collect_anomaly_rules(
    conn: &rusqlite::Connection,
    patient_id: &str,
    today: &str,
) -> Result<Vec<(String, String, String, String)>, String> {
    let mut triggered: Vec<(String, String, String, String)> = Vec::new();

    // Rule 1: hba1c_rising — last 3 HbA1c values all > 7.0
    {
        let vals: Result<Vec<f64>, _> = conn
            .prepare(
                "SELECT value_numeric FROM structured_lab_values
                 WHERE test_name LIKE '%HbA1c%'
                   AND result_id IN (
                       SELECT id FROM structured_lab_results
                       WHERE patient_id = ?1
                   )
                   AND value_numeric IS NOT NULL
                 ORDER BY (
                     SELECT report_date FROM structured_lab_results
                     WHERE id = structured_lab_values.result_id
                 ) DESC
                 LIMIT 3",
            )
            .and_then(|mut s| {
                s.query_map(rusqlite::params![patient_id], |r| r.get::<_, f64>(0))
                    .and_then(|rows| rows.collect())
            });
        if let Ok(vals) = vals {
            if vals.len() == 3 && vals.iter().all(|&v| v > 7.0) {
                triggered.push((
                    "hba1c_rising".into(),
                    "alert".into(),
                    "HbA1c Persistently Elevated".into(),
                    format!(
                        "Your last 3 HbA1c readings ({:.1}, {:.1}, {:.1}) are all above 7.0, \
                         suggesting sustained high blood sugar.",
                        vals[0], vals[1], vals[2]
                    ),
                ));
            }
        }
    }

    // Rule 2: sleep_deficit — last 7 days avg sleep < 6.0 h
    {
        let since = {
            use chrono::NaiveDate;
            NaiveDate::parse_from_str(&today[..10], "%Y-%m-%d")
                .map(|d| (d - chrono::Duration::days(7)).format("%Y-%m-%d").to_string())
                .unwrap_or_else(|_| today.to_string())
        };
        let avg_sleep: Option<f64> = conn
            .query_row(
                "SELECT AVG(sleep_hours) FROM fitness_metrics
                 WHERE date >= ?1 AND sleep_hours IS NOT NULL",
                rusqlite::params![since],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        if let Some(avg) = avg_sleep {
            if avg < 6.0 {
                triggered.push((
                    "sleep_deficit".into(),
                    "warning".into(),
                    "Sleep Deficit Detected".into(),
                    format!(
                        "Your average sleep over the past 7 days is {:.1} hours, \
                         below the recommended 6 hours.",
                        avg
                    ),
                ));
            }
        }
    }

    // Rule 3: lab_critical — any CRITICAL flag in last 90 days
    {
        let since = {
            use chrono::NaiveDate;
            NaiveDate::parse_from_str(&today[..10], "%Y-%m-%d")
                .map(|d| (d - chrono::Duration::days(90)).format("%Y-%m-%d").to_string())
                .unwrap_or_else(|_| today.to_string())
        };
        let critical_test: Option<String> = conn
            .query_row(
                "SELECT slv.test_name FROM structured_lab_values slv
                 JOIN structured_lab_results slr ON slr.id = slv.result_id
                 WHERE slr.patient_id = ?1
                   AND slv.flag = 'CRITICAL'
                   AND slr.report_date >= ?2
                 ORDER BY slr.report_date DESC
                 LIMIT 1",
                rusqlite::params![patient_id, since],
                |r| r.get(0),
            )
            .ok();
        if let Some(test) = critical_test {
            triggered.push((
                "lab_critical".into(),
                "alert".into(),
                "Critical Lab Value".into(),
                format!(
                    "A CRITICAL flag was found on '{test}' in the past 90 days. \
                     Please review with your doctor immediately."
                ),
            ));
        }
    }

    // Rule 4: bp_elevated — systolic > 140 in last 30 days
    {
        let since = {
            use chrono::NaiveDate;
            NaiveDate::parse_from_str(&today[..10], "%Y-%m-%d")
                .map(|d| (d - chrono::Duration::days(30)).format("%Y-%m-%d").to_string())
                .unwrap_or_else(|_| today.to_string())
        };
        let high_bp: Option<f64> = conn
            .query_row(
                "SELECT systolic_bp FROM vitals
                 WHERE patient_id = ?1
                   AND systolic_bp > 140
                   AND recorded_at >= ?2
                 ORDER BY recorded_at DESC
                 LIMIT 1",
                rusqlite::params![patient_id, since],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        if let Some(bp) = high_bp {
            triggered.push((
                "bp_elevated".into(),
                "warning".into(),
                "Elevated Blood Pressure".into(),
                format!(
                    "A recent systolic reading of {:.0} mmHg exceeds 140 mmHg. \
                     Consider monitoring and consulting a physician.",
                    bp
                ),
            ));
        }
    }

    // Rule 5: vitamin_gap — vitamin prescription older than 12 weeks with no follow-up lab
    {
        let twelve_weeks_ago = {
            use chrono::NaiveDate;
            NaiveDate::parse_from_str(&today[..10], "%Y-%m-%d")
                .map(|d| (d - chrono::Duration::weeks(12)).format("%Y-%m-%d").to_string())
                .unwrap_or_else(|_| today.to_string())
        };
        let vitamin_rx: Option<String> = conn
            .query_row(
                "SELECT pi.drug_name FROM prescription_items pi
                 JOIN prescriptions p ON p.id = pi.prescription_id
                 WHERE p.patient_id = ?1
                   AND pi.drug_name LIKE '%Vitamin%'
                   AND p.prescribed_date > ?2
                 ORDER BY p.prescribed_date DESC
                 LIMIT 1",
                rusqlite::params![patient_id, twelve_weeks_ago],
                |r| r.get(0),
            )
            .ok();
        if let Some(drug) = vitamin_rx {
            // Check for a follow-up lab referencing the same vitamin in the same period.
            let has_lab: bool = conn
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM structured_lab_values slv
                         JOIN structured_lab_results slr ON slr.id = slv.result_id
                         WHERE slr.patient_id = ?1
                           AND slv.test_name LIKE ?2
                           AND slr.report_date > ?3
                     )",
                    rusqlite::params![patient_id, format!("%{}%", drug), twelve_weeks_ago],
                    |r| r.get(0),
                )
                .unwrap_or(false);
            if !has_lab {
                triggered.push((
                    "vitamin_gap".into(),
                    "warning".into(),
                    "Vitamin Follow-up Missing".into(),
                    format!(
                        "You were prescribed '{drug}' in the past 12 weeks \
                         but no follow-up lab for this supplement was found."
                    ),
                ));
            }
        }
    }

    Ok(triggered)
}

// =====================================================================
// Tauri commands — Reports
// =====================================================================

/// Generate an AI narrative "doctor report" for the last 180 days.
/// `consent_confirmed` must be `true` or the command returns an error.
#[tauri::command]
pub async fn health_generate_report(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    consent_confirmed: bool,
) -> Result<IntelligenceReport, String> {
    if !consent_confirmed {
        return Err(
            "Consent required: check the box to confirm you accept sending \
             your health summary to the configured LLM."
                .to_string(),
        );
    }

    // Build the timeline summary (last 180 days).
    let cutoff = {
        use chrono::NaiveDate;
        let today_str = Utc::now().format("%Y-%m-%d").to_string();
        NaiveDate::parse_from_str(&today_str, "%Y-%m-%d")
            .map(|d| (d - chrono::Duration::days(180)).format("%Y-%m-%d").to_string())
            .unwrap_or(today_str)
    };

    let summary = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT event_date, category, title, description, severity
                 FROM health_timeline_events
                 WHERE patient_id = ?1 AND event_date >= ?2
                 ORDER BY event_date DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![patient_id, cutoff], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut lines: Vec<String> = Vec::new();
        for r in rows {
            let (date, cat, title, desc, sev) = r.map_err(|e| e.to_string())?;
            let sev_tag = sev.unwrap_or_else(|| "info".into());
            let desc_part = desc
                .filter(|d| !d.is_empty())
                .map(|d| format!(" — {d}"))
                .unwrap_or_default();
            lines.push(format!("[{date}] [{cat}] [{sev_tag}] {title}{desc_part}"));
        }
        if lines.is_empty() {
            return Err("No timeline events found in the last 180 days. \
                        Run 'Rebuild Timeline' first."
                .to_string());
        }
        lines.join("\n")
    };

    // Resolve endpoint.
    let endpoint = {
        let ep = classify_endpoint_for_feature(&state, "health_analyze").await;
        match ep {
            Ok(Some(e)) => e,
            _ => classify_endpoint_for_feature(&state, "health_extract")
                .await?
                .ok_or_else(|| "no LLM endpoint configured".to_string())?,
        }
    };

    let model_name = endpoint.default_model.clone();
    let provider = create_provider(endpoint);

    let system = "You are a careful medical assistant. \
        Using ONLY the structured timeline events provided, write a concise \
        doctor-ready health summary in plain English. \
        Group observations by body system. \
        Note trends, anomalies, and items needing attention. \
        End with a one-paragraph 'Suggested follow-up' section. \
        Do NOT invent data not present in the timeline. \
        Include a footer: 'This summary was generated by AI and must be \
        reviewed by a qualified clinician before clinical use.'";

    let user_msg = format!(
        "PATIENT TIMELINE (last 180 days):\n\n{summary}\n\n\
         Write the health summary now."
    );

    let resp = provider
        .chat(ChatRequest {
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: user_msg,
            }],
            model: None,
            temperature: Some(0.2_f32),
            json_mode: false,
            max_tokens: Some(2000_u32),
            system: Some(system.to_string()),
        })
        .await
        .map_err(|e| e.to_string())?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO health_intelligence_reports
             (id, patient_id, generated_at, model_used, report_text, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![id, patient_id, now, model_name, resp.content, now],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(IntelligenceReport {
        id,
        patient_id,
        generated_at: now.clone(),
        model_used: model_name,
        report_text: resp.content,
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
             WHERE patient_id = ?1
             ORDER BY generated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![patient_id], |row| {
            Ok(IntelligenceReport {
                id: row.get(0)?,
                patient_id: row.get(1)?,
                generated_at: row.get(2)?,
                model_used: row.get(3)?,
                report_text: row.get(4)?,
                anomalies_json: row.get(5)?,
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
pub async fn health_delete_report(
    state: State<'_, AppStateHandle>,
    id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM health_intelligence_reports WHERE id = ?1",
        rusqlite::params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
// Tauri commands — Location Visits
// =====================================================================

#[tauri::command]
pub async fn health_add_location_visit(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    visit_date: String,
    city: String,
    country: Option<String>,
    source: String,
    notes: Option<String>,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO location_visits
         (id, patient_id, visit_date, city, country, source, notes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![id, patient_id, visit_date, city, country, source, notes, now],
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
             FROM location_visits
             WHERE patient_id = ?1
             ORDER BY visit_date DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![patient_id], |row| {
            Ok(LocationVisit {
                id: row.get(0)?,
                patient_id: row.get(1)?,
                visit_date: row.get(2)?,
                city: row.get(3)?,
                country: row.get(4)?,
                source: row.get(5)?,
                notes: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anomaly_id_is_deterministic() {
        let a = anomaly_id("patient-1", "hba1c_rising", "2025-01-01");
        let b = anomaly_id("patient-1", "hba1c_rising", "2025-01-01");
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn anomaly_id_differs_on_different_inputs() {
        let a = anomaly_id("patient-1", "hba1c_rising", "2025-01-01");
        let b = anomaly_id("patient-1", "sleep_deficit", "2025-01-01");
        assert_ne!(a, b);
    }
}
```

- [ ] **Step 2: Verify unit tests pass**

```bash
cargo test -p minion-app -- health_intelligence 2>&1 | tail -10
```

Expected:
```
test health_intelligence::tests::anomaly_id_is_deterministic ... ok
test health_intelligence::tests::anomaly_id_differs_on_different_inputs ... ok
```

**Commit:**
```
feat(health): health_intelligence.rs — timeline builder, anomaly detector, report generator, 8 Tauri commands
```

---

## Task 4: Wire `lib.rs`

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add module declaration**

In `src-tauri/src/lib.rs`, after the line `mod health_timeline;`, add:

```rust
mod health_intelligence;
```

- [ ] **Step 2: Register commands in `generate_handler!`**

In `src-tauri/src/lib.rs`, after the last `health_drive_sync::` handler block (before the sysmon block), add:

```rust
            // Health Intelligence Phase C — timeline, anomalies, reports, location
            health_intelligence::health_rebuild_timeline,
            health_intelligence::health_get_timeline,
            health_intelligence::health_detect_anomalies,
            health_intelligence::health_generate_report,
            health_intelligence::health_list_reports,
            health_intelligence::health_delete_report,
            health_intelligence::health_add_location_visit,
            health_intelligence::health_list_location_visits,
```

- [ ] **Step 3: Verify the app compiles**

```bash
cargo check -p minion-app 2>&1 | tail -5
```

Expected: `Finished dev [unoptimized + debuginfo] target(s) in ...` with no errors.

- [ ] **Step 4: Run Tauri-layer tests**

```bash
cargo test -p minion-app 2>&1 | tail -10
```

Expected: all tests pass.

**Commit:**
```
feat(health): wire health_intelligence commands in lib.rs
```

---

## Task 5: Create `ui/src/pages/health/IntelligenceTab.tsx`

**Files:**
- Create: `ui/src/pages/health/IntelligenceTab.tsx`

- [ ] **Step 1: Create the file**

```tsx
import { Component, createSignal, createEffect, For, Show } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

// =====================================================================
// Types
// =====================================================================

interface TimelineEventRow {
  id: string;
  patient_id: string;
  event_date: string;
  category: string;
  title: string;
  description?: string;
  source_type: string;
  source_id?: string;
  severity?: string;
  metadata_json?: string;
}

interface AnomalyAlert {
  id: string;
  rule_name: string;
  severity: string;
  title: string;
  description: string;
  detected_at: string;
}

interface IntelligenceReport {
  id: string;
  patient_id: string;
  generated_at: string;
  model_used: string;
  report_text: string;
  anomalies_json?: string;
}

// =====================================================================
// Helpers
// =====================================================================

const CATEGORY_COLORS: Record<string, string> = {
  lab: 'bg-blue-500',
  prescription: 'bg-purple-500',
  fitness: 'bg-green-500',
  location: 'bg-yellow-500',
  symptom: 'bg-orange-500',
  vital: 'bg-red-500',
};

const SEVERITY_BORDER: Record<string, string> = {
  alert: 'border-l-4 border-red-500',
  warning: 'border-l-4 border-amber-400',
  info: 'border-l-4 border-gray-300 dark:border-gray-600',
};

const SEVERITY_ICON: Record<string, string> = {
  alert: '🔴',
  warning: '🟡',
  info: '🟢',
};

function fmtDate(s: string): string {
  try {
    return new Date(s).toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  } catch {
    return s.slice(0, 10);
  }
}

function monthKey(dateStr: string): string {
  return dateStr.slice(0, 7); // "YYYY-MM"
}

function monthLabel(key: string): string {
  const [y, m] = key.split('-');
  const d = new Date(Number(y), Number(m) - 1, 1);
  return d.toLocaleDateString(undefined, { month: 'long', year: 'numeric' });
}

// =====================================================================
// Component
// =====================================================================

const IntelligenceTab: Component<{ patientId: string }> = (props) => {
  // ----- anomaly state -----
  const [anomalies, setAnomalies] = createSignal<AnomalyAlert[]>([]);
  const [loadingAnomalies, setLoadingAnomalies] = createSignal(false);
  const [anomalyError, setAnomalyError] = createSignal<string | null>(null);
  const [anomalyOpen, setAnomalyOpen] = createSignal(true);

  // ----- timeline state -----
  const [events, setEvents] = createSignal<TimelineEventRow[]>([]);
  const [timelineOffset, setTimelineOffset] = createSignal(0);
  const [hasMore, setHasMore] = createSignal(false);
  const [loadingTimeline, setLoadingTimeline] = createSignal(false);
  const [timelineError, setTimelineError] = createSignal<string | null>(null);
  const [rebuildLoading, setRebuildLoading] = createSignal(false);
  const [categoryFilter, setCategoryFilter] = createSignal<string | undefined>(undefined);
  const PAGE = 50;

  // ----- report state -----
  const [reports, setReports] = createSignal<IntelligenceReport[]>([]);
  const [reportLoading, setReportLoading] = createSignal(false);
  const [reportError, setReportError] = createSignal<string | null>(null);
  const [consentChecked, setConsentChecked] = createSignal(false);
  const [expandedReport, setExpandedReport] = createSignal<string | null>(null);

  // ==================== Anomalies ====================

  const detectAnomalies = async () => {
    setLoadingAnomalies(true);
    setAnomalyError(null);
    try {
      const list = await invoke<AnomalyAlert[]>('health_detect_anomalies', {
        patient_id: props.patientId,
      });
      setAnomalies(list);
    } catch (e) {
      setAnomalyError(String(e));
    } finally {
      setLoadingAnomalies(false);
    }
  };

  // ==================== Timeline ====================

  const loadTimeline = async (reset: boolean) => {
    setLoadingTimeline(true);
    setTimelineError(null);
    const offset = reset ? 0 : timelineOffset();
    try {
      const rows = await invoke<TimelineEventRow[]>('health_get_timeline', {
        patient_id: props.patientId,
        limit: PAGE + 1,
        offset,
        category_filter: categoryFilter() ?? null,
      });
      const hasMoreRows = rows.length > PAGE;
      const page = rows.slice(0, PAGE);
      if (reset) {
        setEvents(page);
        setTimelineOffset(0);
      } else {
        setEvents((prev) => [...prev, ...page]);
      }
      setHasMore(hasMoreRows);
      setTimelineOffset(offset + page.length);
    } catch (e) {
      setTimelineError(String(e));
    } finally {
      setLoadingTimeline(false);
    }
  };

  const rebuildTimeline = async () => {
    setRebuildLoading(true);
    setTimelineError(null);
    try {
      const count = await invoke<number>('health_rebuild_timeline', {
        patient_id: props.patientId,
      });
      setTimelineOffset(0);
      await loadTimeline(true);
      console.log(`Rebuilt ${count} timeline events`);
    } catch (e) {
      setTimelineError(String(e));
    } finally {
      setRebuildLoading(false);
    }
  };

  // ==================== Reports ====================

  const loadReports = async () => {
    try {
      const list = await invoke<IntelligenceReport[]>('health_list_reports', {
        patient_id: props.patientId,
      });
      setReports(list);
    } catch (e) {
      setReportError(String(e));
    }
  };

  const generateReport = async () => {
    if (!consentChecked()) return;
    setReportLoading(true);
    setReportError(null);
    try {
      await invoke<IntelligenceReport>('health_generate_report', {
        patient_id: props.patientId,
        consent_confirmed: true,
      });
      await loadReports();
    } catch (e) {
      setReportError(String(e));
    } finally {
      setReportLoading(false);
    }
  };

  const deleteReport = async (id: string) => {
    try {
      await invoke('health_delete_report', { id });
      setReports((prev) => prev.filter((r) => r.id !== id));
    } catch (e) {
      setReportError(String(e));
    }
  };

  const copyReport = (text: string) => {
    navigator.clipboard.writeText(text).catch(() => undefined);
  };

  // ==================== Effects ====================

  createEffect(() => {
    props.patientId;
    setAnomalies([]);
    setEvents([]);
    setReports([]);
    setTimelineOffset(0);
    setCategoryFilter(undefined);
    loadTimeline(true);
    loadReports();
  });

  // Re-fetch timeline when filter changes.
  createEffect(() => {
    categoryFilter();
    loadTimeline(true);
  });

  // ==================== Derived ====================

  // Group events by "YYYY-MM" for month headers.
  const groupedEvents = () => {
    const groups: Array<{ month: string; rows: TimelineEventRow[] }> = [];
    let current: string | null = null;
    for (const ev of events()) {
      const mk = monthKey(ev.event_date);
      if (mk !== current) {
        groups.push({ month: mk, rows: [] });
        current = mk;
      }
      groups[groups.length - 1].rows.push(ev);
    }
    return groups;
  };

  // ==================== Render ====================

  const FILTERS: Array<[string | undefined, string]> = [
    [undefined, 'All'],
    ['lab', 'Labs'],
    ['prescription', 'Prescriptions'],
    ['fitness', 'Fitness'],
    ['location', 'Location'],
    ['symptom', 'Symptoms'],
    ['vital', 'Vitals'],
  ];

  return (
    <div class="space-y-8">

      {/* ============================================================
          Section 1: Anomaly Alerts
          ============================================================ */}
      <div class="card p-4">
        <div class="flex items-center justify-between mb-3">
          <button
            class="flex items-center gap-2 font-semibold text-gray-800 dark:text-gray-100"
            onClick={() => setAnomalyOpen((v) => !v)}
          >
            <span>{anomalyOpen() ? '▾' : '▸'}</span>
            <span>Anomaly Alerts</span>
            <Show when={anomalies().length > 0}>
              <span class="ml-1 px-2 py-0.5 text-xs rounded-full bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-200">
                {anomalies().length}
              </span>
            </Show>
          </button>
          <button
            class="btn btn-secondary text-sm px-3 py-1"
            onClick={detectAnomalies}
            disabled={loadingAnomalies()}
          >
            {loadingAnomalies() ? 'Detecting…' : '↻ Run Detection'}
          </button>
        </div>

        <Show when={anomalyOpen()}>
          <Show when={anomalyError()}>
            <p class="text-sm text-red-600 dark:text-red-400 mb-2">{anomalyError()}</p>
          </Show>

          <Show
            when={anomalies().length > 0}
            fallback={
              <p class="text-sm text-gray-500 dark:text-gray-400 italic">
                ✓ No anomalies detected — click "Run Detection" to check.
              </p>
            }
          >
            <div class="space-y-2">
              <For each={anomalies()}>
                {(alert) => (
                  <div
                    class={`rounded p-3 bg-gray-50 dark:bg-gray-800 ${SEVERITY_BORDER[alert.severity] ?? SEVERITY_BORDER.info}`}
                  >
                    <div class="flex items-start gap-2">
                      <span class="mt-0.5">{SEVERITY_ICON[alert.severity] ?? '⚪'}</span>
                      <div class="flex-1 min-w-0">
                        <p class="font-medium text-gray-900 dark:text-gray-100 text-sm">
                          {alert.title}
                        </p>
                        <p class="text-sm text-gray-600 dark:text-gray-300 mt-0.5">
                          {alert.description}
                        </p>
                        <p class="text-xs text-gray-400 mt-1">
                          Detected {fmtDate(alert.detected_at)} · rule: {alert.rule_name}
                        </p>
                      </div>
                    </div>
                  </div>
                )}
              </For>
            </div>
          </Show>
        </Show>
      </div>

      {/* ============================================================
          Section 2: Unified Timeline
          ============================================================ */}
      <div class="card p-4">
        <div class="flex flex-wrap items-center gap-3 mb-4">
          <h3 class="font-semibold text-gray-800 dark:text-gray-100">Unified Timeline</h3>
          <div class="flex-1" />
          <button
            class="btn btn-secondary text-sm px-3 py-1"
            onClick={rebuildTimeline}
            disabled={rebuildLoading()}
          >
            {rebuildLoading() ? 'Rebuilding…' : '↻ Rebuild Timeline'}
          </button>
        </div>

        {/* Category filter chips */}
        <div class="flex flex-wrap gap-1 mb-4">
          <For each={FILTERS}>
            {([val, label]) => (
              <button
                class="px-3 py-1 rounded-full text-xs font-medium border transition-colors"
                classList={{
                  'bg-minion-500 text-white border-minion-500': categoryFilter() === val,
                  'bg-white dark:bg-gray-800 text-gray-600 dark:text-gray-300 border-gray-300 dark:border-gray-600 hover:border-minion-400':
                    categoryFilter() !== val,
                }}
                onClick={() => setCategoryFilter(val)}
              >
                {label}
              </button>
            )}
          </For>
        </div>

        <Show when={timelineError()}>
          <p class="text-sm text-red-600 dark:text-red-400 mb-2">{timelineError()}</p>
        </Show>

        <Show
          when={events().length > 0}
          fallback={
            <p class="text-sm text-gray-500 dark:text-gray-400 italic">
              No timeline events. Click "Rebuild Timeline" to generate them from your health data.
            </p>
          }
        >
          <div class="space-y-1">
            <For each={groupedEvents()}>
              {(group) => (
                <>
                  <div class="sticky top-0 z-10 bg-gray-100 dark:bg-gray-900 px-2 py-1 text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide rounded mt-4 mb-1">
                    {monthLabel(group.month)}
                  </div>
                  <For each={group.rows}>
                    {(ev) => (
                      <div class="flex items-start gap-3 py-2 border-b border-gray-100 dark:border-gray-800 last:border-0">
                        <div class="mt-1 flex-shrink-0">
                          <span
                            class={`inline-block w-2.5 h-2.5 rounded-full ${CATEGORY_COLORS[ev.category] ?? 'bg-gray-400'}`}
                          />
                        </div>
                        <div class="flex-1 min-w-0">
                          <div class="flex items-baseline gap-2">
                            <span class="font-medium text-sm text-gray-900 dark:text-gray-100">
                              {ev.title}
                            </span>
                            <Show when={ev.severity && ev.severity !== 'info'}>
                              <span
                                class="text-xs px-1.5 py-0.5 rounded"
                                classList={{
                                  'bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-200':
                                    ev.severity === 'alert',
                                  'bg-amber-100 text-amber-700 dark:bg-amber-900 dark:text-amber-200':
                                    ev.severity === 'warning',
                                }}
                              >
                                {ev.severity}
                              </span>
                            </Show>
                          </div>
                          <Show when={ev.description}>
                            <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                              {ev.description}
                            </p>
                          </Show>
                        </div>
                        <span class="flex-shrink-0 text-xs text-gray-400 dark:text-gray-500 mt-1">
                          {fmtDate(ev.event_date)}
                        </span>
                      </div>
                    )}
                  </For>
                </>
              )}
            </For>
          </div>

          <Show when={hasMore()}>
            <button
              class="mt-4 w-full btn btn-secondary text-sm"
              onClick={() => loadTimeline(false)}
              disabled={loadingTimeline()}
            >
              {loadingTimeline() ? 'Loading…' : 'Load more'}
            </button>
          </Show>
        </Show>
      </div>

      {/* ============================================================
          Section 3: AI Analysis Panel
          ============================================================ */}
      <div class="card p-4">
        <h3 class="font-semibold text-gray-800 dark:text-gray-100 mb-4">AI Analysis Panel</h3>

        <Show when={reportError()}>
          <p class="text-sm text-red-600 dark:text-red-400 mb-3">{reportError()}</p>
        </Show>

        {/* Consent + Generate */}
        <div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-4 mb-4">
          <label class="flex items-start gap-3 cursor-pointer">
            <input
              type="checkbox"
              class="mt-0.5 h-4 w-4 rounded border-gray-300 text-minion-600 focus:ring-minion-500"
              checked={consentChecked()}
              onChange={(e) => setConsentChecked(e.currentTarget.checked)}
            />
            <span class="text-sm text-gray-700 dark:text-gray-300">
              I understand that clicking "Generate Doctor Report" will send a structured
              summary of my health timeline to the configured LLM endpoint for analysis.
              No raw document text or images are sent — only structured event data.
            </span>
          </label>

          <button
            class="mt-3 btn btn-primary w-full"
            onClick={generateReport}
            disabled={!consentChecked() || reportLoading()}
          >
            {reportLoading()
              ? 'Generating report… (may take up to 90s)'
              : 'Generate Doctor Report'}
          </button>

          <Show when={reportLoading()}>
            <div class="mt-3 flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400">
              <span class="inline-block w-4 h-4 border-2 border-minion-500 border-t-transparent rounded-full animate-spin" />
              <span>The AI is composing your health summary…</span>
            </div>
          </Show>
        </div>

        {/* Report history */}
        <Show
          when={reports().length > 0}
          fallback={
            <p class="text-sm text-gray-500 dark:text-gray-400 italic">
              No reports generated yet. Rebuild the timeline, then click "Generate Doctor Report".
            </p>
          }
        >
          <h4 class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">
            Previous Reports
          </h4>
          <div class="space-y-3">
            <For each={reports()}>
              {(report) => {
                const isExpanded = () => expandedReport() === report.id;
                const preview = report.report_text.slice(0, 300);
                const needsTruncation = report.report_text.length > 300;
                return (
                  <div class="border border-gray-200 dark:border-gray-700 rounded-lg p-3">
                    <div class="flex items-center justify-between mb-1">
                      <span class="text-xs text-gray-500 dark:text-gray-400">
                        {fmtDate(report.generated_at)} · {report.model_used}
                      </span>
                      <div class="flex items-center gap-2">
                        <button
                          class="text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 px-2 py-0.5 border border-gray-300 dark:border-gray-600 rounded"
                          onClick={() => copyReport(report.report_text)}
                          title="Copy for doctor"
                        >
                          Copy for doctor
                        </button>
                        <button
                          class="text-xs text-red-500 hover:text-red-700 px-1"
                          onClick={() => deleteReport(report.id)}
                          title="Delete report"
                        >
                          ✕
                        </button>
                      </div>
                    </div>
                    <p class="text-sm text-gray-800 dark:text-gray-200 whitespace-pre-wrap">
                      {isExpanded() ? report.report_text : preview}
                      {!isExpanded() && needsTruncation ? '…' : ''}
                    </p>
                    <Show when={needsTruncation}>
                      <button
                        class="mt-1 text-xs text-minion-600 dark:text-minion-400 hover:underline"
                        onClick={() =>
                          setExpandedReport(isExpanded() ? null : report.id)
                        }
                      >
                        {isExpanded() ? 'Show less' : 'Show full report'}
                      </button>
                    </Show>
                  </div>
                );
              }}
            </For>
          </div>
        </Show>
      </div>

    </div>
  );
};

export default IntelligenceTab;
```

- [ ] **Step 2: Type-check**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep -i "intelligence\|error" | head -20
```

Expected: no errors referencing `IntelligenceTab.tsx`.

- [ ] **Step 3: Lint**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm lint 2>&1 | grep "IntelligenceTab" | head -10
```

Expected: no errors.

**Commit:**
```
feat(health): IntelligenceTab.tsx — anomaly alerts, unified timeline, AI analysis panel
```

---

## Task 6: Wire `Health.tsx`

**Files:**
- Modify: `ui/src/pages/Health.tsx`

- [ ] **Step 1: Add import**

At the top of `ui/src/pages/Health.tsx`, after the `import CloudBackupTab` line, add:

```tsx
import IntelligenceTab from './health/IntelligenceTab';
```

- [ ] **Step 2: Add `'intelligence'` to the `HealthTab` union type**

Find:

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
  | 'intelligence'
  | 'cloud_backup';
```

- [ ] **Step 3: Add tab bar entry**

In the tabs array inside the `map(...)` call, find `['cloud_backup', 'Cloud Backup'],` and insert before it:

```tsx
              ['intelligence', 'Intelligence'],
```

The array should end with:
```tsx
              ['intelligence', 'Intelligence'],
              ['cloud_backup', 'Cloud Backup'],
```

- [ ] **Step 4: Add the `<Show>` block**

Find:

```tsx
        <Show when={activeTab() === 'cloud_backup'}>
```

Insert before it:

```tsx
        <Show when={activeTab() === 'intelligence'}>
          <IntelligenceTab patientId={activePatient()!.id} />
        </Show>
```

- [ ] **Step 5: Type-check and lint**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | tail -5
cd /home/dk/Documents/git/minion/ui && pnpm lint 2>&1 | tail -5
```

Expected: no errors.

**Commit:**
```
feat(health): wire Intelligence tab in Health.tsx
```

---

## Task 7: Rust Compilation and Full Test Suite

**Files:** none (verification only)

- [ ] **Step 1: Clippy**

```bash
cargo clippy -p minion-app -- -D warnings 2>&1 | grep "health_intelligence\|warning\|error" | head -20
```

Expected: no warnings in `health_intelligence.rs`.

- [ ] **Step 2: Full workspace tests**

```bash
cargo test --workspace 2>&1 | tail -20
```

Expected: all tests pass. In particular:
- `test minion_db::migrations::tests::test_migrations_are_recorded ... ok`
- `test health_intelligence::tests::anomaly_id_is_deterministic ... ok`
- `test health_intelligence::tests::anomaly_id_differs_on_different_inputs ... ok`

- [ ] **Step 3: Frontend build**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm build 2>&1 | tail -10
```

Expected: `✓ built in ...` with no errors.

**Commit:** (none — verification task)

---

## Task 8: Integration Smoke Test (Manual)

**Files:** none (manual verification steps)

These steps require a running app (`cargo tauri dev`) with at least one patient in the DB.

- [ ] **Step 1: Start the app**

```bash
cargo tauri dev 2>&1 | head -20
```

Expected: app launches, no panic in the console.

- [ ] **Step 2: Open Health → Intelligence tab**

Navigate to the Health section, click the "Intelligence" tab. Verify:
- Anomaly Alerts section is visible with "↻ Run Detection" button.
- Unified Timeline section shows an empty state message: "No timeline events. Click 'Rebuild Timeline'…".
- AI Analysis Panel shows the consent checkbox and disabled button.

- [ ] **Step 3: Rebuild timeline**

Click "↻ Rebuild Timeline". Verify:
- Loading indicator appears briefly.
- Timeline populates with month headers and category-dot events (if data exists).
- Category filter chips work.
- "Load more" appears only if > 50 events.

- [ ] **Step 4: Run anomaly detection**

Click "↻ Run Detection". Verify:
- With no data, returns empty state "✓ No anomalies detected".
- With test data (insert a CRITICAL lab value in DB), an alert card appears with red left border.

- [ ] **Step 5: Generate report (consent flow)**

Check the consent checkbox — the "Generate Doctor Report" button becomes enabled.
Click it. Verify:
- Spinner appears.
- Report appears in "Previous Reports" with "Copy for doctor" button.
- "Show full report" toggle works.
- Delete (✕) removes the report.

- [ ] **Step 6: Verify no data leaves the device without consent**

Leave the consent checkbox unchecked and attempt to call `health_generate_report` via DevTools:
```javascript
window.__TAURI__.core.invoke('health_generate_report', { patient_id: 'test', consent_confirmed: false })
```
Expected: rejects with `"Consent required: check the box…"`.

---

## Task 9: Add Migration Count Test for Tables

**Files:**
- Modify: `crates/minion-db/src/migrations.rs`

Add a targeted test that verifies the three new tables exist after migration 020 runs.

- [ ] **Step 1: Append the test**

In `crates/minion-db/src/migrations.rs`, inside the `#[cfg(test)]` block (after the last existing test function), add:

```rust
    #[test]
    fn test_migration_020_health_intelligence_tables() {
        let conn = setup_test_db();
        run(&conn).expect("Failed to run migrations");

        // location_visits
        conn.execute(
            "INSERT INTO location_visits (id, patient_id, visit_date, city, source)
             VALUES ('lv1', 'p1', '2025-01-01', 'Chennai', 'document_text')",
            [],
        )
        .expect("location_visits insert failed");

        // health_timeline_events
        conn.execute(
            "INSERT INTO health_timeline_events
             (id, patient_id, event_date, category, title, source_type)
             VALUES ('te1', 'p1', '2025-01-01', 'lab', 'CBC Report', 'structured_lab_result')",
            [],
        )
        .expect("health_timeline_events insert failed");

        // health_intelligence_reports
        conn.execute(
            "INSERT INTO health_intelligence_reports
             (id, patient_id, generated_at, model_used, report_text)
             VALUES ('ir1', 'p1', '2025-01-01T00:00:00Z', 'llama3', 'Summary text')",
            [],
        )
        .expect("health_intelligence_reports insert failed");

        let lv_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM location_visits", [], |r| r.get(0))
            .unwrap();
        assert_eq!(lv_count, 1);

        let te_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM health_timeline_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(te_count, 1);

        let ir_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM health_intelligence_reports", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ir_count, 1);
    }
```

- [ ] **Step 2: Run the new test**

```bash
cargo test -p minion-db -- test_migration_020 2>&1 | tail -5
```

Expected:
```
test migrations::tests::test_migration_020_health_intelligence_tables ... ok
```

- [ ] **Step 3: Run all DB tests**

```bash
cargo test -p minion-db 2>&1 | tail -10
```

Expected: all pass.

**Commit:**
```
test(health): add migration 020 table existence test in minion-db
```

---

## Summary

| Task | Files Changed | Test Command |
|------|---------------|--------------|
| 1 | `crates/minion-db/src/migrations.rs` | `cargo test -p minion-db -- test_migrations_are_recorded` |
| 2 | `src-tauri/src/health_intelligence.rs` (create, header + helpers) | `cargo check -p minion-app` |
| 3 | `src-tauri/src/health_intelligence.rs` (append commands) | `cargo test -p minion-app -- health_intelligence` |
| 4 | `src-tauri/src/lib.rs` | `cargo check -p minion-app` |
| 5 | `ui/src/pages/health/IntelligenceTab.tsx` (create) | `cd ui && pnpm typecheck && pnpm lint` |
| 6 | `ui/src/pages/Health.tsx` | `cd ui && pnpm typecheck && pnpm lint` |
| 7 | (verification) | `cargo test --workspace && cd ui && pnpm build` |
| 8 | (manual smoke test) | `cargo tauri dev` |
| 9 | `crates/minion-db/src/migrations.rs` | `cargo test -p minion-db -- test_migration_020` |
