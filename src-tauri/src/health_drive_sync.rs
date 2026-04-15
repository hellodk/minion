//! Health Vault week 5: Google Drive backup/restore.
//!
//! Strategy: zero-knowledge backup to the user's own Google Drive
//! `drive.appdata` hidden folder. The full health DB content (patients +
//! all per-event tables) is exported to JSON, encrypted with a
//! passphrase-derived AES-256-GCM key, then uploaded as a single blob
//! named `health_vault_backup.minion`.
//!
//! Drive scope `drive.appdata` is the smallest scope that lets us write a
//! single hidden file private to MINION; Google's UI never shows this file
//! and other apps cannot read it. The encryption passphrase never leaves
//! the device, so even Google can't decrypt the backup.
//!
//! UX is deliberately manual: the user clicks "Backup now" and "Restore
//! from cloud" rather than hidden auto-sync, because conflict resolution
//! across devices is Hard™ and weekly checkpoints are sufficient for
//! medical records.

use crate::state::AppState;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chrono::Utc;
use minion_crypto::{decrypt, encrypt, MasterKey};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<AppState>>;

const BACKUP_FILENAME: &str = "health_vault_backup.minion";
const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata";
const LOOPBACK_REDIRECT: &str = "http://127.0.0.1:8746/";
const LOOPBACK_PORT: u16 = 8746;

// =====================================================================
// Backup payload (what we encrypt and upload)
// =====================================================================

#[derive(Debug, Serialize, Deserialize)]
struct BackupPayload {
    schema_version: u32,
    exported_at: String,
    patients: Vec<serde_json::Value>,
    medical_records: Vec<serde_json::Value>,
    lab_tests: Vec<serde_json::Value>,
    medications_v2: Vec<serde_json::Value>,
    health_conditions: Vec<serde_json::Value>,
    vitals: Vec<serde_json::Value>,
    family_history: Vec<serde_json::Value>,
    life_events: Vec<serde_json::Value>,
    symptoms: Vec<serde_json::Value>,
    health_entities: Vec<serde_json::Value>,
    episodes: Vec<serde_json::Value>,
}

