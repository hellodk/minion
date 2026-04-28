//! Book format parsers

use crate::{Error, Result};
use std::path::Path;

/// Chapter content
#[derive(Debug, Clone)]
pub struct Chapter {
    pub index: usize,
    pub title: String,
    pub content: String, // HTML content
}

/// Table of contents entry
#[derive(Debug, Clone)]
pub struct TocEntry {
    pub title: String,
    pub href: String,
    pub children: Vec<TocEntry>,
}

/// Parsed book content
pub struct BookContent {
    pub chapters: Vec<Chapter>,
    pub toc: Vec<TocEntry>,
    pub cover_base64: Option<String>,
}

/// Convert raw text to styled HTML paragraphs
fn text_to_html(text: &str) -> String {
    let mut html = String::new();
    let paragraphs: Vec<&str> = text.split("\n\n").collect();

    for para in paragraphs {
        let trimmed = para.trim();
        if trimmed.is_empty() {
            continue;
        }

        let escaped = html_escape::encode_text(trimmed).to_string();

        // Detect potential headings (short lines, often uppercase or title-case)
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() == 1
            && trimmed.len() < 80
            && !trimmed.ends_with('.')
            && !trimmed.ends_with(',')
        {
            let upper_ratio = trimmed.chars().filter(|c| c.is_uppercase()).count() as f64
                / trimmed.chars().filter(|c| c.is_alphabetic()).count().max(1) as f64;

            if upper_ratio > 0.5 || trimmed.chars().all(|c| !c.is_lowercase()) {
                html.push_str(&format!(
                    "<h2 style=\"font-size:1.3em;font-weight:700;margin:1.5em 0 0.5em;\">{}</h2>\n",
                    escaped
                ));
            } else {
                html.push_str(&format!(
                    "<h3 style=\"font-size:1.15em;font-weight:600;margin:1.2em 0 0.4em;\">{}</h3>\n",
                    escaped
                ));
            }
        } else {
            // Regular paragraph — join lines within a paragraph with spaces
            let joined = escaped.replace('\n', " ");
            html.push_str(&format!(
                "<p style=\"margin:0.6em 0;line-height:1.8;text-align:justify;\">{}</p>\n",
                joined
            ));
        }
    }

    if html.is_empty() {
        html.push_str(&format!(
            "<p style=\"line-height:1.8;\">{}</p>",
            html_escape::encode_text(text.trim())
        ));
    }

    html
}

/// Parse a PDF file
pub fn parse_pdf(path: &Path) -> Result<BookContent> {
    let text = pdf_extract::extract_text(path)
        .map_err(|e| Error::Parse(format!("Failed to extract PDF text: {}", e)))?;

    // Split into pages by form-feed characters (common in PDFs) or triple newlines
    let page_separators = if text.contains('\u{000C}') {
        text.split('\u{000C}').collect::<Vec<_>>()
    } else {
        text.split("\n\n\n").collect::<Vec<_>>()
    };

    let mut chapters = Vec::new();

    // Group pages into chapters of reasonable size
    let pages_per_chapter = 5;
    let mut page_group = Vec::new();
    let mut chapter_idx = 0;

    for (i, page) in page_separators.iter().enumerate() {
        let trimmed = page.trim();
        if trimmed.is_empty() {
            continue;
        }
        page_group.push(trimmed);

        if page_group.len() >= pages_per_chapter || i == page_separators.len() - 1 {
            let combined = page_group.join("\n\n");
            let html_content = text_to_html(&combined);

            chapters.push(Chapter {
                index: chapter_idx,
                title: if page_separators.len() <= pages_per_chapter {
                    "Full Document".to_string()
                } else {
                    format!("Pages {}-{}", chapter_idx * pages_per_chapter + 1, chapter_idx * pages_per_chapter + page_group.len())
                },
                content: format!(
                    "<div style=\"font-family:Georgia,'Times New Roman',serif;max-width:100%;\">{}</div>",
                    html_content
                ),
            });
            chapter_idx += 1;
            page_group.clear();
        }
    }

    // If no chapters were created, create one with the full text
    if chapters.is_empty() {
        let html_content = text_to_html(&text);
        chapters.push(Chapter {
            index: 0,
            title: "Content".to_string(),
            content: format!(
                "<div style=\"font-family:Georgia,'Times New Roman',serif;max-width:100%;\">{}</div>",
                html_content
            ),
        });
    }

    Ok(BookContent {
        chapters,
        toc: vec![],
        cover_base64: None,
    })
}

/// Guess MIME type from file path
fn mime_from_path(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}

