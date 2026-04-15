//! Health Vault Tauri commands.
//!
//! Multi-patient longitudinal medical records: patients, medical records,
//! lab tests, medications, conditions, vitals, family history, life events,
//! and symptoms. Week 1 = CRUD only (no AI).

use crate::state::AppState;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<AppState>>;

// =====================================================================
// CONSENT
// =====================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthConsent {
    pub id: i64,
    pub accepted_at: String,
    pub version: String,
    pub local_only_mode: bool,
    pub drive_sync_enabled: bool,
    pub cloud_llm_allowed: bool,
}

#[tauri::command]
pub async fn health_get_consent(
    state: State<'_, AppStateHandle>,
) -> Result<Option<HealthConsent>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let result = conn.query_row(
        "SELECT id, accepted_at, version, local_only_mode, drive_sync_enabled,
         cloud_llm_allowed FROM health_consent ORDER BY id DESC LIMIT 1",
        [],
        |row| {
            Ok(HealthConsent {
                id: row.get(0)?,
                accepted_at: row.get(1)?,
                version: row.get(2)?,
                local_only_mode: row.get::<_, i64>(3)? != 0,
                drive_sync_enabled: row.get::<_, i64>(4)? != 0,
                cloud_llm_allowed: row.get::<_, i64>(5)? != 0,
            })
        },
    );
    match result {
        Ok(c) => Ok(Some(c)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn health_accept_consent(
    state: State<'_, AppStateHandle>,
    local_only_mode: bool,
    drive_sync_enabled: bool,
    cloud_llm_allowed: bool,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO health_consent
         (accepted_at, version, local_only_mode, drive_sync_enabled,
          cloud_llm_allowed, user_signature)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            Utc::now().to_rfc3339(),
            "1.0",
            local_only_mode as i64,
            drive_sync_enabled as i64,
            cloud_llm_allowed as i64,
            uuid::Uuid::new_v4().to_string(),
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
// PATIENTS
// =====================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Patient {
    pub id: String,
    pub phone_number: String,
    pub full_name: String,
    pub date_of_birth: Option<String>,
    pub sex: Option<String>,
    pub blood_group: Option<String>,
    pub relationship: String,
    pub is_primary: bool,
    pub avatar_color: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn row_to_patient(row: &rusqlite::Row<'_>) -> rusqlite::Result<Patient> {
    Ok(Patient {
        id: row.get(0)?,
        phone_number: row.get(1)?,
        full_name: row.get(2)?,
        date_of_birth: row.get(3)?,
        sex: row.get(4)?,
        blood_group: row.get(5)?,
        relationship: row.get(6)?,
        is_primary: row.get::<_, i64>(7)? != 0,
        avatar_color: row.get(8)?,
        notes: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

const PATIENT_COLUMNS: &str = "id, phone_number, full_name, date_of_birth, sex,
    blood_group, relationship, is_primary, avatar_color, notes, created_at, updated_at";

#[derive(Debug, Deserialize)]
pub struct CreatePatientRequest {
    pub phone_number: String,
    pub full_name: String,
    pub date_of_birth: Option<String>,
    pub sex: Option<String>,
    pub blood_group: Option<String>,
    pub relationship: String,
    pub is_primary: bool,
    pub avatar_color: Option<String>,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn health_create_patient(
    state: State<'_, AppStateHandle>,
    request: CreatePatientRequest,
) -> Result<Patient, String> {
    let st = state.read().await;
    let mut conn = st.db.get().map_err(|e| e.to_string())?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    // Wrap the unset-old-primary + insert in a single transaction so two
    // racing "add as primary" requests can't both win the unique
    // is_primary index.
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if request.is_primary {
        tx.execute("UPDATE patients SET is_primary = 0", [])
            .map_err(|e| e.to_string())?;
    }
    tx.execute(
        "INSERT INTO patients (id, phone_number, full_name, date_of_birth, sex,
         blood_group, relationship, is_primary, avatar_color, notes,
         created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
        rusqlite::params![
            id,
            request.phone_number,
            request.full_name,
            request.date_of_birth,
            request.sex,
            request.blood_group,
            request.relationship,
            request.is_primary as i64,
            request.avatar_color,
            request.notes,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;

    let q = format!("SELECT {} FROM patients WHERE id = ?1", PATIENT_COLUMNS);
    conn.query_row(&q, rusqlite::params![id], row_to_patient)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn health_list_patients(
    state: State<'_, AppStateHandle>,
) -> Result<Vec<Patient>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let q = format!(
        "SELECT {} FROM patients ORDER BY is_primary DESC, created_at ASC",
        PATIENT_COLUMNS
    );
    let mut stmt = conn.prepare(&q).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], row_to_patient)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub async fn health_get_primary_patient(
    state: State<'_, AppStateHandle>,
) -> Result<Option<Patient>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let q = format!(
        "SELECT {} FROM patients WHERE is_primary = 1 LIMIT 1",
        PATIENT_COLUMNS
    );
    match conn.query_row(&q, [], row_to_patient) {
        Ok(p) => Ok(Some(p)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn health_delete_patient(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM patients WHERE id = ?1",
        rusqlite::params![patient_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
// MEDICAL RECORDS
// =====================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct MedicalRecord {
    pub id: String,
    pub patient_id: String,
    pub record_type: String,
    pub title: String,
    pub description: Option<String>,
    pub date: String,
    pub tags: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateMedicalRecordRequest {
    pub patient_id: String,
    pub record_type: String,
    pub title: String,
    pub description: Option<String>,
    pub date: String,
    pub tags: Option<String>,
}

#[tauri::command]
pub async fn health_create_record(
    state: State<'_, AppStateHandle>,
    request: CreateMedicalRecordRequest,
) -> Result<MedicalRecord, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO medical_records (id, patient_id, record_type, title,
         description, date, tags, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            id,
            request.patient_id,
            request.record_type,
            request.title,
            request.description,
            request.date,
            request.tags,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(MedicalRecord {
        id,
        patient_id: request.patient_id,
        record_type: request.record_type,
        title: request.title,
        description: request.description,
        date: request.date,
        tags: request.tags,
        created_at: now,
    })
}

#[tauri::command]
pub async fn health_list_records(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<MedicalRecord>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, patient_id, record_type, title, description, date, tags, created_at
             FROM medical_records WHERE patient_id = ?1 ORDER BY date DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![patient_id], |row| {
            Ok(MedicalRecord {
                id: row.get(0)?,
                patient_id: row.get(1)?,
                record_type: row.get(2)?,
                title: row.get(3)?,
                description: row.get(4)?,
                date: row.get(5)?,
                tags: row.get(6)?,
                created_at: row.get(7)?,
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
pub async fn health_delete_record(
    state: State<'_, AppStateHandle>,
    record_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM medical_records WHERE id = ?1",
        rusqlite::params![record_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
// LAB TESTS
// =====================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct LabTest {
    pub id: String,
    pub patient_id: String,
    pub test_name: String,
    pub canonical_name: Option<String>,
    pub test_category: Option<String>,
    pub value: f64,
    pub unit: Option<String>,
    pub reference_low: Option<f64>,
    pub reference_high: Option<f64>,
    pub reference_text: Option<String>,
    pub flag: Option<String>,
    pub collected_at: String,
    pub source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLabTestRequest {
    pub patient_id: String,
    pub test_name: String,
    pub canonical_name: Option<String>,
    pub test_category: Option<String>,
    pub value: f64,
    pub unit: Option<String>,
    pub reference_low: Option<f64>,
    pub reference_high: Option<f64>,
    pub reference_text: Option<String>,
    pub flag: Option<String>,
    pub collected_at: String,
}

#[tauri::command]
pub async fn health_create_lab_test(
    state: State<'_, AppStateHandle>,
    request: CreateLabTestRequest,
) -> Result<LabTest, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let source = "manual".to_string();
    conn.execute(
        "INSERT INTO lab_tests (id, patient_id, test_name, canonical_name,
         test_category, value, unit, reference_low, reference_high,
         reference_text, flag, collected_at, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        rusqlite::params![
            id,
            request.patient_id,
            request.test_name,
            request.canonical_name,
            request.test_category,
            request.value,
            request.unit,
            request.reference_low,
            request.reference_high,
            request.reference_text,
            request.flag,
            request.collected_at,
            source,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(LabTest {
        id,
        patient_id: request.patient_id,
        test_name: request.test_name,
        canonical_name: request.canonical_name,
        test_category: request.test_category,
        value: request.value,
        unit: request.unit,
        reference_low: request.reference_low,
        reference_high: request.reference_high,
        reference_text: request.reference_text,
        flag: request.flag,
        collected_at: request.collected_at,
        source: Some(source),
    })
}

#[tauri::command]
pub async fn health_list_lab_tests(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    category: Option<String>,
) -> Result<Vec<LabTest>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let q = if category.is_some() {
        "SELECT id, patient_id, test_name, canonical_name, test_category, value,
         unit, reference_low, reference_high, reference_text, flag, collected_at, source
         FROM lab_tests WHERE patient_id = ?1 AND test_category = ?2
         ORDER BY collected_at DESC"
    } else {
        "SELECT id, patient_id, test_name, canonical_name, test_category, value,
         unit, reference_low, reference_high, reference_text, flag, collected_at, source
         FROM lab_tests WHERE patient_id = ?1 ORDER BY collected_at DESC"
    };
    let mut stmt = conn.prepare(q).map_err(|e| e.to_string())?;
    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<LabTest> {
        Ok(LabTest {
            id: row.get(0)?,
            patient_id: row.get(1)?,
            test_name: row.get(2)?,
            canonical_name: row.get(3)?,
            test_category: row.get(4)?,
            value: row.get(5)?,
            unit: row.get(6)?,
            reference_low: row.get(7)?,
            reference_high: row.get(8)?,
            reference_text: row.get(9)?,
            flag: row.get(10)?,
            collected_at: row.get(11)?,
            source: row.get(12)?,
        })
    };
    let result: Result<Vec<LabTest>, rusqlite::Error> = if let Some(ref cat) = category {
        stmt.query_map(rusqlite::params![patient_id, cat], map_row)
            .and_then(|rows| rows.collect())
    } else {
        stmt.query_map(rusqlite::params![patient_id], map_row)
            .and_then(|rows| rows.collect())
    };
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn health_list_test_names(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<String>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT COALESCE(canonical_name, test_name) FROM lab_tests
             WHERE patient_id = ?1 ORDER BY 1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![patient_id], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub async fn health_delete_lab_test(
    state: State<'_, AppStateHandle>,
    test_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM lab_tests WHERE id = ?1",
        rusqlite::params![test_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
// MEDICATIONS
// =====================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct Medication {
    pub id: String,
    pub patient_id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub dose: Option<String>,
    pub frequency: Option<String>,
    pub route: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub indication: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMedicationRequest {
    pub patient_id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub dose: Option<String>,
    pub frequency: Option<String>,
    pub route: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub indication: Option<String>,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn health_create_medication(
    state: State<'_, AppStateHandle>,
    request: CreateMedicationRequest,
) -> Result<Medication, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO medications_v2 (id, patient_id, name, generic_name, dose,
         frequency, route, start_date, end_date, indication, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            id,
            request.patient_id,
            request.name,
            request.generic_name,
            request.dose,
            request.frequency,
            request.route,
            request.start_date,
            request.end_date,
            request.indication,
            request.notes,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(Medication {
        id,
        patient_id: request.patient_id,
        name: request.name,
        generic_name: request.generic_name,
        dose: request.dose,
        frequency: request.frequency,
        route: request.route,
        start_date: request.start_date,
        end_date: request.end_date,
        indication: request.indication,
        notes: request.notes,
    })
}

#[tauri::command]
pub async fn health_list_medications(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    active_only: bool,
) -> Result<Vec<Medication>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let q = if active_only {
        "SELECT id, patient_id, name, generic_name, dose, frequency, route,
         start_date, end_date, indication, notes
         FROM medications_v2 WHERE patient_id = ?1 AND end_date IS NULL
         ORDER BY start_date DESC"
    } else {
        "SELECT id, patient_id, name, generic_name, dose, frequency, route,
         start_date, end_date, indication, notes
         FROM medications_v2 WHERE patient_id = ?1
         ORDER BY COALESCE(end_date, '9999') DESC, start_date DESC"
    };
    let mut stmt = conn.prepare(q).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![patient_id], |row| {
            Ok(Medication {
                id: row.get(0)?,
                patient_id: row.get(1)?,
                name: row.get(2)?,
                generic_name: row.get(3)?,
                dose: row.get(4)?,
                frequency: row.get(5)?,
                route: row.get(6)?,
                start_date: row.get(7)?,
                end_date: row.get(8)?,
                indication: row.get(9)?,
                notes: row.get(10)?,
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
pub async fn health_delete_medication(
    state: State<'_, AppStateHandle>,
    medication_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM medications_v2 WHERE id = ?1",
        rusqlite::params![medication_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
// CONDITIONS
// =====================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthCondition {
    pub id: String,
    pub patient_id: String,
    pub name: String,
    pub condition_type: Option<String>,
    pub severity: Option<String>,
    pub diagnosed_at: Option<String>,
    pub resolved_at: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateConditionRequest {
    pub patient_id: String,
    pub name: String,
    pub condition_type: Option<String>,
    pub severity: Option<String>,
    pub diagnosed_at: Option<String>,
    pub resolved_at: Option<String>,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn health_create_condition(
    state: State<'_, AppStateHandle>,
    request: CreateConditionRequest,
) -> Result<HealthCondition, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO health_conditions (id, patient_id, name, condition_type,
         severity, diagnosed_at, resolved_at, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            id,
            request.patient_id,
            request.name,
            request.condition_type,
            request.severity,
            request.diagnosed_at,
            request.resolved_at,
            request.notes,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(HealthCondition {
        id,
        patient_id: request.patient_id,
        name: request.name,
        condition_type: request.condition_type,
        severity: request.severity,
        diagnosed_at: request.diagnosed_at,
        resolved_at: request.resolved_at,
        notes: request.notes,
    })
}

#[tauri::command]
pub async fn health_list_conditions(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<HealthCondition>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, patient_id, name, condition_type, severity,
             diagnosed_at, resolved_at, notes
             FROM health_conditions WHERE patient_id = ?1
             ORDER BY diagnosed_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![patient_id], |row| {
            Ok(HealthCondition {
                id: row.get(0)?,
                patient_id: row.get(1)?,
                name: row.get(2)?,
                condition_type: row.get(3)?,
                severity: row.get(4)?,
                diagnosed_at: row.get(5)?,
                resolved_at: row.get(6)?,
                notes: row.get(7)?,
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
pub async fn health_delete_condition(
    state: State<'_, AppStateHandle>,
    condition_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM health_conditions WHERE id = ?1",
        rusqlite::params![condition_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
// VITALS
// =====================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct Vital {
    pub id: String,
    pub patient_id: String,
    pub measurement_type: String,
    pub value: f64,
    pub unit: Option<String>,
    pub measured_at: String,
    pub context: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateVitalRequest {
    pub patient_id: String,
    pub measurement_type: String,
    pub value: f64,
    pub unit: Option<String>,
    pub measured_at: String,
    pub context: Option<String>,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn health_create_vital(
    state: State<'_, AppStateHandle>,
    request: CreateVitalRequest,
) -> Result<Vital, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO vitals (id, patient_id, measurement_type, value, unit,
         measured_at, context, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            id,
            request.patient_id,
            request.measurement_type,
            request.value,
            request.unit,
            request.measured_at,
            request.context,
            request.notes,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(Vital {
        id,
        patient_id: request.patient_id,
        measurement_type: request.measurement_type,
        value: request.value,
        unit: request.unit,
        measured_at: request.measured_at,
        context: request.context,
        notes: request.notes,
    })
}

#[tauri::command]
pub async fn health_list_vitals(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    measurement_type: Option<String>,
) -> Result<Vec<Vital>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let q = if measurement_type.is_some() {
        "SELECT id, patient_id, measurement_type, value, unit, measured_at,
         context, notes
         FROM vitals WHERE patient_id = ?1 AND measurement_type = ?2
         ORDER BY measured_at DESC"
    } else {
        "SELECT id, patient_id, measurement_type, value, unit, measured_at,
         context, notes
         FROM vitals WHERE patient_id = ?1 ORDER BY measured_at DESC"
    };
    let mut stmt = conn.prepare(q).map_err(|e| e.to_string())?;
    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<Vital> {
        Ok(Vital {
            id: row.get(0)?,
            patient_id: row.get(1)?,
            measurement_type: row.get(2)?,
            value: row.get(3)?,
            unit: row.get(4)?,
            measured_at: row.get(5)?,
            context: row.get(6)?,
            notes: row.get(7)?,
        })
    };
    let result: Result<Vec<Vital>, rusqlite::Error> = if let Some(ref mt) = measurement_type {
        stmt.query_map(rusqlite::params![patient_id, mt], map_row)
            .and_then(|rows| rows.collect())
    } else {
        stmt.query_map(rusqlite::params![patient_id], map_row)
            .and_then(|rows| rows.collect())
    };
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn health_delete_vital(
    state: State<'_, AppStateHandle>,
    vital_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM vitals WHERE id = ?1",
        rusqlite::params![vital_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
// FAMILY HISTORY
// =====================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct FamilyHistoryEntry {
    pub id: String,
    pub patient_id: String,
    pub relation: String,
    pub condition: String,
    pub age_at_diagnosis: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFamilyHistoryRequest {
    pub patient_id: String,
    pub relation: String,
    pub condition: String,
    pub age_at_diagnosis: Option<i64>,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn health_create_family_history(
    state: State<'_, AppStateHandle>,
    request: CreateFamilyHistoryRequest,
) -> Result<FamilyHistoryEntry, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO family_history (id, patient_id, relation, condition,
         age_at_diagnosis, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            id,
            request.patient_id,
            request.relation,
            request.condition,
            request.age_at_diagnosis,
            request.notes,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(FamilyHistoryEntry {
        id,
        patient_id: request.patient_id,
        relation: request.relation,
        condition: request.condition,
        age_at_diagnosis: request.age_at_diagnosis,
        notes: request.notes,
    })
}

#[tauri::command]
pub async fn health_list_family_history(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<FamilyHistoryEntry>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, patient_id, relation, condition, age_at_diagnosis, notes
             FROM family_history WHERE patient_id = ?1 ORDER BY relation",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![patient_id], |row| {
            Ok(FamilyHistoryEntry {
                id: row.get(0)?,
                patient_id: row.get(1)?,
                relation: row.get(2)?,
                condition: row.get(3)?,
                age_at_diagnosis: row.get(4)?,
                notes: row.get(5)?,
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
pub async fn health_delete_family_history(
    state: State<'_, AppStateHandle>,
    entry_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM family_history WHERE id = ?1",
        rusqlite::params![entry_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
// LIFE EVENTS
// =====================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct LifeEvent {
    pub id: String,
    pub patient_id: String,
    pub category: String,
    pub subcategory: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub intensity: Option<i64>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub tags: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLifeEventRequest {
    pub patient_id: String,
    pub category: String,
    pub subcategory: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub intensity: Option<i64>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub tags: Option<String>,
}

#[tauri::command]
pub async fn health_create_life_event(
    state: State<'_, AppStateHandle>,
    request: CreateLifeEventRequest,
) -> Result<LifeEvent, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO life_events (id, patient_id, category, subcategory,
         title, description, intensity, started_at, ended_at, tags)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            id,
            request.patient_id,
            request.category,
            request.subcategory,
            request.title,
            request.description,
            request.intensity,
            request.started_at,
            request.ended_at,
            request.tags,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(LifeEvent {
        id,
        patient_id: request.patient_id,
        category: request.category,
        subcategory: request.subcategory,
        title: request.title,
        description: request.description,
        intensity: request.intensity,
        started_at: request.started_at,
        ended_at: request.ended_at,
        tags: request.tags,
    })
}

#[tauri::command]
pub async fn health_list_life_events(
    state: State<'_, AppStateHandle>,
    patient_id: String,
    category: Option<String>,
) -> Result<Vec<LifeEvent>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let q = if category.is_some() {
        "SELECT id, patient_id, category, subcategory, title, description,
         intensity, started_at, ended_at, tags
         FROM life_events WHERE patient_id = ?1 AND category = ?2
         ORDER BY started_at DESC"
    } else {
        "SELECT id, patient_id, category, subcategory, title, description,
         intensity, started_at, ended_at, tags
         FROM life_events WHERE patient_id = ?1 ORDER BY started_at DESC"
    };
    let mut stmt = conn.prepare(q).map_err(|e| e.to_string())?;
    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<LifeEvent> {
        Ok(LifeEvent {
            id: row.get(0)?,
            patient_id: row.get(1)?,
            category: row.get(2)?,
            subcategory: row.get(3)?,
            title: row.get(4)?,
            description: row.get(5)?,
            intensity: row.get(6)?,
            started_at: row.get(7)?,
            ended_at: row.get(8)?,
            tags: row.get(9)?,
        })
    };
    let result: Result<Vec<LifeEvent>, rusqlite::Error> = if let Some(ref c) = category {
        stmt.query_map(rusqlite::params![patient_id, c], map_row)
            .and_then(|rows| rows.collect())
    } else {
        stmt.query_map(rusqlite::params![patient_id], map_row)
            .and_then(|rows| rows.collect())
    };
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn health_delete_life_event(
    state: State<'_, AppStateHandle>,
    event_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM life_events WHERE id = ?1",
        rusqlite::params![event_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
// SYMPTOMS
// =====================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct Symptom {
    pub id: String,
    pub patient_id: String,
    pub description: String,
    pub canonical_name: Option<String>,
    pub body_part: Option<String>,
    pub severity: Option<i64>,
    pub first_noticed: String,
    pub resolved_at: Option<String>,
    pub frequency: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSymptomRequest {
    pub patient_id: String,
    pub description: String,
    pub severity: Option<i64>,
    pub first_noticed: String,
    pub frequency: Option<String>,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn health_create_symptom(
    state: State<'_, AppStateHandle>,
    request: CreateSymptomRequest,
) -> Result<Symptom, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO symptoms (id, patient_id, description, severity,
         first_noticed, frequency, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            id,
            request.patient_id,
            request.description,
            request.severity,
            request.first_noticed,
            request.frequency,
            request.notes,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(Symptom {
        id,
        patient_id: request.patient_id,
        description: request.description,
        canonical_name: None,
        body_part: None,
        severity: request.severity,
        first_noticed: request.first_noticed,
        resolved_at: None,
        frequency: request.frequency,
        notes: request.notes,
    })
}

#[tauri::command]
pub async fn health_list_symptoms(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<Symptom>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, patient_id, description, canonical_name, body_part,
             severity, first_noticed, resolved_at, frequency, notes
             FROM symptoms WHERE patient_id = ?1 ORDER BY first_noticed DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![patient_id], |row| {
            Ok(Symptom {
                id: row.get(0)?,
                patient_id: row.get(1)?,
                description: row.get(2)?,
                canonical_name: row.get(3)?,
                body_part: row.get(4)?,
                severity: row.get(5)?,
                first_noticed: row.get(6)?,
                resolved_at: row.get(7)?,
                frequency: row.get(8)?,
                notes: row.get(9)?,
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
pub async fn health_resolve_symptom(
    state: State<'_, AppStateHandle>,
    symptom_id: String,
    resolved_at: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE symptoms SET resolved_at = ?1 WHERE id = ?2",
        rusqlite::params![resolved_at, symptom_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn health_delete_symptom(
    state: State<'_, AppStateHandle>,
    symptom_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM symptoms WHERE id = ?1",
        rusqlite::params![symptom_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