fn dump_table(
    conn: &rusqlite::Connection,
    sql: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let col_names: Vec<String> = stmt
        .column_names()
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let col_count = col_names.len();
    let rows = stmt
        .query_map([], |row| {
            let mut obj = serde_json::Map::with_capacity(col_count);
            for (i, name) in col_names.iter().enumerate() {
                let v = row.get_ref(i)?;
                let json = match v {
                    rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                    rusqlite::types::ValueRef::Integer(n) => {
                        serde_json::Value::Number(n.into())
                    }
                    rusqlite::types::ValueRef::Real(f) => serde_json::Number::from_f64(f)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null),
                    rusqlite::types::ValueRef::Text(t) => {
                        serde_json::Value::String(String::from_utf8_lossy(t).into_owned())
                    }
                    rusqlite::types::ValueRef::Blob(b) => {
                        serde_json::Value::String(B64.encode(b))
                    }
                };
                obj.insert(name.clone(), json);
            }
            Ok(serde_json::Value::Object(obj))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn export_payload(conn: &rusqlite::Connection) -> Result<BackupPayload, String> {
    Ok(BackupPayload {
        schema_version: 1,
        exported_at: Utc::now().to_rfc3339(),
        patients: dump_table(conn, "SELECT * FROM patients")?,
        medical_records: dump_table(conn, "SELECT * FROM medical_records")?,
        lab_tests: dump_table(conn, "SELECT * FROM lab_tests")?,
        medications_v2: dump_table(conn, "SELECT * FROM medications_v2")?,
        health_conditions: dump_table(conn, "SELECT * FROM health_conditions")?,
        vitals: dump_table(conn, "SELECT * FROM vitals")?,
        family_history: dump_table(conn, "SELECT * FROM family_history")?,
        life_events: dump_table(conn, "SELECT * FROM life_events")?,
        symptoms: dump_table(conn, "SELECT * FROM symptoms")?,
        health_entities: dump_table(conn, "SELECT * FROM health_entities")?,
        episodes: dump_table(conn, "SELECT * FROM episodes")?,
    })
}

/// Conflict policy: by default we INSERT OR REPLACE on primary key, which
/// means the cloud backup wins. The user is warned in the UI before they
/// run a restore.
fn restore_payload(
    conn: &rusqlite::Connection,
    payload: &BackupPayload,
) -> Result<RestoreSummary, String> {
    let mut summary = RestoreSummary::default();
    fn upsert(
        conn: &rusqlite::Connection,
        table: &str,
        rows: &[serde_json::Value],
        counter: &mut u64,
    ) -> Result<(), String> {
        for row in rows {
            let obj = row.as_object().ok_or("non-object row")?;
            let cols: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
            let placeholders: Vec<String> =
                (1..=cols.len()).map(|i| format!("?{i}")).collect();
            let sql = format!(
                "INSERT OR REPLACE INTO {} ({}) VALUES ({})",
                table,
                cols.join(", "),
                placeholders.join(", ")
            );
            let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
            // Bind values.
            let values: Vec<Box<dyn rusqlite::ToSql>> = cols
                .iter()
                .map(|c| -> Box<dyn rusqlite::ToSql> {
                    let v = &obj[*c];
                    match v {
                        serde_json::Value::Null => Box::new(Option::<String>::None),
                        serde_json::Value::Bool(b) => Box::new(*b as i64),
                        serde_json::Value::Number(n) => {
                            if let Some(i) = n.as_i64() {
                                Box::new(i)
                            } else if let Some(f) = n.as_f64() {
                                Box::new(f)
                            } else {
                                Box::new(Option::<String>::None)
                            }
                        }
                        serde_json::Value::String(s) => Box::new(s.clone()),
                        _ => Box::new(v.to_string()),
                    }
                })
                .collect();
            let refs: Vec<&dyn rusqlite::ToSql> = values.iter().map(|b| b.as_ref()).collect();
            stmt.execute(refs.as_slice()).map_err(|e| e.to_string())?;
            *counter += 1;
        }
        Ok(())
    }
    upsert(conn, "patients", &payload.patients, &mut summary.patients)?;
    upsert(conn, "health_entities", &payload.health_entities, &mut summary.entities)?;
    upsert(conn, "episodes", &payload.episodes, &mut summary.episodes)?;
    upsert(conn, "medical_records", &payload.medical_records, &mut summary.records)?;
    upsert(conn, "lab_tests", &payload.lab_tests, &mut summary.labs)?;
    upsert(conn, "medications_v2", &payload.medications_v2, &mut summary.medications)?;
    upsert(conn, "health_conditions", &payload.health_conditions, &mut summary.conditions)?;
    upsert(conn, "vitals", &payload.vitals, &mut summary.vitals)?;
    upsert(conn, "family_history", &payload.family_history, &mut summary.family_history)?;
    upsert(conn, "life_events", &payload.life_events, &mut summary.life_events)?;
    upsert(conn, "symptoms", &payload.symptoms, &mut summary.symptoms)?;
    Ok(summary)
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RestoreSummary {
    pub patients: u64,
    pub records: u64,
    pub labs: u64,
    pub medications: u64,
    pub conditions: u64,
    pub vitals: u64,
    pub family_history: u64,
    pub life_events: u64,
    pub symptoms: u64,
    pub entities: u64,
    pub episodes: u64,
}

// =====================================================================
// State and config helpers
// =====================================================================

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DriveSyncStatus {
    pub enabled: bool,
    pub connected: bool,
    pub passphrase_set: bool,
    pub last_synced_at: Option<String>,
    pub last_remote_etag: Option<String>,
    pub error: Option<String>,
    pub remote_file_id: Option<String>,
    pub client_id_set: bool,
}

#[tauri::command]
pub async fn health_drive_status(
    state: State<'_, AppStateHandle>,
) -> Result<DriveSyncStatus, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let row = conn
        .query_row(
            "SELECT enabled, account_id, file_id_remote, last_synced_at,
                    last_remote_etag, error
             FROM drive_sync_state WHERE id = 1",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)? != 0,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .ok();

    let access_token: Option<String> = conn
        .query_row(
            "SELECT value FROM config WHERE key = 'health_drive_access_token'",
            [],
            |r| r.get(0),
        )
        .ok();
    let salt: Option<String> = conn
        .query_row(
            "SELECT value FROM config WHERE key = 'health_drive_salt'",
            [],
            |r| r.get(0),
        )
        .ok();
    let client_id: Option<String> = conn
        .query_row(
            "SELECT value FROM config WHERE key = 'health_drive_client_id'",
            [],
            |r| r.get(0),
        )
        .ok();

    let (enabled, _acct, file_id, last_synced, etag, err) = row.unwrap_or((false, None, None, None, None, None));
    Ok(DriveSyncStatus {
        enabled,
        connected: access_token.is_some(),
        passphrase_set: salt.is_some(),
        last_synced_at: last_synced,
        last_remote_etag: etag,
        error: err,
        remote_file_id: file_id,
        client_id_set: client_id.is_some(),
    })
}

#[tauri::command]
pub async fn health_drive_save_client_id(
    state: State<'_, AppStateHandle>,
    client_id: String,
    client_secret: Option<String>,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO config (key, value) VALUES ('health_drive_client_id', ?1)",
        rusqlite::params![client_id.trim()],
    )
    .map_err(|e| e.to_string())?;
    if let Some(secret) = client_secret {
        conn.execute(
            "INSERT OR REPLACE INTO config (key, value, encrypted) VALUES ('health_drive_client_secret', ?1, 1)",
            rusqlite::params![secret.trim()],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// One-time: derive an encryption key from a passphrase and persist its
/// salt. The passphrase itself is NEVER stored — the user types it again
/// at backup/restore time.
#[tauri::command]
pub async fn health_drive_set_passphrase(
    state: State<'_, AppStateHandle>,
    passphrase: String,
) -> Result<(), String> {
    if passphrase.len() < 8 {
        return Err("passphrase must be at least 8 characters".into());
    }
    let key = MasterKey::derive(&passphrase).map_err(|e| e.to_string())?;
    let salt = key.salt().to_string();
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO config (key, value) VALUES ('health_drive_salt', ?1)",
        rusqlite::params![salt],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn load_encryption_key(
    conn: &rusqlite::Connection,
    passphrase: &str,
) -> Result<[u8; 32], String> {
    let salt: String = conn
        .query_row(
            "SELECT value FROM config WHERE key = 'health_drive_salt'",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "passphrase has not been set; call health_drive_set_passphrase first".to_string())?;
    let key = MasterKey::derive_with_salt(passphrase, &salt).map_err(|e| e.to_string())?;
    Ok(*key.as_bytes())
}

// =====================================================================
// OAuth (loopback redirect, mirrors gfit pattern)
// =====================================================================

fn parse_oauth_callback(buf: &[u8]) -> Result<String, String> {
    let request = String::from_utf8_lossy(buf);
    let first_line = request.lines().next().ok_or("empty request")?;
    let path = first_line.split_whitespace().nth(1).ok_or("bad request line")?;
    let qs = path.split('?').nth(1).ok_or("missing query string")?;
    for pair in qs.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        if k == "code" {
            return Ok(urlencoding::decode(v).map_err(|e| e.to_string())?.into_owned());
        }
        if k == "error" {
            return Err(format!("OAuth error: {}", v));
        }
    }
    Err("no code in callback".into())
}

async fn exchange_code_for_token(
    client_id: &str,
    client_secret: Option<&str>,
    code: &str,
    redirect_uri: &str,
) -> Result<TokenSet, String> {
    let client = reqwest::Client::new();
    let mut form = vec![
        ("client_id", client_id.to_string()),
        ("code", code.to_string()),
        ("grant_type", "authorization_code".to_string()),
        ("redirect_uri", redirect_uri.to_string()),
    ];
    if let Some(s) = client_secret {
        form.push(("client_secret", s.to_string()));
    }
    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&form)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("token exchange failed ({}): {}", status, body));
    }
    let v: TokenSet = resp.json().await.map_err(|e| e.to_string())?;
    Ok(v)
}

#[derive(Debug, Deserialize)]
struct TokenSet {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

#[tauri::command]
pub async fn health_drive_connect(
    app: tauri::AppHandle,
    state: State<'_, AppStateHandle>,
) -> Result<(), String> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::time::Duration;

    let (client_id, client_secret) = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        let cid: String = conn
            .query_row(
                "SELECT value FROM config WHERE key = 'health_drive_client_id'",
                [],
                |r| r.get(0),
            )
            .map_err(|_| {
                "Drive client ID not configured. Save your OAuth client_id under \
                 Health → Cloud Backup first.".to_string()
            })?;
        let csec: Option<String> = conn
            .query_row(
                "SELECT value FROM config WHERE key = 'health_drive_client_secret'",
                [],
                |r| r.get(0),
            )
            .ok();
        (cid, csec)
    };

    let listener = TcpListener::bind(("127.0.0.1", LOOPBACK_PORT))
        .await
        .map_err(|e| {
            format!(
                "Could not listen on 127.0.0.1:{} ({}). Make sure the port is free \
                 and your OAuth client redirect URI is {}",
                LOOPBACK_PORT, e, LOOPBACK_REDIRECT
            )
        })?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Result<String, String>>();
    let task = tokio::spawn(async move {
        let res = tokio::time::timeout(Duration::from_secs(300), listener.accept()).await;
        match res {
            Ok(Ok((mut stream, _))) => {
                let mut buf = vec![0u8; 16_384];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                buf.truncate(n);
                let parse = parse_oauth_callback(&buf);
                let body = if parse.is_ok() {
                    "<!DOCTYPE html><html><body style=\"font-family:system-ui;padding:2rem\">\
                     <p>MINION Health Drive is connected. You can close this window.</p></body></html>"
                } else {
                    "<!DOCTYPE html><html><body style=\"font-family:system-ui;padding:2rem\">\
                     <p>Authorization did not complete.</p></body></html>"
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
                let _ = tx.send(parse);
            }
            Ok(Err(e)) => {
                let _ = tx.send(Err(format!("accept failed: {e}")));
            }
            Err(_) => {
                let _ = tx.send(Err("OAuth timed out (5 minutes)".into()));
            }
        }
    });

    let url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&scope={}&\
         response_type=code&access_type=offline&prompt=consent&redirect_uri={}",
        urlencoding::encode(&client_id),
        urlencoding::encode(DRIVE_SCOPE),
        urlencoding::encode(LOOPBACK_REDIRECT),
    );
    let parsed = url::Url::parse(&url).map_err(|e| e.to_string())?;

    if let Some(w) = app.get_webview_window("health-drive-auth") {
        let _: Result<(), tauri::Error> = w.close();
    }
    WebviewWindowBuilder::new(
        &app,
        "health-drive-auth",
        WebviewUrl::External(parsed),
    )
    .title("MINION — Connect Google Drive")
    .inner_size(500.0, 700.0)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    let code = tokio::time::timeout(Duration::from_secs(300), rx)
        .await
        .map_err(|_| {
            task.abort();
            "OAuth timed out".to_string()
        })?
        .map_err(|_| {
            task.abort();
            "OAuth was cancelled".to_string()
        })??;
    task.abort();

    let tokens = exchange_code_for_token(&client_id, client_secret.as_deref(), &code, LOOPBACK_REDIRECT)
        .await?;

    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO config (key, value) VALUES ('health_drive_access_token', ?1)",
        rusqlite::params![tokens.access_token],
    )
    .map_err(|e| e.to_string())?;
    if let Some(rt) = tokens.refresh_token {
        conn.execute(
            "INSERT OR REPLACE INTO config (key, value, encrypted) VALUES ('health_drive_refresh_token', ?1, 1)",
            rusqlite::params![rt],
        )
        .map_err(|e| e.to_string())?;
    }
    if let Some(exp) = tokens.expires_in {
        let expires_at = (Utc::now() + chrono::Duration::seconds(exp - 30)).to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES ('health_drive_token_expires_at', ?1)",
            rusqlite::params![expires_at],
        )
        .map_err(|e| e.to_string())?;
    }
    conn.execute(
        "INSERT OR REPLACE INTO drive_sync_state
         (id, enabled, account_id, error)
         VALUES (1, 1, NULL, NULL)",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn health_drive_disconnect(
    state: State<'_, AppStateHandle>,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    for k in [
        "health_drive_access_token",
        "health_drive_refresh_token",
        "health_drive_token_expires_at",
    ] {
        let _ = conn.execute("DELETE FROM config WHERE key = ?1", rusqlite::params![k]);
    }
    conn.execute(
        "UPDATE drive_sync_state SET enabled = 0 WHERE id = 1",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
// Drive REST helpers
// =====================================================================

async fn get_access_token(state: &AppStateHandle) -> Result<String, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT value FROM config WHERE key = 'health_drive_access_token'",
        [],
        |r| r.get::<_, String>(0),
    )
    .map_err(|_| "Drive not connected. Run health_drive_connect first.".to_string())
}

