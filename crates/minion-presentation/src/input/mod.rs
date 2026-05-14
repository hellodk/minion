pub mod document;
pub mod git;
pub mod image;
pub mod text;
pub mod url;

use anyhow::Context;
use minion_llm::LlmProvider;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputSource {
    Text { content: String },
    FilePath { content: String },
    Url { content: String },
    GitUrl { content: String },
}

pub async fn process_all(sources: Vec<InputSource>, llm: &dyn LlmProvider) -> anyhow::Result<String> {
    use futures::future::join_all;
    if sources.is_empty() { return Ok(String::new()); }
    let futures: Vec<_> = sources.into_iter().enumerate()
        .map(|(idx, source)| process_one(idx, source, llm))
        .collect();
    let results = join_all(futures).await;
    let parts: Vec<String> = results.into_iter().enumerate()
        .map(|(idx, res)| match res {
            Ok(text) => text,
            Err(e) => { tracing::warn!("input source {idx} failed: {e:#}"); format!("[Input source {idx} could not be processed: {e}]") }
        })
        .collect();
    Ok(parts.join("\n\n---\n\n"))
}

async fn process_one(_idx: usize, source: InputSource, llm: &dyn LlmProvider) -> anyhow::Result<String> {
    match source {
        InputSource::Text { content } => Ok(text::process_text(&content)),
        InputSource::FilePath { content } => dispatch_file(&content, llm).await,
        InputSource::Url { content } => url::process_url(&content).await
            .with_context(|| format!("URL input failed: {content}")),
        InputSource::GitUrl { content } => crate::security::git_sandbox::summarize_git_repo(&content).await
            .with_context(|| format!("git URL input failed: {content}")),
    }
}

async fn dispatch_file(path: &str, llm: &dyn LlmProvider) -> anyhow::Result<String> {
    let lower = path.to_lowercase();
    if lower.ends_with(".pdf") {
        tokio::task::spawn_blocking({ let p = path.to_owned(); move || document::process_pdf(&p) })
            .await.context("spawn_blocking panicked")?
    } else if lower.ends_with(".docx") {
        tokio::task::spawn_blocking({ let p = path.to_owned(); move || document::process_docx(&p) })
            .await.context("spawn_blocking panicked")?
    } else if lower.ends_with(".md") || lower.ends_with(".markdown") {
        let content = std::fs::read_to_string(path).with_context(|| format!("failed to read MD: {path}"))?;
        Ok(document::process_markdown(&content))
    } else if lower.ends_with(".xlsx") || lower.ends_with(".xls") || lower.ends_with(".ods") || lower.ends_with(".csv") {
        tokio::task::spawn_blocking({ let p = path.to_owned(); move || document::process_xlsx(&p) })
            .await.context("spawn_blocking panicked")?
    } else if lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".webp") || lower.ends_with(".gif") {
        image::process_image(path, llm).await
    } else {
        let content = std::fs::read_to_string(path).with_context(|| format!("unsupported file type: {path}"))?;
        Ok(text::process_text(&content))
    }
}
