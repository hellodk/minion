//! Multi-account Google Calendar and Microsoft Outlook (Graph) sync.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<crate::state::AppState>>;

/// Redirect URI registered in Google Cloud Console (Desktop OAuth client).
pub const GOOGLE_CAL_LOOPBACK: &str = "http://127.0.0.1:8747/";
/// Redirect URI registered in Azure app registration (Mobile/desktop).
pub const OUTLOOK_LOOPBACK: &str = "http://127.0.0.1:8748/";

const GOOGLE_CAL_SCOPE: &str = concat!(
    "https://www.googleapis.com/auth/calendar.readonly ",
    "https://www.googleapis.com/auth/userinfo.email",
);

#[derive(Debug, Serialize)]
pub struct CalendarAccountInfo {
    pub id: String,
    pub provider: String,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenJson {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

fn parse_oauth_code(buf: &[u8]) -> Result<String, String> {
    let s = std::str::from_utf8(buf).map_err(|_| "Invalid HTTP request".to_string())?;
    let first = s
        .lines()
        .next()
        .ok_or_else(|| "Empty request".to_string())?;
    let path = first
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "Bad request line".to_string())?;
    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
    let query = query.split_whitespace().next().unwrap_or(query);
    for pair in query.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        if k == "error" {
            return Err(format!(
                "OAuth error: {}",
                urlencoding::decode(v).unwrap_or_else(|_| v.into())
            ));
        }
        if k == "code" {
            return urlencoding::decode(v)
                .map(|c| c.into_owned())
                .map_err(|e| e.to_string());
        }
    }
    Err("No authorization code in OAuth callback".to_string())
}

fn pkce_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let digest = hasher.finalize();
    URL_SAFE_NO_PAD.encode(digest)
}

async fn google_exchange_code(
    client_id: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<OAuthTokenJson, String> {
    let client = reqwest::Client::new();
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
    ];
    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token request failed: {}", e))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "Google token endpoint returned {}: {}",
            status, body
        ));
    }
    serde_json::from_str::<OAuthTokenJson>(&body).map_err(|e| format!("Invalid token JSON: {}", e))
}

async fn google_refresh_token(
    client_id: &str,
    refresh_token: &str,
) -> Result<OAuthTokenJson, String> {
    let client = reqwest::Client::new();
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
    ];
    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token refresh failed: {}", e))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "Google token refresh returned {}: {}",
            status, body
        ));
    }
    serde_json::from_str::<OAuthTokenJson>(&body)
        .map_err(|e| format!("Invalid refresh JSON: {}", e))
}

async fn fetch_google_email(access_token: &str) -> Option<String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(access_token)
        .send()
        .await
        .ok()?;
    let v: Value = resp.json().await.ok()?;
    v.get("email")
        .and_then(|e| e.as_str())
        .map(|s| s.to_string())
}

fn close_cal_window(app: &tauri::AppHandle, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.close();
    }
}

fn expires_iso_from_secs(secs: Option<u64>) -> Option<String> {
    let secs = secs?;
    let dt = chrono::Utc::now() + chrono::Duration::seconds(secs as i64);
    Some(dt.to_rfc3339())
}