/// Look up an existing backup file in drive.appdata. Returns its file_id
/// when found.
async fn find_existing_backup(token: &str) -> Result<Option<String>, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://www.googleapis.com/drive/v3/files?spaces=appDataFolder&q=name%3D'{}'&\
         fields=files(id,name,modifiedTime,md5Checksum)",
        BACKUP_FILENAME
    );
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let s = resp.status();
        let b = resp.text().await.unwrap_or_default();
        return Err(format!("drive list failed ({}): {}", s, b));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let files = v.get("files").and_then(|f| f.as_array()).cloned().unwrap_or_default();
    if let Some(first) = files.first() {
        return Ok(first.get("id").and_then(|x| x.as_str()).map(|s| s.to_string()));
    }
    Ok(None)
}

async fn upload_backup(
    token: &str,
    file_id: Option<&str>,
    bytes: &[u8],
) -> Result<String, String> {
    let client = reqwest::Client::new();

    // Multipart upload (metadata + content).
    let metadata = serde_json::json!({
        "name": BACKUP_FILENAME,
        "parents": ["appDataFolder"],
    });
    let metadata_s = serde_json::to_string(&metadata).unwrap();

    let boundary = format!("minion-boundary-{}", uuid::Uuid::new_v4().simple());
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
    body.extend_from_slice(metadata_s.as_bytes());
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(bytes);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    let url = match file_id {
        Some(id) => format!(
            "https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=multipart",
            id
        ),
        None => "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart".to_string(),
    };
    let req = if file_id.is_some() {
        client.patch(&url)
    } else {
        client.post(&url)
    };
    let resp = req
        .bearer_auth(token)
        .header(
            "Content-Type",
            format!("multipart/related; boundary={}", boundary),
        )
        .body(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        let s = resp.status();
        let b = resp.text().await.unwrap_or_default();
        return Err(format!("drive upload failed ({}): {}", s, b));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let id = v
        .get("id")
        .and_then(|x| x.as_str())
        .ok_or("no id in upload response")?
        .to_string();
    Ok(id)
}

async fn download_backup(token: &str, file_id: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://www.googleapis.com/drive/v3/files/{}?alt=media",
        file_id
    );
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let s = resp.status();
        let b = resp.text().await.unwrap_or_default();
        return Err(format!("drive download failed ({}): {}", s, b));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    Ok(bytes.to_vec())
}

