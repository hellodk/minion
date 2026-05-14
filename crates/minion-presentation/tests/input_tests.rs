use std::io::Write;

#[test]
fn text_passthrough_returns_content() {
    let result = minion_presentation::input::text::process_text("hello world");
    assert_eq!(result, "hello world");
}

#[test]
fn text_passthrough_preserves_whitespace() {
    let content = "line 1\nline 2\n  indented";
    let result = minion_presentation::input::text::process_text(content);
    assert_eq!(result, content);
}

#[test]
fn markdown_passthrough_returns_raw_source() {
    let md = "# Title\n\nSome **bold** text.";
    let result = minion_presentation::input::document::process_markdown(md);
    assert_eq!(result, md);
}

#[test]
fn xlsx_rejects_oversized_file() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&vec![0u8; 26 * 1024 * 1024]).unwrap();
    tmp.flush().unwrap();
    let result = minion_presentation::input::document::process_xlsx(tmp.path().to_str().unwrap());
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("25 MB") || msg.contains("size"), "error: {msg}");
}

#[test]
fn xlsx_rejects_nonexistent_file() {
    let result = minion_presentation::input::document::process_xlsx("/nonexistent/path/data.xlsx");
    assert!(result.is_err());
}

#[test]
fn pdf_rejects_oversized_file() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&vec![0u8; 26 * 1024 * 1024]).unwrap();
    tmp.flush().unwrap();
    let result = minion_presentation::input::document::process_pdf(tmp.path().to_str().unwrap());
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("25 MB") || msg.contains("size"), "error: {msg}");
}

#[test]
fn docx_rejects_oversized_file() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(&vec![0u8; 26 * 1024 * 1024]).unwrap();
    tmp.flush().unwrap();
    let result = minion_presentation::input::document::process_docx(tmp.path().to_str().unwrap());
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("25 MB") || msg.contains("size"), "error: {msg}");
}

// ── DOCX text extraction ──────────────────────────────────────────────────────

#[test]
fn docx_extracts_text_from_valid_file() {
    let xml_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello from DOCX</w:t></w:r></w:p>
    <w:p><w:r><w:t>Second paragraph.</w:t></w:r></w:p>
  </w:body>
</w:document>"#;
    let tmp = tempfile::NamedTempFile::with_suffix(".docx").unwrap();
    {
        use std::fs::File;
        let file = File::create(tmp.path()).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options: zip::write::FileOptions<()> = Default::default();
        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(xml_content.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    let result = minion_presentation::input::document::process_docx(tmp.path().to_str().unwrap());
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);
    let text = result.unwrap();
    assert!(text.contains("Hello from DOCX"), "text: {text}");
    assert!(text.contains("Second paragraph"), "text: {text}");
}

#[test]
fn docx_rejects_non_zip_file() {
    let mut tmp = tempfile::NamedTempFile::with_suffix(".docx").unwrap();
    tmp.write_all(b"this is not a zip file").unwrap();
    tmp.flush().unwrap();
    let result = minion_presentation::input::document::process_docx(tmp.path().to_str().unwrap());
    assert!(result.is_err(), "expected error for non-ZIP DOCX");
}

#[test]
fn pdf_rejects_non_pdf_file() {
    let mut tmp = tempfile::NamedTempFile::with_suffix(".pdf").unwrap();
    tmp.write_all(b"not a PDF file at all").unwrap();
    tmp.flush().unwrap();
    let result = minion_presentation::input::document::process_pdf(tmp.path().to_str().unwrap());
    assert!(result.is_err(), "expected error for non-PDF content");
}
