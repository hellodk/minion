//! Health Vault entity resolution (week 3).
//!
//! Canonicalization + fuzzy matching for doctors, facilities, labs,
//! medications, and lab tests so that "Dr. Smith", "Smith MD", and
//! "smith" all resolve to the same `health_entities` row.
//!
//! Uses a built-in synonym table for the most common drugs and tests, a
//! normalization pass (lowercase, strip titles/punctuation), and the
//! Levenshtein-based `strsim` similarity as a fallback.

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<AppState>>;

// =====================================================================
// Canonical synonym maps
// =====================================================================

/// Common drug brand/salt names mapped to their canonical generic.
/// Keys MUST be lowercased — `canonicalize_drug` lowercases the input.
pub const DRUG_CANONICAL: &[(&str, &str)] = &[
    // Antidiabetic
    ("metformin hcl", "metformin"),
    ("metformin hydrochloride", "metformin"),
    ("glucophage", "metformin"),
    ("glycomet", "metformin"),
    ("obimet", "metformin"),
    ("glimepiride", "glimepiride"),
    ("amaryl", "glimepiride"),
    ("sitagliptin", "sitagliptin"),
    ("januvia", "sitagliptin"),
    // Statins / lipid-lowering
    ("atorvastatin calcium", "atorvastatin"),
    ("atorvastatin", "atorvastatin"),
    ("lipitor", "atorvastatin"),
    ("rosuvastatin calcium", "rosuvastatin"),
    ("rosuvastatin", "rosuvastatin"),
    ("crestor", "rosuvastatin"),
    ("simvastatin", "simvastatin"),
    ("zocor", "simvastatin"),
    // Antihypertensives
    ("amlodipine besylate", "amlodipine"),
    ("amlodipine besilate", "amlodipine"),
    ("amlodipine", "amlodipine"),
    ("norvasc", "amlodipine"),
    ("losartan potassium", "losartan"),
    ("losartan", "losartan"),
    ("cozaar", "losartan"),
    ("telmisartan", "telmisartan"),
    ("micardis", "telmisartan"),
    ("ramipril", "ramipril"),
    ("altace", "ramipril"),
    ("enalapril maleate", "enalapril"),
    ("enalapril", "enalapril"),
    ("metoprolol succinate", "metoprolol"),
    ("metoprolol tartrate", "metoprolol"),
    ("metoprolol", "metoprolol"),
    ("atenolol", "atenolol"),
    // Thyroid
    ("levothyroxine sodium", "levothyroxine"),
    ("levothyroxine", "levothyroxine"),
    ("synthroid", "levothyroxine"),
    ("eltroxin", "levothyroxine"),
    ("thyronorm", "levothyroxine"),
    // Analgesics / NSAIDs
    ("aspirin", "aspirin"),
    ("acetylsalicylic acid", "aspirin"),
    ("ecosprin", "aspirin"),
    ("paracetamol", "paracetamol"),
    ("acetaminophen", "paracetamol"),
    ("tylenol", "paracetamol"),
    ("crocin", "paracetamol"),
    ("ibuprofen", "ibuprofen"),
    ("advil", "ibuprofen"),
    ("brufen", "ibuprofen"),
    ("diclofenac sodium", "diclofenac"),
    ("diclofenac", "diclofenac"),
    ("voltaren", "diclofenac"),
    ("naproxen", "naproxen"),
    // PPIs
    ("omeprazole", "omeprazole"),
    ("prilosec", "omeprazole"),
    ("pantoprazole sodium", "pantoprazole"),
    ("pantoprazole", "pantoprazole"),
    ("pantocid", "pantoprazole"),
    ("esomeprazole", "esomeprazole"),
    ("nexium", "esomeprazole"),
    // Antibiotics
    ("amoxicillin", "amoxicillin"),
    ("amoxycillin", "amoxicillin"),
    ("azithromycin", "azithromycin"),
    ("zithromax", "azithromycin"),
    ("ciprofloxacin", "ciprofloxacin"),
    ("cipro", "ciprofloxacin"),
    ("cefixime", "cefixime"),
    // Anticoagulants
    ("warfarin sodium", "warfarin"),
    ("warfarin", "warfarin"),
    ("clopidogrel bisulfate", "clopidogrel"),
    ("clopidogrel", "clopidogrel"),
    ("plavix", "clopidogrel"),
    // Vitamins / supplements
    ("cholecalciferol", "vitamin d3"),
    ("vitamin d3", "vitamin d3"),
    ("vitamin d", "vitamin d3"),
    ("methylcobalamin", "vitamin b12"),
    ("cyanocobalamin", "vitamin b12"),
    ("vitamin b12", "vitamin b12"),
];

