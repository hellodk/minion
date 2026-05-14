use anyhow::{bail, Context};

const MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;

pub async fn process_url(raw_url: &str) -> anyhow::Result<String> {
    let raw_url = raw_url.to_owned();
    let validated = tokio::task::spawn_blocking({
        let u = raw_url.clone();
        move || {
            crate::security::ssrf_guard::validate_url(&u)
                .map_err(|e| anyhow::anyhow!("SSRF guard: {e}"))
        }
    })
    .await
    .context("spawn_blocking panicked")??;
    let url_str = validated.to_string();

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(
            crate::security::ssrf_guard::MAX_REDIRECTS,
        ))
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("minion-presentation/1.0 (content ingestion)")
        .build()
        .context("failed to build HTTP client")?;

    let response = client
        .get(&url_str)
        .send()
        .await
        .with_context(|| format!("HTTP GET failed: {url_str}"))?;
    let status = response.status();
    if !status.is_success() {
        bail!("HTTP {status} fetching '{url_str}'");
    }

    let bytes = read_limited_body(response, MAX_RESPONSE_BYTES).await?;
    let raw_text = String::from_utf8_lossy(&bytes).into_owned();
    let text = extract_readable_text(&raw_text, &url_str);
    if text.trim().is_empty() {
        bail!("URL '{}' returned no extractable text", url_str);
    }
    Ok(format!("[Source: {url_str}]\n\n{text}"))
}

async fn read_limited_body(response: reqwest::Response, limit: usize) -> anyhow::Result<Vec<u8>> {
    // Read up to `limit` bytes; reqwest 0.11 without the `stream` feature uses .bytes()
    let content_length = response.content_length().unwrap_or(0);
    if content_length > limit as u64 {
        bail!(
            "response Content-Length ({content_length}) exceeds {} MB limit",
            limit / (1024 * 1024)
        );
    }
    let bytes = response
        .bytes()
        .await
        .context("error reading response body")?;
    if bytes.len() > limit {
        bail!("response body exceeds {} MB limit", limit / (1024 * 1024));
    }
    Ok(bytes.to_vec())
}

fn extract_readable_text(raw: &str, url: &str) -> String {
    let lower_url = url.to_lowercase();
    let is_html = raw.trim_start().starts_with("<!") || raw.contains("<html") || raw.contains("<body");
    if is_html || lower_url.ends_with(".html") || lower_url.ends_with(".htm") {
        strip_html_to_text(raw)
    } else {
        raw.chars().take(50_000).collect()
    }
}

fn strip_html_to_text(html: &str) -> String {
    let without_scripts =
        remove_block_tags(html, &["script", "style", "head", "nav", "footer"]);
    let mut result = String::with_capacity(without_scripts.len());
    let mut inside_tag = false;
    for ch in without_scripts.chars() {
        match ch {
            '<' => inside_tag = true,
            '>' => {
                inside_tag = false;
                result.push(' ');
            }
            _ if !inside_tag => result.push(ch),
            _ => {}
        }
    }
    let decoded = result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&#39;", "'");
    let collapsed: String = decoded.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(50_000).collect()
}

fn remove_block_tags(html: &str, tags: &[&str]) -> String {
    let mut result = html.to_string();
    for tag in tags {
        let open = format!("<{tag}");
        let close = format!("</{tag}>");
        loop {
            let start = result.to_lowercase().find(&open);
            let end = result.to_lowercase().find(&close);
            match (start, end) {
                (Some(s), Some(e)) if s < e => {
                    result.drain(s..(e + close.len()));
                }
                _ => break,
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strip_html_extracts_text() {
        let html = "<html><body><h1>Title</h1><p>Hello world</p></body></html>";
        let text = strip_html_to_text(html);
        assert!(text.contains("Title"));
        assert!(text.contains("Hello world"));
    }
    #[test]
    fn strip_html_removes_scripts() {
        let html =
            "<html><head><script>var x=1;</script></head><body><p>Content</p></body></html>";
        let text = strip_html_to_text(html);
        assert!(!text.contains("var x"));
        assert!(text.contains("Content"));
    }
    #[test]
    fn strip_html_decodes_entities() {
        let html = "<p>a &amp; b &lt;c&gt;</p>";
        let text = strip_html_to_text(html);
        assert!(text.contains("a & b"));
    }
}
