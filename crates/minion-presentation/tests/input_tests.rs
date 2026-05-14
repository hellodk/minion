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