/// Common lab test aliases mapped to their canonical label.
/// Keys MUST be lowercased — `canonicalize_test` lowercases the input.
pub const TEST_CANONICAL: &[(&str, &str)] = &[
    // Diabetes
    ("hba1c", "HbA1c"),
    ("hemoglobin a1c", "HbA1c"),
    ("haemoglobin a1c", "HbA1c"),
    ("glycated hemoglobin", "HbA1c"),
    ("glycosylated hemoglobin", "HbA1c"),
    ("a1c", "HbA1c"),
    ("fasting glucose", "Fasting Glucose"),
    ("fasting blood glucose", "Fasting Glucose"),
    ("fasting blood sugar", "Fasting Glucose"),
    ("fbs", "Fasting Glucose"),
    ("glucose fasting", "Fasting Glucose"),
    ("post prandial glucose", "Postprandial Glucose"),
    ("post-prandial glucose", "Postprandial Glucose"),
    ("postprandial glucose", "Postprandial Glucose"),
    ("ppbs", "Postprandial Glucose"),
    ("random blood sugar", "Random Glucose"),
    ("rbs", "Random Glucose"),
    // Lipid panel
    ("ldl cholesterol", "LDL"),
    ("ldl-c", "LDL"),
    ("ldl", "LDL"),
    ("low density lipoprotein", "LDL"),
    ("hdl cholesterol", "HDL"),
    ("hdl-c", "HDL"),
    ("hdl", "HDL"),
    ("high density lipoprotein", "HDL"),
    ("vldl", "VLDL"),
    ("triglycerides", "Triglycerides"),
    ("tg", "Triglycerides"),
    ("total cholesterol", "Total Cholesterol"),
    ("cholesterol total", "Total Cholesterol"),
    ("cholesterol", "Total Cholesterol"),
    // Thyroid
    ("tsh", "TSH"),
    ("thyroid stimulating hormone", "TSH"),
    ("t3", "T3"),
    ("t4", "T4"),
    ("free t3", "Free T3"),
    ("ft3", "Free T3"),
    ("free t4", "Free T4"),
    ("ft4", "Free T4"),
    // Kidney
    ("creatinine", "Creatinine"),
    ("serum creatinine", "Creatinine"),
    ("bun", "BUN"),
    ("blood urea nitrogen", "BUN"),
    ("urea", "Urea"),
    ("blood urea", "Urea"),
    ("uric acid", "Uric Acid"),
    ("egfr", "eGFR"),
    // Liver
    ("alt", "ALT"),
    ("sgpt", "ALT"),
    ("alanine aminotransferase", "ALT"),
    ("ast", "AST"),
    ("sgot", "AST"),
    ("aspartate aminotransferase", "AST"),
    ("alkaline phosphatase", "ALP"),
    ("alp", "ALP"),
    ("ggt", "GGT"),
    ("bilirubin total", "Total Bilirubin"),
    ("total bilirubin", "Total Bilirubin"),
    ("direct bilirubin", "Direct Bilirubin"),
    ("albumin", "Albumin"),
    ("total protein", "Total Protein"),
    // CBC
    ("hemoglobin", "Hemoglobin"),
    ("haemoglobin", "Hemoglobin"),
    ("hb", "Hemoglobin"),
    ("hgb", "Hemoglobin"),
    ("wbc", "WBC"),
    ("white blood cells", "WBC"),
    ("total leukocyte count", "WBC"),
    ("tlc", "WBC"),
    ("rbc", "RBC"),
    ("red blood cells", "RBC"),
    ("platelets", "Platelets"),
    ("platelet count", "Platelets"),
    ("plt", "Platelets"),
    ("hematocrit", "Hematocrit"),
    ("haematocrit", "Hematocrit"),
    ("hct", "Hematocrit"),
    ("pcv", "Hematocrit"),
    ("mcv", "MCV"),
    ("mch", "MCH"),
    ("mchc", "MCHC"),
    ("esr", "ESR"),
    // Electrolytes
    ("sodium", "Sodium"),
    ("na", "Sodium"),
    ("potassium", "Potassium"),
    ("k", "Potassium"),
    ("chloride", "Chloride"),
    ("calcium", "Calcium"),
    ("magnesium", "Magnesium"),
    ("phosphorus", "Phosphorus"),
    // Vitamins / hormones
    ("vitamin d", "Vitamin D"),
    ("25-hydroxy vitamin d", "Vitamin D"),
    ("25 hydroxy vitamin d", "Vitamin D"),
    ("vitamin b12", "Vitamin B12"),
    ("b12", "Vitamin B12"),
    ("folate", "Folate"),
    ("folic acid", "Folate"),
    ("ferritin", "Ferritin"),
    ("iron", "Iron"),
    ("tibc", "TIBC"),
    // Cardiac / inflammation
    ("crp", "CRP"),
    ("hs-crp", "hs-CRP"),
    ("hscrp", "hs-CRP"),
    ("troponin", "Troponin"),
    ("troponin i", "Troponin I"),
    ("troponin t", "Troponin T"),
];