// =====================================================================
// Public Tauri commands: backup + restore
// =====================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupResult {
    pub bytes_uploaded: u64,
    pub remote_file_id: String,
    pub at: String,
}

#[tauri::command]
pub async fn health_drive_backup_now(
    state: State<'_, AppStateHandle>,
    passphrase: String,
) -> Result<BackupResult, String> {
    let token = get_access_token(&state).await?;
    let key = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        load_encryption_key(&conn, &passphrase)?
    };
    let payload = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        export_payload(&conn)?
    };
    let json = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    let encrypted = encrypt(&key, &json).map_err(|e| e.to_string())?;
    let existing = find_existing_backup(&token).await?;
    let file_id = upload_backup(&token, existing.as_deref(), &encrypted).await?;

    let now = Utc::now().to_rfc3339();
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO drive_sync_state
         (id, enabled, file_id_remote, last_synced_at, error)
         VALUES (1, 1, ?1, ?2, NULL)",
        rusqlite::params![file_id, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(BackupResult {
        bytes_uploaded: encrypted.len() as u64,
        remote_file_id: file_id,
        at: now,
    })
}

#[tauri::command]
pub async fn health_drive_restore_now(
    state: State<'_, AppStateHandle>,
    passphrase: String,
) -> Result<RestoreSummary, String> {
    let token = get_access_token(&state).await?;
    let key = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        load_encryption_key(&conn, &passphrase)?
    };
    let file_id = find_existing_backup(&token)
        .await?
        .ok_or_else(|| "no backup file in Drive appdata folder".to_string())?;
    let bytes = download_backup(&token, &file_id).await?;
    let decrypted = decrypt(&key, &bytes).map_err(|e| {
        format!("decryption failed — wrong passphrase? ({})", e)
    })?;
    let payload: BackupPayload =
        serde_json::from_slice(&decrypted).map_err(|e| format!("backup parse failed: {e}"))?;

    let summary = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        restore_payload(&conn, &payload)?
    };
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO drive_sync_state
         (id, enabled, file_id_remote, last_synced_at, error)
         VALUES (1, 1, ?1, ?2, NULL)",
        rusqlite::params![file_id, Utc::now().to_rfc3339()],
    )
    .map_err(|e| e.to_string())?;
    Ok(summary)
}

