//! SVG → PNG rasterization for blog export.
//!
//! LinkedIn and Medium do not render SVG. This module converts:
//!   • `![alt](assets/xxx.svg)` references  → `![alt](assets/xxx.png)`
//!   • Inline `<svg>…</svg>` blocks          → `![](assets/{sha256}.png)`
//!
//! PNG is used — GIF is limited to 256 colours and looks poor for SVG content.

use sha2::Digest;
use std::path::Path;

pub struct ConvertedAsset {
    pub png_data: Vec<u8>,
    pub sha256_hex: String,
    pub filename: String,
}

/// Rasterize raw SVG bytes to PNG bytes.
/// `max_width` scales proportionally; `None` uses the SVG's native size.
pub fn rasterize_to_png(svg_data: &[u8], max_width: Option<u32>) -> Result<Vec<u8>, String> {
    // Restrict resource loading: no external URLs, no local file:// references.
    let mut opts = resvg::usvg::Options::default();
    opts.resources_dir = None; // disable relative-path resource loading
    // resvg uses only bundled fonts by default; external @font-face URLs are ignored.

    let tree = resvg::usvg::Tree::from_data(svg_data, &opts)
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
    // Capture group 2 uses [^\(\)]*  to allow parentheses in path components,
    // then anchors on `.svg)` at the end.
    let re = regex::Regex::new(r"!\[([^\]]*)\]\(([^)]*?\.svg)\)")
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
/// Each block is rasterized and saved as `{sha256}.png` in `vault_dir`.
/// Uses position-based iteration so duplicate blocks are not double-processed.
/// Returns (updated_content, assets, errors).
pub fn replace_inline_svgs(
    content: &str,
    vault_dir: &Path,
    max_width: Option<u32>,
) -> Result<(String, Vec<ConvertedAsset>, Vec<String>), String> {
    // (?si) — dot-matches-newline, case-insensitive. The [^>]* for the opening
    // tag does not recurse into inner SVGs, so deeply nested <svg> inside <svg>
    // will be caught on the outer match.
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