// =====================================================================
// Normalization helpers
// =====================================================================

/// Lowercase, trim, collapse whitespace, strip titles (Dr., MD, Prof.)
/// and remove non-alphanumeric punctuation.
pub fn normalize_name(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut cleaned = String::with_capacity(lower.len());
    // Strip punctuation -> spaces
    for ch in lower.chars() {
        if ch.is_alphanumeric() || ch.is_whitespace() {
            cleaned.push(ch);
        } else {
            cleaned.push(' ');
        }
    }
    // Collapse whitespace + drop common title/suffix tokens.
    let title_tokens = [
        "dr", "doctor", "prof", "professor", "mr", "mrs", "ms",
        "md", "mbbs", "mds", "do", "dnb", "phd", "rn", "np", "pa",
        "frcs", "frcp", "mrcp", "facp", "facs",
    ];
    let tokens: Vec<&str> = cleaned
        .split_whitespace()
        .filter(|t| !title_tokens.contains(t))
        .collect();
    tokens.join(" ")
}

/// Levenshtein similarity normalized to [0.0, 1.0] (1.0 = identical).
pub fn similarity(a: &str, b: &str) -> f64 {
    strsim::normalized_levenshtein(a, b)
}

/// Look up a lowercased key in a canonical table.
fn lookup_canonical<'a>(key: &str, table: &'a [(&'a str, &'a str)]) -> Option<&'a str> {
    let k = key.to_lowercase();
    table.iter().find(|(alias, _)| *alias == k).map(|(_, c)| *c)
}

/// Title-case a name (first letter of each word uppercased).
fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut capitalize = true;
    for ch in s.chars() {
        if ch.is_whitespace() {
            out.push(ch);
            capitalize = true;
        } else if capitalize {
            for up in ch.to_uppercase() {
                out.push(up);
            }
            capitalize = false;
        } else {
            for low in ch.to_lowercase() {
                out.push(low);
            }
        }
    }
    out
}

/// Canonicalize a drug name: return the known canonical generic if known,
/// otherwise a trimmed lowercased version (so duplicate spellings still
/// collapse even when not in the dictionary).
pub fn canonicalize_drug(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(c) = lookup_canonical(trimmed, DRUG_CANONICAL) {
        return c.to_string();
    }
    // Fall back to a normalized-but-readable representation.
    let norm = normalize_name(trimmed);
    if norm.is_empty() {
        trimmed.to_lowercase()
    } else {
        norm
    }
}

/// Canonicalize a lab test name: return the known canonical label if
/// known, otherwise a title-cased fallback.
pub fn canonicalize_test(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(c) = lookup_canonical(trimmed, TEST_CANONICAL) {
        return c.to_string();
    }
    title_case(trimmed)
}

// =====================================================================
// Entity resolution against the `health_entities` table
// =====================================================================

