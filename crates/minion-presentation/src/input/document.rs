use std::path::Path;
use anyhow::{bail, Context};

pub const MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;

fn check_file_size(path: &Path) -> anyhow::Result<u64> {
    let meta = std::fs::metadata(path)
        .with_context(|| format!("cannot stat file: {}", path.display()))?;
    let size = meta.len();
    if size > MAX_FILE_BYTES {
        bail!("file '{}' is {} bytes, exceeds 25 MB size limit", path.display(), size);
    }
    Ok(size)
}

pub fn process_markdown(content: &str) -> String {
    let _event_count = pulldown_cmark::Parser::new(content).count();
    content.to_string()
}

const MAX_XLSX_ROWS: usize = 200;

pub fn process_xlsx(path: &str) -> anyhow::Result<String> {
    use calamine::{open_workbook_auto, Reader};
    let p = Path::new(path);
    check_file_size(p)?;
    let mut workbook = open_workbook_auto(p)
        .with_context(|| format!("failed to open spreadsheet: {path}"))?;
    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
    let mut out = String::new();
    for sheet_name in sheet_names {
        let range = workbook.worksheet_range(&sheet_name)
            .with_context(|| format!("failed to read sheet '{sheet_name}'"))?;
        out.push_str(&format!("### Sheet: {sheet_name}\n\n"));
        for row in range.rows().take(MAX_XLSX_ROWS) {
            let line = row.iter().map(|cell| cell.to_string()).collect::<Vec<_>>().join("\t");
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
    }
    if out.trim().is_empty() { out.push_str("(spreadsheet appears to be empty)"); }
    Ok(out)
}

pub fn process_pdf(path: &str) -> anyhow::Result<String> {
    let p = Path::new(path);
    check_file_size(p)?;
    let doc = lopdf::Document::load(p)
        .with_context(|| format!("failed to load PDF: {path}"))?;
    let page_ids: Vec<lopdf::ObjectId> = doc.get_pages().values().copied().collect();
    let mut pages_text = Vec::with_capacity(page_ids.len());
    for (page_num, &page_id) in page_ids.iter().enumerate() {
        match extract_page_text(&doc, page_id) {
            Ok(text) if !text.trim().is_empty() => {
                pages_text.push(format!("--- Page {} ---\n{}", page_num + 1, text));
            }
            Err(e) => tracing::warn!("PDF page {} extraction failed: {e}", page_num + 1),
            _ => {}
        }
    }
    if pages_text.is_empty() { bail!("PDF '{}' yielded no extractable text", path); }
    Ok(pages_text.join("\n\n"))
}

fn extract_page_text(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> anyhow::Result<String> {
    let content_data = doc.get_page_content(page_id).context("failed to get page content")?;
    let content = lopdf::content::Content::decode(&content_data).context("failed to decode page content")?;
    let mut text = String::new();
    for op in &content.operations {
        match op.operator.as_str() {
            "Tj" | "TJ" => {
                for operand in &op.operands {
                    match operand {
                        lopdf::Object::String(bytes, _) => {
                            if let Ok(s) = std::str::from_utf8(bytes) { text.push_str(s); }
                        }
                        lopdf::Object::Array(arr) => {
                            for item in arr {
                                if let lopdf::Object::String(bytes, _) = item {
                                    if let Ok(s) = std::str::from_utf8(bytes) { text.push_str(s); }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                text.push(' ');
            }
            "Td" | "TD" | "T*" => text.push('\n'),
            _ => {}
        }
    }
    Ok(text)
}

pub fn process_docx(path: &str) -> anyhow::Result<String> {
    use std::io::Read;
    let p = Path::new(path);
    check_file_size(p)?;
    let file = std::fs::File::open(p).with_context(|| format!("failed to open DOCX: {path}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("DOCX '{path}' is not a valid ZIP archive"))?;
    let mut xml_bytes = Vec::new();
    {
        let mut entry = archive.by_name("word/document.xml")
            .context("'word/document.xml' not found — is this a valid DOCX?")?;
        entry.read_to_end(&mut xml_bytes).context("failed to read word/document.xml")?;
    }
    let xml_str = String::from_utf8_lossy(&xml_bytes);
    let text = strip_xml_tags(&xml_str);
    if text.trim().is_empty() { bail!("DOCX '{}' yielded no extractable text", path); }
    Ok(text)
}

fn strip_xml_tags(xml: &str) -> String {
    let mut result = String::with_capacity(xml.len() / 2);
    let mut inside_tag = false;
    for ch in xml.chars() {
        match ch {
            '<' => inside_tag = true,
            '>' => { inside_tag = false; result.push(' '); }
            _ if !inside_tag => result.push(ch),
            _ => {}
        }
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn markdown_returns_source() {
        let md = "# Heading\n\nParagraph.";
        assert_eq!(process_markdown(md), md);
    }
    #[test]
    fn strip_xml_simple() {
        let xml = "<root><w:t>Hello</w:t><w:t>World</w:t></root>";
        let result = strip_xml_tags(xml);
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
        assert!(!result.contains('<'));
    }
}