pub async fn list_accounts(state: &AppStateHandle) -> Result<Vec<CalendarAccountInfo>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, provider, email FROM calendar_accounts ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(CalendarAccountInfo {
                id: row.get(0)?,
                provider: row.get(1)?,
                email: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn read_google_client_id(conn: &rusqlite::Connection) -> Result<String, String> {
    conn.query_row(
        "SELECT value FROM config WHERE key = 'gfit_client_id'",
        [],
        |row| row.get(0),
    )
    .map_err(|_| {
        "Google OAuth Client ID not configured. Add your Desktop OAuth client ID under \
         Health & Fitness (Google Fit) or Settings, enable Google Calendar API, and add redirect \
         URI http://127.0.0.1:8747/"
            .to_string()
    })
}

pub async fn google_open_auth(app: tauri::AppHandle, state: AppStateHandle) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::time::Duration;

    let client_id = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        read_google_client_id(&conn)?
    };

    let redirect_uri = GOOGLE_CAL_LOOPBACK;
    let listener = TcpListener::bind("127.0.0.1:8747").await.map_err(|e| {
        format!(
            "Could not listen on 127.0.0.1:8747 ({}). Close anything using that port.",
            e
        )
    })?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Result<String, String>>();
    let callback_task = tokio::spawn(async move {
        let result = tokio::time::timeout(Duration::from_secs(300), listener.accept()).await;
        match result {
            Ok(Ok((mut stream, _))) => {
                let mut buf = vec![0u8; 16_384];
                let n = match stream.read(&mut buf).await {
                    Ok(n) => n,
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string()));
                        return;
                    }
                };
                buf.truncate(n);
                let parse_result = parse_oauth_code(&buf);
                let ok = parse_result.is_ok();
                let body = if ok {
                    "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Connected</title></head>\
                     <body style=\"font-family:system-ui;padding:2rem\">\
                     <p>Google Calendar account added. You can close this window.</p></body></html>"
                } else {
                    "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Authorization</title></head>\
                     <body style=\"font-family:system-ui;padding:2rem\">\
                     <p>Authorization did not complete. You can close this window.</p></body></html>"
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
                let _ = tx.send(parse_result);
            }
            Ok(Err(e)) => {
                let _ = tx.send(Err(format!("Accept failed: {}", e)));
            }
            Err(_) => {
                let _ = tx.send(Err(
                    "OAuth login timed out waiting for browser (5 minutes).".to_string(),
                ));
            }
        }
    });

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&scope={}&response_type=code&\
         access_type=offline&prompt=consent&redirect_uri={}",
        urlencoding::encode(&client_id),
        urlencoding::encode(GOOGLE_CAL_SCOPE),
        urlencoding::encode(redirect_uri),
    );

    close_cal_window(&app, "google-cal-auth");
    WebviewWindowBuilder::new(
        &app,
        "google-cal-auth",
        WebviewUrl::External(
            auth_url
                .parse()
                .map_err(|e: url::ParseError| e.to_string())?,
        ),
    )
    .title("MINION - Google Calendar")
    .inner_size(500.0, 700.0)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    let code_result = tokio::time::timeout(Duration::from_secs(300), rx)
        .await
        .map_err(|_| {
            callback_task.abort();
            "OAuth login timed out.".to_string()
        })?
        .map_err(|_| {
            callback_task.abort();
            "OAuth was cancelled.".to_string()
        })?;

    callback_task.abort();
    let code = code_result?;
    let tokens = google_exchange_code(&client_id, &code, redirect_uri).await?;

    let email = fetch_google_email(&tokens.access_token).await;
    let expires_at = expires_iso_from_secs(tokens.expires_in);
    let refresh = tokens.refresh_token.ok_or(
        "Google did not return a refresh token. Remove the app from your Google account \
                security settings and try again (use prompt=consent).",
    )?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO calendar_accounts (id, provider, email, access_token, refresh_token, expires_at, created_at) \
         VALUES (?1, 'google', ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, email, tokens.access_token, refresh, expires_at, now],
    )
    .map_err(|e| e.to_string())?;
    drop(st);

    close_cal_window(&app, "google-cal-auth");
    Ok(())
}

fn read_outlook_client_id(conn: &rusqlite::Connection) -> Result<String, String> {
    conn.query_row(
        "SELECT value FROM config WHERE key = 'outlook_client_id'",
        [],
        |row| row.get(0),
    )
    .map_err(|_| {
        "Outlook Application (client) ID not configured. Save it under Settings > Calendar, \
         and add redirect URI http://127.0.0.1:8748/ in Azure."
            .to_string()
    })
}