/// Find-or-create an entity row. Returns the `health_entities.id` to use.
///
/// * `entity_type` — one of `doctor`, `facility`, `lab`, `medication`, `test`.
/// * `raw_name` — the name exactly as it appeared in the document.
/// * `similarity_threshold` — Levenshtein similarity above which we reuse
///   an existing row (0.85 is a sensible default).
///
/// If a row already exists above threshold, appends `raw_name` to its
/// `aliases` JSON array (de-duplicated). Otherwise inserts a new row with
/// `canonical_name` built from the resolver.
pub async fn resolve_entity(
    state: &AppStateHandle,
    entity_type: &str,
    raw_name: &str,
    similarity_threshold: f64,
) -> Result<String, String> {
    let trimmed = raw_name.trim();
    if trimmed.is_empty() {
        return Err("resolve_entity: empty name".into());
    }

    // Compute a canonical form we can use for display + similarity.
    let canonical = match entity_type {
        "medication" => canonicalize_drug(trimmed),
        "test" => canonicalize_test(trimmed),
        _ => {
            let n = normalize_name(trimmed);
            if n.is_empty() {
                trimmed.to_string()
            } else {
                title_case(&n)
            }
        }
    };
    let canonical_norm = normalize_name(&canonical);

    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    // Pull every existing entity of this type to score.
    let mut stmt = conn
        .prepare(
            "SELECT id, canonical_name, aliases FROM health_entities
             WHERE entity_type = ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, String, Option<String>)> = stmt
        .query_map(rusqlite::params![entity_type], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    // Find best match by similarity on normalized names.
    let mut best: Option<(String, String, Option<String>, f64)> = None;
    for (id, existing_canonical, aliases_json) in rows {
        let existing_norm = normalize_name(&existing_canonical);
        let mut score = similarity(&canonical_norm, &existing_norm);
        // Also check aliases so a previously-seen raw name wins even if its
        // canonical spelling has drifted.
        if let Some(j) = &aliases_json {
            if let Ok(v) = serde_json::from_str::<Vec<String>>(j) {
                for alias in v {
                    let alias_norm = normalize_name(&alias);
                    let s = similarity(&canonical_norm, &alias_norm);
                    if s > score {
                        score = s;
                    }
                }
            }
        }
        match &best {
            Some((_, _, _, s)) if *s >= score => {}
            _ => best = Some((id, existing_canonical, aliases_json, score)),
        }
    }

    if let Some((id, _canonical, aliases_json, score)) = best {
        if score >= similarity_threshold {
            // Reuse — add the raw input as an alias if it's new.
            let mut aliases: Vec<String> = aliases_json
                .and_then(|j| serde_json::from_str(&j).ok())
                .unwrap_or_default();
            if !aliases.iter().any(|a| a.eq_ignore_ascii_case(trimmed)) {
                aliases.push(trimmed.to_string());
                let encoded = serde_json::to_string(&aliases).unwrap_or_else(|_| "[]".into());
                let _ = conn.execute(
                    "UPDATE health_entities SET aliases = ?1 WHERE id = ?2",
                    rusqlite::params![encoded, id],
                );
            }
            return Ok(id);
        }
    }

    // No match — insert a new row.
    let id = uuid::Uuid::new_v4().to_string();
    let aliases = vec![trimmed.to_string()];
    let aliases_json = serde_json::to_string(&aliases).unwrap_or_else(|_| "[]".into());
    let now = chrono::Utc::now().to_rfc3339();
    // Collision-safe insert: if (entity_type, canonical_name) already exists,
    // fall back to reading that row's id.
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO health_entities
         (id, entity_type, canonical_name, aliases, first_seen_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, entity_type, canonical, aliases_json, now],
    );
    match inserted {
        Ok(1) => Ok(id),
        Ok(_) => {
            // Unique constraint hit — fetch existing.
            conn.query_row(
                "SELECT id FROM health_entities
                 WHERE entity_type = ?1 AND canonical_name = ?2",
                rusqlite::params![entity_type, canonical],
                |row| row.get::<_, String>(0),
            )
            .map_err(|e| e.to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

// =====================================================================
// Tauri commands
// =====================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct EntityInfo {
    pub id: String,
    pub entity_type: String,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub first_seen_at: Option<String>,
}

#[tauri::command]
pub async fn health_list_entities(
    state: State<'_, AppStateHandle>,
    entity_type: String,
) -> Result<Vec<EntityInfo>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, entity_type, canonical_name, aliases, first_seen_at
             FROM health_entities WHERE entity_type = ?1
             ORDER BY canonical_name ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![entity_type], |row| {
            let aliases_json: Option<String> = row.get(3)?;
            let aliases: Vec<String> = aliases_json
                .and_then(|j| serde_json::from_str(&j).ok())
                .unwrap_or_default();
            Ok(EntityInfo {
                id: row.get(0)?,
                entity_type: row.get(1)?,
                canonical_name: row.get(2)?,
                aliases,
                first_seen_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// Merge `merge_ids` into `keep_id`: repoint foreign keys, move aliases,
/// and delete the merged rows. All FK updates happen in one transaction.
#[tauri::command]
pub async fn health_merge_entities(
    state: State<'_, AppStateHandle>,
    keep_id: String,
    merge_ids: Vec<String>,
) -> Result<(), String> {
    if merge_ids.is_empty() {
        return Ok(());
    }
    if merge_ids.iter().any(|m| m == &keep_id) {
        return Err("keep_id cannot appear in merge_ids".into());
    }

    let st = state.read().await;
    let mut conn = st.db.get().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // Collect existing aliases from the keeper + all merged rows.
    let keeper_aliases: Vec<String> = tx
        .query_row(
            "SELECT aliases FROM health_entities WHERE id = ?1",
            rusqlite::params![keep_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .map_err(|e| e.to_string())?
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default();
    let mut merged_aliases: Vec<String> = keeper_aliases;

    for mid in &merge_ids {
        let (canonical, aliases_json): (String, Option<String>) = tx
            .query_row(
                "SELECT canonical_name, aliases FROM health_entities WHERE id = ?1",
                rusqlite::params![mid],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        if !merged_aliases
            .iter()
            .any(|a| a.eq_ignore_ascii_case(&canonical))
        {
            merged_aliases.push(canonical);
        }
        if let Some(j) = aliases_json {
            if let Ok(v) = serde_json::from_str::<Vec<String>>(&j) {
                for a in v {
                    if !merged_aliases.iter().any(|x| x.eq_ignore_ascii_case(&a)) {
                        merged_aliases.push(a);
                    }
                }
            }
        }
    }
    let aliases_encoded =
        serde_json::to_string(&merged_aliases).unwrap_or_else(|_| "[]".into());

    // Repoint foreign keys in every table that references health_entities.
    let fk_updates: &[(&str, &str)] = &[
        ("medical_records", "doctor_id"),
        ("medical_records", "facility_id"),
        ("lab_tests", "lab_entity_id"),
        ("medications_v2", "prescribing_doctor_id"),
    ];

    for mid in &merge_ids {
        for (table, col) in fk_updates {
            let sql = format!(
                "UPDATE {table} SET {col} = ?1 WHERE {col} = ?2",
                table = table,
                col = col
            );
            tx.execute(&sql, rusqlite::params![keep_id, mid])
                .map_err(|e| e.to_string())?;
        }
    }

    // Update keeper aliases, then delete merged rows.
    tx.execute(
        "UPDATE health_entities SET aliases = ?1 WHERE id = ?2",
        rusqlite::params![aliases_encoded, keep_id],
    )
    .map_err(|e| e.to_string())?;

    for mid in &merge_ids {
        tx.execute(
            "DELETE FROM health_entities WHERE id = ?1",
            rusqlite::params![mid],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_titles_and_punctuation() {
        assert_eq!(normalize_name("Dr. John Smith, MD"), "john smith");
        assert_eq!(normalize_name("  Prof.   Jane  Doe  PhD "), "jane doe");
        assert_eq!(normalize_name("Mrs Alice-Brown"), "alice brown");
    }

    #[test]
    fn canonicalize_drug_known_and_unknown() {
        assert_eq!(canonicalize_drug("Metformin HCl"), "metformin");
        assert_eq!(canonicalize_drug("GLUCOPHAGE"), "metformin");
        assert_eq!(canonicalize_drug("Lipitor"), "atorvastatin");
        assert_eq!(canonicalize_drug("Ecosprin"), "aspirin");
        // Unknown drug should still normalize consistently.
        let unknown = canonicalize_drug("Frobnitol 500mg");
        assert_eq!(unknown, canonicalize_drug("  frobnitol 500mg "));
    }

    #[test]
    fn canonicalize_test_known_and_unknown() {
        assert_eq!(canonicalize_test("HbA1c"), "HbA1c");
        assert_eq!(canonicalize_test("A1C"), "HbA1c");
        assert_eq!(canonicalize_test("hemoglobin a1c"), "HbA1c");
        assert_eq!(canonicalize_test("LDL Cholesterol"), "LDL");
        assert_eq!(canonicalize_test("tsh"), "TSH");
        // Unknown should be title-cased.
        assert_eq!(canonicalize_test("some new marker"), "Some New Marker");
    }

    #[test]
    fn similarity_same_and_diff() {
        assert!((similarity("metformin", "metformin") - 1.0).abs() < 1e-9);
        assert!(similarity("metformin", "metformine") > 0.85);
        assert!(similarity("metformin", "aspirin") < 0.5);
    }
}
