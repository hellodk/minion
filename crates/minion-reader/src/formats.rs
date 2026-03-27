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
    })
}

/// Parse an EPUB file
pub fn parse_epub(path: &Path) -> Result<BookContent> {
    let mut doc = epub::doc::EpubDoc::new(path).map_err(|e| Error::Parse(e.to_string()))?;

    let mut chapters = Vec::new();
    let mut index = 0;

    // Get the first chapter (current position)
    if let Some((content, _mime)) = doc.get_current_str() {
        let title = doc
            .get_current_id()
            .unwrap_or_else(|| format!("Chapter {}", index + 1));
        chapters.push(Chapter {
            index,
            title,
            content,
        });
        index += 1;
    }

    // Extract remaining chapters
    while doc.go_next() {
        if let Some((content, _mime)) = doc.get_current_str() {
            let title = doc
                .get_current_id()
                .unwrap_or_else(|| format!("Chapter {}", index + 1));
            chapters.push(Chapter {
                index,
                title,
                content,
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
            children: vec![], // Simplified
        })
        .collect();

    Ok(BookContent { chapters, toc })
}

/// Sanitize HTML content for display
pub fn sanitize_html(html: &str) -> String {
    // Basic sanitization - remove scripts, keep safe elements
    // In production, use a proper HTML sanitizer
    html.replace("<script", "<!--script")
        .replace("</script>", "</script-->")
}