/// Replace image src attributes with base64 data URIs from EPUB resources
fn inline_epub_images(
    html: &str,
    doc: &mut epub::doc::EpubDoc<std::io::BufReader<std::fs::File>>,
) -> String {
    use std::collections::HashMap;

    // Build a map of resource paths to IDs for lookup
    let mut path_to_id: HashMap<String, String> = HashMap::new();
    for (id, item) in doc.resources.iter() {
        let path_str = item.path.to_string_lossy().to_string();
        path_to_id.insert(path_str.clone(), id.clone());
        // Also store just the filename
        if let Some(fname) = item.path.file_name() {
            path_to_id.insert(fname.to_string_lossy().to_string(), id.clone());
        }
    }

    let mut image_cache: HashMap<String, String> = HashMap::new();
    let mut result = html.to_string();
    let mut search_start = 0;

    loop {
        let src_pos = result[search_start..].find("src=\"");
        if src_pos.is_none() {
            break;
        }
        let abs_pos = search_start + src_pos.unwrap() + 4;
        let end_quote = result[abs_pos..].find('"');
        if end_quote.is_none() {
            break;
        }
        let src_value = result[abs_pos..abs_pos + end_quote.unwrap()].to_string();

        if src_value.starts_with("data:") || src_value.starts_with("http") {
            search_start = abs_pos + end_quote.unwrap();
            continue;
        }

        let data_uri = if let Some(cached) = image_cache.get(&src_value) {
            cached.clone()
        } else {
            let resource_name = src_value
                .rsplit('/')
                .next()
                .unwrap_or(&src_value)
                .to_string();

            // Find the resource ID
            let resource_id = path_to_id
                .get(&src_value)
                .or_else(|| path_to_id.get(&resource_name))
                .cloned();

            if let Some(ref id) = resource_id {
                if let Some((data, _mime_str)) = doc.get_resource(id) {
                    let mime = mime_from_path(&resource_name);
                    let b64 = base64_encode(&data);
                    let uri = format!("data:{};base64,{}", mime, b64);
                    image_cache.insert(src_value.clone(), uri.clone());
                    uri
                } else {
                    search_start = abs_pos + end_quote.unwrap();
                    continue;
                }
            } else {
                search_start = abs_pos + end_quote.unwrap();
                continue;
            }
        };

        result = format!(
            "{}{}{}",
            &result[..abs_pos],
            data_uri,
            &result[abs_pos + end_quote.unwrap()..]
        );
        search_start = abs_pos + data_uri.len();
    }

    result
}

/// Simple base64 encoder (no external dep needed)
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

/// Parse an EPUB file
pub fn parse_epub(path: &Path) -> Result<BookContent> {
    let mut doc = epub::doc::EpubDoc::new(path).map_err(|e| Error::Parse(e.to_string()))?;

    // Extract cover image
    let cover_base64 = doc.get_cover().map(|(cover_data, _mime_str)| {
        let mime = doc
            .get_cover_id()
            .and_then(|id| {
                doc.resources
                    .get(&id)
                    .map(|item| mime_from_path(&item.path.to_string_lossy()))
            })
            .unwrap_or("image/jpeg");
        format!("data:{};base64,{}", mime, base64_encode(&cover_data))
    });

    let mut chapters = Vec::new();
    let mut index = 0;

    // Get the first chapter (current position)
    if let Some((content, _mime)) = doc.get_current_str() {
        let title = doc
            .get_current_id()
            .unwrap_or_else(|| format!("Chapter {}", index + 1));
        let content_with_images = inline_epub_images(&content, &mut doc);
        chapters.push(Chapter {
            index,
            title,
            content: content_with_images,
        });
        index += 1;
    }

    // Extract remaining chapters
    while doc.go_next() {
        if let Some((content, _mime)) = doc.get_current_str() {
            let title = doc
                .get_current_id()
                .unwrap_or_else(|| format!("Chapter {}", index + 1));
            let content_with_images = inline_epub_images(&content, &mut doc);
            chapters.push(Chapter {
                index,
                title,
                content: content_with_images,
            });
            index += 1;
        }
    }

    // Extract TOC
    let toc = doc
        .toc
        .iter()
        .map(|entry| TocEntry {
            title: entry.label.clone(),
            href: entry.content.to_string_lossy().to_string(),
            children: vec![],
        })
        .collect();

    Ok(BookContent {
        chapters,
        toc,
        cover_base64,
    })
}

/// Sanitize HTML content for display
pub fn sanitize_html(html: &str) -> String {
    ammonia::Builder::default()
        .add_generic_attributes(&["style", "class", "id"])
        .add_url_schemes(&["file"])
        .clean(html)
        .to_string()
}