pub async fn outlook_open_auth(app: tauri::AppHandle, state: AppStateHandle) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::time::Duration;

    let client_id = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        read_outlook_client_id(&conn)?
    };

    let verifier = format!("{}{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let challenge = pkce_challenge(&verifier);
    let redirect_uri = OUTLOOK_LOOPBACK;

    let listener = TcpListener::bind("127.0.0.1:8748").await.map_err(|e| {
        format!(
            "Could not listen on 127.0.0.1:8748 ({}). Close anything using that port.",
            e
        )
    })?;

    let (tx, rx) = tokio::sync::oneshot::channel::<Result<String, String>>();
    let callback_task = tokio::spawn(async move {
        let result = tokio::time::timeout(Duration::from_secs(300), listener.accept()).await;
        match result {
            Ok(Ok((mut stream, _))) => {
                let mut buf = vec![0u8; 16_384];
                let n = match stream.read(&mut buf).await {
                    Ok(n) => n,
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string()));
                        return;
                    }
                };
                buf.truncate(n);
                let parse_result = parse_oauth_code(&buf);
                let ok = parse_result.is_ok();
                let body = if ok {
                    "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Connected</title></head>\
                     <body style=\"font-family:system-ui;padding:2rem\">\
                     <p>Outlook Calendar account added. You can close this window.</p></body></html>"
                } else {
                    "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Authorization</title></head>\
                     <body style=\"font-family:system-ui;padding:2rem\">\
                     <p>Authorization did not complete. You can close this window.</p></body></html>"
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
                let _ = tx.send(parse_result);
            }
            Ok(Err(e)) => {
                let _ = tx.send(Err(format!("Accept failed: {}", e)));
            }
            Err(_) => {
                let _ = tx.send(Err(
                    "OAuth login timed out waiting for browser (5 minutes).".to_string(),
                ));
            }
        }
    });

    let scope = "offline_access Calendars.Read User.Read";
    let auth_url = format!(
        "https://login.microsoftonline.com/common/oauth2/v2.0/authorize?client_id={}&\
         response_type=code&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256",
        urlencoding::encode(&client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(scope),
        urlencoding::encode(&challenge),
    );

    close_cal_window(&app, "outlook-cal-auth");
    WebviewWindowBuilder::new(
        &app,
        "outlook-cal-auth",
        WebviewUrl::External(
            auth_url
                .parse()
                .map_err(|e: url::ParseError| e.to_string())?,
        ),
    )
    .title("MINION - Outlook Calendar")
    .inner_size(500.0, 700.0)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    let code_result = tokio::time::timeout(Duration::from_secs(300), rx)
        .await
        .map_err(|_| {
            callback_task.abort();
            "OAuth login timed out.".to_string()
        })?
        .map_err(|_| {
            callback_task.abort();
            "OAuth was cancelled.".to_string()
        })?;

    callback_task.abort();
    let code = code_result?;

    let client = reqwest::Client::new();
    let params = [
        ("client_id", client_id.as_str()),
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", redirect_uri),
        ("code_verifier", verifier.as_str()),
    ];
    let resp = client
        .post("https://login.microsoftonline.com/common/oauth2/v2.0/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token request failed: {}", e))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "Microsoft token endpoint returned {}: {}",
            status, body
        ));
    }
    let tokens: OAuthTokenJson =
        serde_json::from_str(&body).map_err(|e| format!("Invalid token JSON: {}", e))?;

    let email = {
        let client = reqwest::Client::new();
        let r = client
            .get("https://graph.microsoft.com/v1.0/me?$select=mail,userPrincipalName")
            .bearer_auth(&tokens.access_token)
            .send()
            .await
            .ok();
        if let Some(r) = r {
            if let Ok(v) = r.json::<Value>().await {
                v.get("mail")
                    .or_else(|| v.get("userPrincipalName"))
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        } else {
            None
        }
    };

    let expires_at = expires_iso_from_secs(tokens.expires_in);
    let refresh = tokens
        .refresh_token
        .ok_or("Microsoft did not return a refresh token. Try signing in again.")?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO calendar_accounts (id, provider, email, access_token, refresh_token, expires_at, created_at) \
         VALUES (?1, 'outlook', ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, email, tokens.access_token, refresh, expires_at, now],
    )
    .map_err(|e| e.to_string())?;
    drop(st);

    close_cal_window(&app, "outlook-cal-auth");
    Ok(())
}

