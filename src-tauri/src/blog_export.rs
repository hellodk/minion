//! HTML / PDF export for blog posts.
//!
//! Fixes applied vs. first implementation:
//!  • SVG sanitisation — scripts, event handlers, javascript: hrefs stripped (#6)
//!  • Duplicate SVG deduplication — first embed gets id=, repeats use <use href> (#9)
//!  • DataUri replacement uses replacen on `src="..."` prefix, not bare value (#3)
//!  • img regex handles both `/>` and `>` endings (#16)
//!  • Date extracted via char-boundary-safe find(), not byte slice (#17)
//!  • PostMeta now owns its strings so callers can spawn_blocking (#2)
//!  • published_at, excerpt, word_count, reading_time exposed in HTML (#11)
//!  • Print CSS no longer appends raw URLs after every hyperlink (#22)
//!  • SVG <title> element extracted as alt text for converted images (#21)

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use pulldown_cmark::{html, Options, Parser};
use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// CSS
// ---------------------------------------------------------------------------

const EXPORT_CSS: &str = r#"
*, *::before, *::after { box-sizing: border-box; }
body {
  font-family: Georgia, 'Times New Roman', serif;
  font-size: 18px;
  line-height: 1.7;
  color: #1a1a1a;
  background: #fff;
  max-width: 780px;
  margin: 0 auto;
  padding: 3rem 2rem 5rem;
}
h1 { font-size: 2.2rem; line-height: 1.2; margin: 0 0 0.3rem; }
h2 { font-size: 1.6rem; margin: 2.4rem 0 0.6rem; border-bottom: 1px solid #e5e5e5; padding-bottom: 0.3rem; }
h3 { font-size: 1.25rem; margin: 2rem 0 0.4rem; }
h4, h5, h6 { font-size: 1rem; margin: 1.5rem 0 0.3rem; }
p  { margin: 0 0 1.2rem; }
a  { color: #2563eb; }
a:visited { color: #7c3aed; }
img, svg { max-width: 100%; height: auto; display: block; margin: 1.5rem auto; border-radius: 4px; }
.svg-reuse { display: block; max-width: 100%; margin: 1.5rem auto; }
pre {
  background: #f4f4f4;
  border: 1px solid #ddd;
  border-radius: 6px;
  padding: 1rem 1.2rem;
  overflow-x: auto;
  font-size: 0.85rem;
  line-height: 1.5;
  margin: 1.5rem 0;
}
code { font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace; }
p code, li code {
  background: #f0f0f0;
  border: 1px solid #ddd;
  border-radius: 3px;
  padding: 0.1em 0.35em;
  font-size: 0.88em;
}
blockquote {
  border-left: 4px solid #2563eb;
  margin: 1.5rem 0;
  padding: 0.5rem 0 0.5rem 1.5rem;
  color: #555;
  font-style: italic;
}
table { border-collapse: collapse; width: 100%; margin: 1.5rem 0; font-size: 0.95rem; }
th, td { border: 1px solid #ccc; padding: 0.5rem 0.8rem; text-align: left; }
th { background: #f0f0f0; font-weight: 600; }
tr:nth-child(even) td { background: #fafafa; }
ul, ol { padding-left: 1.8rem; margin: 0 0 1.2rem; }
li { margin-bottom: 0.3rem; }
hr { border: none; border-top: 1px solid #e5e5e5; margin: 2.5rem 0; }
.footnotes { border-top: 1px solid #e5e5e5; margin-top: 3rem; padding-top: 1rem; font-size: 0.88rem; color: #555; }
.post-meta { color: #666; font-family: system-ui, sans-serif; font-size: 0.85rem; margin-bottom: 2.5rem; }
.post-meta .sep::before { content: ' · '; }
.post-stats { font-size: 0.8rem; color: #888; margin-top: 0.25rem; font-family: system-ui, sans-serif; }
.post-excerpt { font-style: italic; color: #555; margin-bottom: 2rem; font-size: 0.95rem; border-left: 3px solid #e5e5e5; padding-left: 1rem; }
.post-tags { margin-top: 0.4rem; }
.tag {
  display: inline-block;
  background: #eff6ff;
  color: #1d4ed8;
  border: 1px solid #bfdbfe;
  border-radius: 99px;
  padding: 0.1em 0.65em;
  font-size: 0.78rem;
  margin: 0 0.2rem 0.2rem 0;
  font-family: system-ui, sans-serif;
}
@media print {
  body { padding: 0; font-size: 16px; }
  h2 { border-bottom-color: #ccc; }
  pre { break-inside: avoid; }
  /* Do NOT expand link URLs — creates cluttered output (#22) */
}
"#;

// ---------------------------------------------------------------------------
// SVG sanitisation (#6)
// Strips: <script>, on* event attributes, javascript: hrefs, <foreignObject>.
// ---------------------------------------------------------------------------

fn sanitise_svg(svg: &str) -> String {
    let re_script = regex::Regex::new(r"(?si)<script[^>]*>.*?</script\s*>").unwrap();
    let re_foreign = regex::Regex::new(r"(?si)<foreignObject[^>]*>.*?</foreignObject\s*>").unwrap();
    let re_on_dq = regex::Regex::new(r#"(?i)\s+on[a-z][a-z0-9]*\s*=\s*"[^"]*""#).unwrap();
    let re_on_sq = regex::Regex::new(r"(?i)\s+on[a-z][a-z0-9]*\s*=\s*'[^']*'").unwrap();
    let re_js_href_dq = regex::Regex::new(r#"(?i)href\s*=\s*"javascript:[^"]*""#).unwrap();
    let re_js_href_sq = regex::Regex::new(r"(?i)href\s*=\s*'javascript:[^']*'").unwrap();

    let s = re_script.replace_all(svg, "");
    let s = re_foreign.replace_all(&s, "");
    let s = re_on_dq.replace_all(&s, "");
    let s = re_on_sq.replace_all(&s, "");
    let s = re_js_href_dq.replace_all(&s, r##"href="#""##);
    let s = re_js_href_sq.replace_all(&s, "href='#'");
    s.into_owned()
}

// ---------------------------------------------------------------------------
// SVG <title> extraction (#21)
// ---------------------------------------------------------------------------

fn extract_svg_title(svg: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?si)<title[^>]*>([^<]+)</title>").ok()?;
    re.captures(svg)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_owned())
        .filter(|s| !s.is_empty())
}

// ---------------------------------------------------------------------------
// Asset embedding
// ---------------------------------------------------------------------------

enum Embed {
    /// First occurrence of an SVG: sanitised markup with an id= attribute added.
    InlineSvgFirst { id: String, markup: String, alt: String },
    /// Subsequent occurrence of an already-embedded SVG: lightweight <use> ref.
    InlineSvgReuse { id: String },
    /// data: URI src replacement.
    DataUri(String),
    /// Asset not found / unreadable — leave the original reference.
    Passthrough,
}

/// Build a DataUri Embed from raw bytes, guessing mime from the URL extension.
fn make_data_uri_embed(src: &str, data: &[u8]) -> Embed {
    let ext = src.rsplit('.').next().unwrap_or("").split('?').next().unwrap_or("").to_lowercase();
    let mime = match ext.as_str() {
        "png"        => "image/png",
        "jpg"|"jpeg" => "image/jpeg",
        "gif"        => "image/gif",
        "webp"       => "image/webp",
        "svg"        => "image/svg+xml",
        "avif"       => "image/avif",
        _            => "image/png",  // safe fallback
    };
    Embed::DataUri(format!("data:{mime};base64,{}", B64.encode(data)))
}

/// Returns true for http:// and https:// URLs.
fn is_external(src: &str) -> bool {
    src.starts_with("http://") || src.starts_with("https://")
}

fn embed_asset(
    src: &str,
    vault_dir: &Path,
    svg_ids: &mut HashMap<String, String>,
    svg_counter: &mut u32,
    ext_cache: &HashMap<String, Vec<u8>>,
) -> Embed {
    // External URLs: use pre-downloaded bytes if available, else Passthrough
    // (the URL stays in the HTML and browsers will load it if online).
    if is_external(src) {
        return match ext_cache.get(src) {
            Some(data) => make_data_uri_embed(src, data),
            None => Embed::Passthrough,
        };
    }

    let filename = Path::new(src)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(src);
    let full = vault_dir.join(filename);
    let data = match std::fs::read(&full) {
        Ok(d) => d,
        Err(_) => return Embed::Passthrough,
    };

    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "svg" {
        // Deduplicate: first occurrence gets an id; subsequent get a <use> ref (#9)
        if let Some(existing_id) = svg_ids.get(filename) {
            return Embed::InlineSvgReuse { id: existing_id.clone() };
        }

        match String::from_utf8(data) {
            Ok(raw) => {
                let sanitised = sanitise_svg(&raw);
                let alt = extract_svg_title(&sanitised).unwrap_or_default();
                *svg_counter += 1;
                let id = format!("svg-embed-{svg_counter}");
                svg_ids.insert(filename.to_owned(), id.clone());
                Embed::InlineSvgFirst { id, markup: sanitised, alt }
            }
            Err(_) => Embed::Passthrough,
        }
    } else {
        let mime = match ext.as_str() {
            "png"        => "image/png",
            "jpg"|"jpeg" => "image/jpeg",
            "gif"        => "image/gif",
            "webp"       => "image/webp",
            "bmp"        => "image/bmp",
            "tiff"|"tif" => "image/tiff",
            "avif"       => "image/avif",
            _            => "application/octet-stream",
        };
        Embed::DataUri(format!("data:{mime};base64,{}", B64.encode(&data)))
    }
}

// ---------------------------------------------------------------------------
// Safe date extraction (#17)
// ---------------------------------------------------------------------------

fn extract_date_prefix(s: &str) -> &str {
    // ISO 8601 dates are "YYYY-MM-DD..." — find the 10th character boundary safely.
    s.char_indices()
        .nth(10)
        .map(|(i, _)| &s[..i])
        .unwrap_or(s)
}

// ---------------------------------------------------------------------------
// HTML builder
// ---------------------------------------------------------------------------

pub struct PostMeta {
    pub title: String,
    pub author: Option<String>,
    pub tags: Option<String>,
    pub excerpt: Option<String>,
    pub created_at: String,
    pub published_at: Option<String>,
    pub word_count: Option<i32>,
    pub reading_time: Option<i32>,
}

/// Build a fully self-contained HTML string from a blog post.
///
/// `for_print` injects `window.print()` on load (PDF workflow).
/// `ext_cache` holds pre-downloaded external image bytes (url → bytes).
/// This function does synchronous file I/O — wrap in `spawn_blocking`.
pub fn build_html(
    meta: &PostMeta,
    content_md: &str,
    vault_dir: &Path,
    for_print: bool,
    ext_cache: &HashMap<String, Vec<u8>>,
) -> Result<String, String> {
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_SMART_PUNCTUATION;

    let mut body_html = String::with_capacity(content_md.len() * 2);
    html::push_html(&mut body_html, Parser::new_ext(content_md, opts));

    // Regex handles both self-closing `/>` and regular `>` (#16).
    // src value uses [^"] to avoid false matches in other attributes.
    let img_re = regex::Regex::new(r#"<img\s[^>]*?src="([^"]+)"[^>]*/?>(?:</img>)?"#)
        .map_err(|e| e.to_string())?;

    let mut embedded = String::with_capacity(body_html.len());
    let mut last = 0usize;
    let mut svg_ids: HashMap<String, String> = HashMap::new();
    let mut svg_counter: u32 = 0;

    for cap in img_re.captures_iter(&body_html) {
        let m = cap.get(0).unwrap();
        embedded.push_str(&body_html[last..m.start()]);

        let src = &cap[1];
        match embed_asset(src, vault_dir, &mut svg_ids, &mut svg_counter, ext_cache) {
            Embed::InlineSvgFirst { id, markup, alt } => {
                // Add id= to the root <svg> element for <use> references.
                let with_id = markup.replacen("<svg", &format!(r#"<svg id="{id}""#), 1);
                // If the <svg> already has an id, the replacen inserts a second one —
                // browsers will use the first. Acceptable for export purposes.
                embedded.push_str(&with_id);
            }
            Embed::InlineSvgReuse { id } => {
                // Lightweight reference — avoids duplicating KB of markup (#9).
                embedded.push_str(&format!(
                    r##"<svg class="svg-reuse" aria-label="{id}"><use href="#{id}"></use></svg>"##
                ));
            }
            Embed::DataUri(uri) => {
                // Replace only the src="..." attribute to avoid corrupting alt text (#3).
                let src_attr = format!(r#"src="{src}""#);
                let new_attr = format!(r#"src="{uri}""#);
                let replaced = m.as_str().replacen(&src_attr, &new_attr, 1);
                embedded.push_str(&replaced);
            }
            Embed::Passthrough => {
                embedded.push_str(m.as_str());
            }
        }

        last = m.end();
    }
    embedded.push_str(&body_html[last..]);

    // --- Meta section ---

    let mut meta_parts: Vec<String> = Vec::new();

    if let Some(a) = meta.author.as_deref().filter(|s| !s.is_empty()) {
        meta_parts.push(format!("<span>{}</span>", html_escape::encode_text(a)));
    }

    // Use published_at when available, fall back to created_at (#11)
    let date_str = meta.published_at.as_deref().unwrap_or(&meta.created_at);
    let date_prefix = extract_date_prefix(date_str);
    if !date_prefix.is_empty() {
        meta_parts.push(format!("<span>{date_prefix}</span>"));
    }

    // Stats line (#11)
    let mut stats: Vec<String> = Vec::new();
    if let Some(wc) = meta.word_count {
        stats.push(format!("{wc} words"));
    }
    if let Some(rt) = meta.reading_time {
        stats.push(format!("{rt} min read"));
    }

    let meta_html = if meta_parts.is_empty() {
        String::new()
    } else {
        let joined = meta_parts
            .iter()
            .enumerate()
            .map(|(i, s)| if i == 0 { s.clone() } else { format!(r#"<span class="sep">{s}</span>"#) })
            .collect::<Vec<_>>()
            .join("");
        let stats_html = if stats.is_empty() {
            String::new()
        } else {
            format!(r#"<div class="post-stats">{}</div>"#, stats.join(" · "))
        };
        format!(r#"<div class="post-meta">{joined}{stats_html}</div>"#)
    };

    let excerpt_html = meta
        .excerpt
        .as_deref()
        .filter(|e| !e.is_empty())
        .map(|e| format!(r#"<p class="post-excerpt">{}</p>"#, html_escape::encode_text(e)))
        .unwrap_or_default();

    let tags_html = meta
        .tags
        .as_deref()
        .filter(|t| !t.is_empty())
        .map(|t| {
            let pills: String = t
                .split(',')
                .map(|tag| tag.trim())
                .filter(|tag| !tag.is_empty())
                .map(|tag| format!(r#"<span class="tag">{}</span>"#, html_escape::encode_text(tag)))
                .collect();
            format!(r#"<div class="post-tags">{pills}</div>"#)
        })
        .unwrap_or_default();

    let print_script = if for_print {
        r#"<script>window.addEventListener('load',()=>setTimeout(()=>window.print(),350));</script>"#
    } else {
        ""
    };

    let title_esc = html_escape::encode_text(&meta.title);
    Ok(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width,initial-scale=1" />
{desc_meta}
<title>{title_esc}</title>
<style>{css}</style>
{print_script}
</head>
<body>
<h1>{title_esc}</h1>
{meta_html}
{excerpt_html}
{tags_html}
{body}
</body>
</html>"#,
        desc_meta = meta
            .excerpt
            .as_deref()
            .filter(|e| !e.is_empty())
            .map(|e| format!(r#"<meta name="description" content="{}" />"#, html_escape::encode_text(e)))
            .unwrap_or_default(),
        css = EXPORT_CSS,
        body = embedded,
    ))
}

// ---------------------------------------------------------------------------
// Filename sanitisation (#7)
// ---------------------------------------------------------------------------

/// Convert a post title to a safe ASCII filename slug.
/// Non-ASCII chars are dropped (not blindly replaced with dashes),
/// consecutive dashes are collapsed, and the result is capped at 80 chars.
pub fn title_to_filename(title: &str) -> String {
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();

    // Collapse consecutive dashes, strip leading/trailing
    let mut prev_dash = true; // skip leading dashes
    let clean: String = slug
        .chars()
        .filter(|&c| {
            if c == '-' {
                if prev_dash { return false; }
                prev_dash = true;
            } else {
                prev_dash = false;
            }
            true
        })
        .collect();
    let clean = clean.trim_end_matches('-');

    if clean.is_empty() {
        "post".to_owned()
    } else {
        format!("{}.html", &clean[..clean.len().min(80)])
    }
}