/// Local-only backup to a file on disk. Useful for offline handoff or
/// when the user doesn't want a Google account in the loop.
#[tauri::command]
pub async fn health_drive_export_local(
    state: State<'_, AppStateHandle>,
    passphrase: String,
    output_path: String,
) -> Result<u64, String> {
    let key = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        load_encryption_key(&conn, &passphrase)?
    };
    let payload = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        export_payload(&conn)?
    };
    let json = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    let encrypted = encrypt(&key, &json).map_err(|e| e.to_string())?;
    std::fs::write(&output_path, &encrypted).map_err(|e| e.to_string())?;
    Ok(encrypted.len() as u64)
}

#[tauri::command]
pub async fn health_drive_import_local(
    state: State<'_, AppStateHandle>,
    passphrase: String,
    input_path: String,
) -> Result<RestoreSummary, String> {
    let key = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        load_encryption_key(&conn, &passphrase)?
    };
    let bytes = std::fs::read(&input_path).map_err(|e| e.to_string())?;
    let decrypted = decrypt(&key, &bytes).map_err(|e| {
        format!("decryption failed — wrong passphrase? ({})", e)
    })?;
    let payload: BackupPayload =
        serde_json::from_slice(&decrypted).map_err(|e| format!("backup parse failed: {e}"))?;
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    restore_payload(&conn, &payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use minion_db::Database;
    use tempfile::tempdir;

    fn make_db() -> (tempfile::TempDir, Database) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("t.db");
        let db = Database::new(&path, 2).unwrap();
        db.migrate().unwrap();
        (dir, db)
    }

    #[test]
    fn export_then_restore_roundtrip() {
        let (_dir, db) = make_db();
        let conn = db.get().unwrap();
        // Insert a patient + lab.
        conn.execute(
            "INSERT INTO patients (id, phone_number, full_name, relationship, is_primary)
             VALUES ('p1', '+1', 'Test', 'self', 1)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO lab_tests (id, patient_id, test_name, value, collected_at)
             VALUES ('l1', 'p1', 'HbA1c', 6.5, '2025-01-01')",
            [],
        ).unwrap();

        let payload = export_payload(&conn).unwrap();
        assert_eq!(payload.patients.len(), 1);
        assert_eq!(payload.lab_tests.len(), 1);

        // Wipe and restore.
        conn.execute("DELETE FROM lab_tests", []).unwrap();
        conn.execute("DELETE FROM patients", []).unwrap();
        let summary = restore_payload(&conn, &payload).unwrap();
        assert_eq!(summary.patients, 1);
        assert_eq!(summary.labs, 1);
    }

    #[test]
    fn parse_oauth_callback_extracts_code() {
        let req = b"GET /?code=abc123&state=x HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        assert_eq!(parse_oauth_callback(req).unwrap(), "abc123");
    }

    #[test]
    fn parse_oauth_callback_surfaces_error() {
        let req = b"GET /?error=access_denied HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        assert!(parse_oauth_callback(req).is_err());
    }
}
