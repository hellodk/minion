//! SVG → PNG rasterization for blog export.
//!
//! LinkedIn and Medium do not render SVG. This module converts:
//!   • `![alt](assets/xxx.svg)` references  → `![alt](assets/xxx.png)`
//!   • Inline `<svg>…</svg>` blocks          → `![](assets/{sha256}.png)`
//!
//! PNG is used — GIF is limited to 256 colours and looks poor for SVG content.

use sha2::Digest;
use std::path::Path;

// ---------------------------------------------------------------------------
// Pre-processing helpers
// ---------------------------------------------------------------------------

/// Expand CommonMark reference-style image links into inline links so the
/// SVG regex can match them.
///
/// `![alt][label]` + `[label]: url` definition → `![alt](url)`
///
/// This handles the case where the user writes:
/// ```markdown
/// ![My diagram][arch-svg]
/// [arch-svg]: assets/abc123.svg
/// ```
pub fn expand_reference_links(content: &str) -> std::borrow::Cow<'_, str> {
    use std::collections::HashMap;

    // Find all link definitions: `[label]: url` (optional angle-bracket quoting,
    // optional title on the same line, up to 3 leading spaces).
    let def_re = regex::Regex::new(
        r#"(?m)^[ ]{0,3}\[([^\]]+)\]:\s+<?(https?://[^\s>]+|[^\s>]+)>?"#,
    )
    .unwrap();

    let mut defs: HashMap<String, String> = HashMap::new();
    for cap in def_re.captures_iter(content) {
        defs.insert(cap[1].to_lowercase(), cap[2].to_owned());
    }

    if defs.is_empty() {
        return std::borrow::Cow::Borrowed(content);
    }

    // Replace `![alt][label]` (and `![alt][]` — implicit label = alt).
    let img_re = regex::Regex::new(r"!\[([^\]]*)\]\[([^\]]*)\]").unwrap();

    let mut result = String::new();
    let mut last = 0usize;

    for cap in img_re.captures_iter(content) {
        let m = cap.get(0).unwrap();
        result.push_str(&content[last..m.start()]);

        let alt = &cap[1];
        let raw_label = &cap[2];
        let key = if raw_label.is_empty() {
            alt.to_lowercase()
        } else {
            raw_label.to_lowercase()
        };

        if let Some(url) = defs.get(&key) {
            result.push_str(&format!("![{alt}]({url})"));
        } else {
            result.push_str(m.as_str());
        }
        last = m.end();
    }
    result.push_str(&content[last..]);
    std::borrow::Cow::Owned(result)
}

/// Decode HTML entities in the content so that entity-encoded SVG blocks
/// (`&lt;svg&gt;…&lt;/svg&gt;`) are detected by the inline-SVG regex.
///
/// Only applied when `&lt;svg` is present — avoids touching unrelated content.
fn decode_entities_for_svg<'a>(content: &'a str) -> std::borrow::Cow<'a, str> {
    if !content.contains("&lt;svg") && !content.contains("&lt;SVG") {
        return std::borrow::Cow::Borrowed(content);
    }
    // Targeted entity decoding: only the three that affect SVG tag structure.
    let decoded = content
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    std::borrow::Cow::Owned(decoded)
}

pub struct ConvertedAsset {
    pub png_data: Vec<u8>,
    pub sha256_hex: String,
    pub filename: String,
}

/// Rasterize raw SVG bytes to PNG bytes.
/// `max_width` scales proportionally; `None` uses the SVG's native size.
pub fn rasterize_to_png(svg_data: &[u8], max_width: Option<u32>) -> Result<Vec<u8>, String> {
    // Restrict external resource loading (security), but load system fonts
    // so text elements render correctly instead of being invisible.
    let mut opts = resvg::usvg::Options::default();
    opts.resources_dir = None;

    // SVGs that use `currentColor` (very common in diagram generators like
    // Mermaid) need an explicit colour context — resvg defaults to black which
    // makes multi-colour diagrams render as monochrome.  We pre-process the SVG
    // to replace `currentColor` with `#333333` so the rasterised PNG retains
    // the visual intent of the original diagram.
    let svg_str = std::str::from_utf8(svg_data).unwrap_or_default();
    let patched: std::borrow::Cow<str> = if svg_str.contains("currentColor") {
        svg_str.replace("currentColor", "#333333").into()
    } else {
        svg_str.into()
    };

    let tree = resvg::usvg::Tree::from_data(patched.as_bytes(), &opts)
        .map_err(|e| format!("SVG parse error: {e}"))?;

    let native = tree.size().to_int_size();
    let nw = native.width().max(1);
    let nh = native.height().max(1);

    let (w, h) = match max_width {
        Some(mw) if nw > mw => {
            let scale = mw as f32 / nw as f32;
            (mw, ((nh as f32 * scale) as u32).max(1))
        }
        _ => (nw, nh),
    };

    let sx = w as f32 / nw as f32;
    let sy = h as f32 / nh as f32;
    let transform = resvg::tiny_skia::Transform::from_scale(sx, sy);

    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| format!("Cannot allocate {w}×{h} pixmap"))?;

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    pixmap.encode_png().map_err(|e| format!("PNG encode error: {e}"))
}

