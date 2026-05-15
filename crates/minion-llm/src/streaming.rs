//! SSE (Server-Sent Events) stream parsing for OpenAI-compatible streaming responses.

use futures::StreamExt;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Token(String),
    Done,
    Error(String),
}

#[derive(Deserialize)]
struct SseDelta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct SseChoice {
    delta: SseDelta,
}

#[derive(Deserialize)]
struct SseChunk {
    choices: Vec<SseChoice>,
}

/// Parse a single SSE line into a `StreamEvent`.
/// Returns `None` for keep-alive comments and empty lines.
pub fn parse_sse_line(line: &str) -> Option<StreamEvent> {
    let line = line.trim();

    if line.is_empty() || line.starts_with(':') {
        return None;
    }

    let data = line.strip_prefix("data: ")?;

    if data == "[DONE]" {
        return Some(StreamEvent::Done);
    }

    match serde_json::from_str::<SseChunk>(data) {
        Ok(chunk) => {
            let token = chunk
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.delta.content)
                .unwrap_or_default();
            if token.is_empty() {
                None
            } else {
                Some(StreamEvent::Token(token))
            }
        }
        Err(e) => Some(StreamEvent::Error(format!("SSE parse error: {e}"))),
    }
}

/// Collect a streaming OpenAI-compatible response into a full string.
/// Calls `on_token` for each incremental token as bytes arrive — true streaming,
/// not buffered. Uses `bytes_stream()` so callers see tokens immediately.
pub async fn collect_stream<F>(
    response: reqwest::Response,
    mut on_token: F,
) -> Result<String, String>
where
    F: FnMut(&str),
{
    let mut byte_stream = response.bytes_stream();
    let mut line_buf = String::new();
    let mut full_text = String::new();

    while let Some(chunk) = byte_stream.next().await {
        let bytes = chunk.map_err(|e| e.to_string())?;
        let text = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
        line_buf.push_str(text);

        // Process every complete newline-terminated SSE line
        while let Some(pos) = line_buf.find('\n') {
            let line = line_buf[..pos].to_string();
            line_buf.drain(..=pos);

            if let Some(event) = parse_sse_line(&line) {
                match event {
                    StreamEvent::Token(t) => {
                        on_token(&t);
                        full_text.push_str(&t);
                    }
                    StreamEvent::Done => return Ok(full_text),
                    StreamEvent::Error(e) => return Err(e),
                }
            }
        }
    }
    Ok(full_text)
}