async fn ensure_google_access(
    state: &AppStateHandle,
    account_id: &str,
    client_id: &str,
) -> Result<String, String> {
    let (access, refresh, expires_at): (String, Option<String>, Option<String>) = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn
            .query_row(
                "SELECT access_token, refresh_token, expires_at FROM calendar_accounts WHERE id = ?1 AND provider = 'google'",
                [account_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|_| "Google calendar account not found".to_string())?
    };

    let need_refresh = if let Some(exp) = expires_at {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&exp) {
            dt.with_timezone(&chrono::Utc) < chrono::Utc::now() + chrono::Duration::minutes(2)
        } else {
            true
        }
    } else {
        false
    };

    if !need_refresh {
        return Ok(access);
    }

    let rt = refresh.ok_or("No refresh token stored for this Google account")?;
    let new_t = google_refresh_token(client_id, &rt).await?;
    let new_exp = expires_iso_from_secs(new_t.expires_in);
    {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE calendar_accounts SET access_token = ?1, expires_at = ?2 WHERE id = ?3",
            rusqlite::params![new_t.access_token, new_exp, account_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(new_t.access_token)
}

async fn outlook_refresh_token(
    client_id: &str,
    refresh_token: &str,
) -> Result<OAuthTokenJson, String> {
    let client = reqwest::Client::new();
    let scope = "offline_access Calendars.Read User.Read";
    let params = [
        ("client_id", client_id),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("scope", scope),
    ];
    let resp = client
        .post("https://login.microsoftonline.com/common/oauth2/v2.0/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Outlook token refresh failed: {}", e))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Outlook refresh returned {}: {}", status, body));
    }
    serde_json::from_str::<OAuthTokenJson>(&body)
        .map_err(|e| format!("Invalid refresh JSON: {}", e))
}