/// Replace `![alt](…/xxx.svg)` links in `content` with `![alt](…/xxx.png)`.
///
/// Uses position-based iteration (not `str::replace`) so duplicate references
/// are each handled exactly once. Returns (updated_content, assets, errors).
pub fn replace_svg_refs(
    content: &str,
    vault_dir: &Path,
    max_width: Option<u32>,
) -> Result<(String, Vec<ConvertedAsset>, Vec<String>), String> {
    // [^"<>\s]*? matches path chars including '(' and ')' so paths like
    // assets/(old)/diagram.svg are handled correctly (#13).
    // The non-greedy *? ensures we stop at the first .svg boundary.
    let re = regex::Regex::new(r#"!\[([^\]]*)\]\(([^"<>\s]*?\.svg)\)"#)
        .map_err(|e| e.to_string())?;

    let mut out = String::new();
    let mut last_end = 0usize;
    let mut assets: Vec<ConvertedAsset> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for cap in re.captures_iter(content) {
        let m = cap.get(0).unwrap();
        out.push_str(&content[last_end..m.start()]);

        let alt = &cap[1];
        let path_str = &cap[2];

        let filename = Path::new(path_str)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path_str);
        let svg_path = vault_dir.join(filename);

        if !svg_path.exists() {
            // File not in vault — leave the original reference unchanged.
            out.push_str(m.as_str());
            last_end = m.end();
            continue;
        }

        match std::fs::read(&svg_path)
            .map_err(|e| e.to_string())
            .and_then(|svg_data| rasterize_to_png(&svg_data, max_width).map(|png| (svg_data, png)))
        {
            Ok((svg_data, png_data)) => {
                let stem = Path::new(filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("svg");
                let png_filename = format!("{stem}.png");

                // Write PNG to vault alongside the SVG.
                if let Err(e) = std::fs::write(vault_dir.join(&png_filename), &png_data) {
                    errors.push(format!("Write failed for {png_filename}: {e}"));
                    out.push_str(m.as_str());
                    last_end = m.end();
                    continue;
                }

                let sha = {
                    let mut h = sha2::Sha256::new();
                    h.update(&png_data);
                    format!("{:x}", h.finalize())
                };

                // Update the markdown path: same directory, .png extension.
                let new_path = Path::new(path_str)
                    .with_extension("png")
                    .to_string_lossy()
                    .into_owned();
                out.push_str(&format!("![{alt}]({new_path})"));

                assets.push(ConvertedAsset {
                    png_data,
                    sha256_hex: sha,
                    filename: png_filename,
                });

                // Keep the SVG bytes accessible for the original sha tracking if needed.
                let _ = svg_data;
            }
            Err(e) => {
                errors.push(format!("Failed to convert {filename}: {e}"));
                out.push_str(m.as_str());
            }
        }

        last_end = m.end();
    }

    out.push_str(&content[last_end..]);
    Ok((out, assets, errors))
}

/// Replace inline `<svg …>…</svg>` blocks in `content` with PNG img references.
///
/// Also handles entity-encoded blocks (`&lt;svg…&gt;`) by decoding before
/// matching — the returned content will contain the decoded (corrected) form
/// with SVG blocks replaced by PNG references.
///
/// Uses position-based iteration so duplicate blocks are not double-processed.
/// Returns (updated_content, assets, errors).
pub fn replace_inline_svgs(
    content: &str,
    vault_dir: &Path,
    max_width: Option<u32>,
) -> Result<(String, Vec<ConvertedAsset>, Vec<String>), String> {
    // Decode HTML entities so &lt;svg&gt; blocks are found by the regex.
    let working = decode_entities_for_svg(content);
    let content = working.as_ref();

    // (?si) — dot-matches-newline, case-insensitive.
    let re = regex::Regex::new(r"(?si)<svg[^>]*>.*?</svg>")
        .map_err(|e| e.to_string())?;

    let mut out = String::new();
    let mut last_end = 0usize;
    let mut assets: Vec<ConvertedAsset> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for cap in re.captures_iter(content) {
        let m = cap.get(0).unwrap();
        out.push_str(&content[last_end..m.start()]);

        let svg_bytes = m.as_str().as_bytes();

        let sha_of_svg = {
            let mut h = sha2::Sha256::new();
            h.update(svg_bytes);
            format!("{:x}", h.finalize())
        };
        let png_filename = format!("{sha_of_svg}.png");

        match rasterize_to_png(svg_bytes, max_width) {
            Ok(png_data) => {
                if let Err(e) = std::fs::write(vault_dir.join(&png_filename), &png_data) {
                    errors.push(format!("Write failed for {png_filename}: {e}"));
                    out.push_str(m.as_str());
                    last_end = m.end();
                    continue;
                }

                let sha_of_png = {
                    let mut h = sha2::Sha256::new();
                    h.update(&png_data);
                    format!("{:x}", h.finalize())
                };

                out.push_str(&format!("![](assets/{png_filename})"));

                assets.push(ConvertedAsset {
                    png_data,
                    sha256_hex: sha_of_png,
                    filename: png_filename,
                });
            }
            Err(e) => {
                errors.push(format!("Inline SVG render failed: {e}"));
                out.push_str(m.as_str()); // leave original in place
            }
        }

        last_end = m.end();
    }

    out.push_str(&content[last_end..]);
    Ok((out, assets, errors))
}
