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

// ── Image processor ───────────────────────────────────────────────────────────

#[test]
fn image_rejects_oversized_file() {
    use std::io::Write;
    let mut tmp = tempfile::NamedTempFile::with_suffix(".png").unwrap();
    tmp.write_all(&vec![0u8; 26 * 1024 * 1024]).unwrap();
    tmp.flush().unwrap();

    struct PanicLlm;
    #[async_trait::async_trait]
    impl minion_llm::LlmProvider for PanicLlm {
        fn name(&self) -> &str { "panic-llm" }
        async fn chat(&self, _: minion_llm::ChatRequest) -> minion_llm::LlmResult<minion_llm::ChatResponse> {
            panic!("LLM should not be called for oversized image")
        }
        async fn health_check(&self) -> minion_llm::LlmResult<bool> { Ok(true) }
        async fn list_models(&self) -> minion_llm::LlmResult<Vec<minion_llm::ModelInfo>> { Ok(vec![]) }
    }

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(
        minion_presentation::input::image::process_image(tmp.path().to_str().unwrap(), &PanicLlm)
    );
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("25 MB") || msg.contains("size"), "error: {msg}");
}

// ── URL processor ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn url_rejects_private_ip() {
    let result = minion_presentation::input::url::process_url("http://192.168.0.1/secret").await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("SSRF") || msg.contains("private") || msg.contains("blocked"), "error: {msg}");
}

#[tokio::test]
async fn url_rejects_file_scheme() {
    let result = minion_presentation::input::url::process_url("file:///etc/passwd").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn url_rejects_localhost() {
    let result = minion_presentation::input::url::process_url("http://localhost/").await;
    assert!(result.is_err());
}

// ── process_all smoke tests ───────────────────────────────────────────────────

use minion_presentation::input::{process_all, InputSource};

struct NoopLlm;

#[async_trait::async_trait]
impl minion_llm::LlmProvider for NoopLlm {
    fn name(&self) -> &str { "noop-llm" }
    async fn chat(&self, _req: minion_llm::ChatRequest) -> minion_llm::LlmResult<minion_llm::ChatResponse> {
        Ok(minion_llm::ChatResponse { content: "(noop)".into(), model: "noop".into(), usage: None })
    }
    async fn health_check(&self) -> minion_llm::LlmResult<bool> { Ok(true) }
    async fn list_models(&self) -> minion_llm::LlmResult<Vec<minion_llm::ModelInfo>> { Ok(vec![]) }
}

#[tokio::test]
async fn process_all_text_and_markdown_sources() {
    use std::io::Write;
    let mut md_file = tempfile::NamedTempFile::with_suffix(".md").unwrap();
    writeln!(md_file, "# Test Slide\n\nThis is the slide content.").unwrap();
    md_file.flush().unwrap();
    let md_path = md_file.path().to_str().unwrap().to_string();
    let sources = vec![
        InputSource::Text { content: "Hello from text input".into() },
        InputSource::FilePath { content: md_path },
    ];
    let result = process_all(sources, &NoopLlm).await;
    assert!(result.is_ok(), "process_all failed: {:?}", result);
    let corpus = result.unwrap();
    assert!(corpus.contains("Hello from text input"), "text source missing: {corpus}");
    assert!(corpus.contains("Test Slide"), "markdown missing: {corpus}");
    assert!(corpus.contains("---"), "separator missing: {corpus}");
}

#[tokio::test]
async fn process_all_empty_sources_returns_empty_string() {
    let result = process_all(vec![], &NoopLlm).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

#[tokio::test]
async fn process_all_failing_source_does_not_abort_others() {
    let sources = vec![
        InputSource::Text { content: "good text".into() },
        InputSource::Url { content: "http://192.168.1.1/bad".into() },
    ];
    let result = process_all(sources, &NoopLlm).await;
    assert!(result.is_ok(), "should not abort on partial failure");
    let corpus = result.unwrap();
    assert!(corpus.contains("good text"), "successful source missing: {corpus}");
    assert!(corpus.contains("could not be processed") || corpus.contains("Input source"), "error placeholder missing: {corpus}");
}