async fn ensure_outlook_access(
    state: &AppStateHandle,
    account_id: &str,
    client_id: &str,
) -> Result<String, String> {
    let (access, refresh, expires_at): (String, Option<String>, Option<String>) = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn
            .query_row(
                "SELECT access_token, refresh_token, expires_at FROM calendar_accounts WHERE id = ?1 AND provider = 'outlook'",
                [account_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|_| "Outlook calendar account not found".to_string())?
    };

    let need_refresh = if let Some(exp) = expires_at {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&exp) {
            dt.with_timezone(&chrono::Utc) < chrono::Utc::now() + chrono::Duration::minutes(2)
        } else {
            true
        }
    } else {
        false
    };

    if !need_refresh {
        return Ok(access);
    }

    let rt = refresh.ok_or("No refresh token stored for this Outlook account")?;
    let new_t = outlook_refresh_token(client_id, &rt).await?;
    let new_exp = expires_iso_from_secs(new_t.expires_in);
    {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE calendar_accounts SET access_token = ?1, expires_at = ?2, refresh_token = COALESCE(?3, refresh_token) WHERE id = ?4",
            rusqlite::params![
                new_t.access_token,
                new_exp,
                new_t.refresh_token.as_deref(),
                account_id
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(new_t.access_token)
}

struct GcalInsert {
    id: String,
    title: String,
    description: Option<String>,
    start_time: String,
    end_time: Option<String>,
    all_day: bool,
    location: Option<String>,
    remote_id: String,
    cal_name: String,
}

/// Sync one Google account: all selected calendars in range.
pub async fn sync_google_account(
    state: &AppStateHandle,
    account_id: &str,
    client_id: &str,
) -> Result<usize, String> {
    let token = ensure_google_access(state, account_id, client_id).await?;
    {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM calendar_events WHERE account_id = ?1 AND source = 'google'",
            [account_id],
        )
        .map_err(|e| e.to_string())?;
    }

    let client = reqwest::Client::new();
    let list_url = "https://www.googleapis.com/calendar/v3/users/me/calendarList";
    let list_resp = client
        .get(list_url)
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| format!("Google calendar list: {}", e))?;
    if !list_resp.status().is_success() {
        let b = list_resp.text().await.unwrap_or_default();
        return Err(format!("Google calendar list failed: {}", b));
    }
    let list_body: Value = list_resp.json().await.map_err(|e| e.to_string())?;
    let items = list_body
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let now = chrono::Utc::now();
    let time_min = (now - chrono::Duration::days(30)).to_rfc3339();
    let time_max = (now + chrono::Duration::days(60)).to_rfc3339();
    let now_str = chrono::Utc::now().to_rfc3339();

    let mut rows: Vec<GcalInsert> = Vec::new();

    for cal in &items {
        let selected = cal
            .get("selected")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        if !selected {
            continue;
        }
        let cal_id = cal
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if cal_id.is_empty() {
            continue;
        }
        let cal_name = cal
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("Calendar")
            .to_string();

        let enc = urlencoding::encode(&cal_id);
        let url = format!(
            "https://www.googleapis.com/calendar/v3/calendars/{}/events?timeMin={}&timeMax={}&singleEvents=true&orderBy=startTime&maxResults=250",
            enc, time_min, time_max
        );

        let resp = client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| format!("Google events: {}", e))?;
        if !resp.status().is_success() {
            let b = resp.text().await.unwrap_or_default();
            return Err(format!("Google events failed for {}: {}", cal_id, b));
        }
        let body: Value = resp.json().await.map_err(|e| e.to_string())?;
        let evs = body
            .get("items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        for item in &evs {
            let remote_id = item
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if remote_id.is_empty() {
                continue;
            }
            let title = item
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("(No title)")
                .to_string();
            let description = item
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let location = item
                .get("location")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let start_obj = item.get("start");
            let end_obj = item.get("end");
            let all_day = start_obj.and_then(|s| s.get("date")).is_some();
            let start_time = start_obj
                .and_then(|s| s.get("dateTime").or(s.get("date")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let end_time = end_obj
                .and_then(|s| s.get("dateTime").or(s.get("date")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if start_time.is_empty() {
                continue;
            }

            let id = format!("g_{}_{}", account_id, remote_id.replace('/', "_"));
            rows.push(GcalInsert {
                id,
                title,
                description,
                start_time,
                end_time,
                all_day,
                location,
                remote_id,
                cal_name: cal_name.clone(),
            });
        }
    }

    let mut upserted = 0;
    {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        for row in rows {
            conn.execute(
                "INSERT OR REPLACE INTO calendar_events \
                 (id, title, description, start_time, end_time, all_day, location, color, source, remote_id, calendar_name, account_id, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '#4285f4', 'google', ?8, ?9, ?10, ?11, ?11)",
                rusqlite::params![
                    row.id,
                    row.title,
                    row.description,
                    row.start_time,
                    row.end_time,
                    row.all_day as i32,
                    row.location,
                    row.remote_id,
                    row.cal_name,
                    account_id,
                    now_str
                ],
            )
            .map_err(|e| e.to_string())?;
            upserted += 1;
        }
    }

    Ok(upserted)
}

struct OcalInsert {
    id: String,
    title: String,
    body_preview: Option<String>,
    start_time: String,
    end_time: Option<String>,
    all_day: bool,
    location: Option<String>,
    remote_id: String,
}

/// Sync one Outlook account via Microsoft Graph calendarView.
pub async fn sync_outlook_account(
    state: &AppStateHandle,
    account_id: &str,
    client_id: &str,
) -> Result<usize, String> {
    let token = ensure_outlook_access(state, account_id, client_id).await?;
    {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM calendar_events WHERE account_id = ?1 AND source = 'outlook'",
            [account_id],
        )
        .map_err(|e| e.to_string())?;
    }

    let client = reqwest::Client::new();
    let now = chrono::Utc::now();
    let start = (now - chrono::Duration::days(30)).to_rfc3339();
    let end = (now + chrono::Duration::days(60)).to_rfc3339();
    let now_str = chrono::Utc::now().to_rfc3339();

    let mut url = format!(
        "https://graph.microsoft.com/v1.0/me/calendar/calendarView?\
         startDateTime={}&endDateTime={}&$top=100&$orderby=start/dateTime",
        urlencoding::encode(&start),
        urlencoding::encode(&end)
    );

    let mut rows: Vec<OcalInsert> = Vec::new();

    loop {
        let resp = client
            .get(&url)
            .bearer_auth(&token)
            .header("Prefer", "outlook.timezone=\"UTC\"")
            .send()
            .await
            .map_err(|e| format!("Graph calendarView: {}", e))?;
        if !resp.status().is_success() {
            let b = resp.text().await.unwrap_or_default();
            return Err(format!("Graph returned: {}", b));
        }
        let body: Value = resp.json().await.map_err(|e| e.to_string())?;
        let evs = body
            .get("value")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        for item in &evs {
            let remote_id = item
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if remote_id.is_empty() {
                continue;
            }
            let title = item
                .get("subject")
                .and_then(|v| v.as_str())
                .unwrap_or("(No title)")
                .to_string();
            let body_preview = item
                .get("bodyPreview")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let location = item
                .get("location")
                .and_then(|l| l.get("displayName"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let start_o = item.get("start");
            let end_o = item.get("end");
            let all_day = start_o.map(|s| s.get("dateTime").is_none()).unwrap_or(true);
            let start_time = start_o
                .and_then(|s| s.get("dateTime").or(s.get("date")).and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string();
            let end_time = end_o
                .and_then(|s| s.get("dateTime").or(s.get("date")).and_then(|v| v.as_str()))
                .map(|s| s.to_string());

            if start_time.is_empty() {
                continue;
            }

            let id = format!("o_{}_{}", account_id, remote_id.replace('/', "_"));
            rows.push(OcalInsert {
                id,
                title,
                body_preview,
                start_time,
                end_time,
                all_day,
                location,
                remote_id,
            });
        }

        if let Some(next) = body.get("@odata.nextLink").and_then(|v| v.as_str()) {
            url = next.to_string();
        } else {
            break;
        }
    }

    let mut upserted = 0;
    {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        for row in rows {
            conn.execute(
                "INSERT OR REPLACE INTO calendar_events \
                 (id, title, description, start_time, end_time, all_day, location, color, source, remote_id, calendar_name, account_id, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '#0078d4', 'outlook', ?8, 'Outlook', ?9, ?10, ?10)",
                rusqlite::params![
                    row.id,
                    row.title,
                    row.body_preview,
                    row.start_time,
                    row.end_time,
                    row.all_day as i32,
                    row.location,
                    row.remote_id,
                    account_id,
                    now_str
                ],
            )
            .map_err(|e| e.to_string())?;
            upserted += 1;
        }
    }

    Ok(upserted)
}

pub async fn remove_account(state: &AppStateHandle, account_id: String) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM calendar_events WHERE account_id = ?1",
        [&account_id],
    )
    .map_err(|e| e.to_string())?;
    let n = conn
        .execute("DELETE FROM calendar_accounts WHERE id = ?1", [&account_id])
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("Account not found".to_string());
    }
    Ok(())
}

pub async fn sync_all_google(state: &AppStateHandle) -> Result<String, String> {
    let (client_id, ids): (String, Vec<String>) = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        let client_id = read_google_client_id(&conn)?;
        let mut stmt = conn
            .prepare("SELECT id FROM calendar_accounts WHERE provider = 'google'")
            .map_err(|e| e.to_string())?;
        let ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        (client_id, ids)
    };

    if ids.is_empty() {
        return Err(
            "No Google Calendar accounts connected. Use Add Google account in Settings."
                .to_string(),
        );
    }

    let mut total = 0usize;
    for id in ids {
        let n = sync_google_account(state, &id, &client_id).await?;
        total += n;
    }
    Ok(format!(
        "Synced {} Google Calendar events across connected accounts.",
        total
    ))
}

pub async fn sync_one_google(state: &AppStateHandle, account_id: String) -> Result<String, String> {
    let client_id = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        read_google_client_id(&conn)?
    };
    let n = sync_google_account(state, &account_id, &client_id).await?;
    Ok(format!("Synced {} events from this Google account.", n))
}

pub async fn sync_all_outlook(state: &AppStateHandle) -> Result<String, String> {
    let (client_id, ids): (String, Vec<String>) = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        let client_id = read_outlook_client_id(&conn)?;
        let mut stmt = conn
            .prepare("SELECT id FROM calendar_accounts WHERE provider = 'outlook'")
            .map_err(|e| e.to_string())?;
        let ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        (client_id, ids)
    };

    if ids.is_empty() {
        return Err("No Outlook accounts connected. Add an account in Settings.".to_string());
    }

    let mut total = 0usize;
    for id in ids {
        let n = sync_outlook_account(state, &id, &client_id).await?;
        total += n;
    }
    Ok(format!(
        "Synced {} Outlook events across connected accounts.",
        total
    ))
}

pub async fn sync_one_outlook(
    state: &AppStateHandle,
    account_id: String,
) -> Result<String, String> {
    let client_id = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        read_outlook_client_id(&conn)?
    };
    let n = sync_outlook_account(state, &account_id, &client_id).await?;
    Ok(format!("Synced {} events from this Outlook account.", n))
}
