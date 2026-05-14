use std::path::Path;
use anyhow::{bail, Context};
use base64ct::{Base64, Encoding};
use minion_llm::{ChatRequest, ChatRole, LlmProvider};

const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;

const VISION_PROMPT: &str =
    "Describe this image in detail for use in a presentation. \
     Focus on: key visual elements, data shown (if a chart/graph), \
     text visible in the image, and the overall message it conveys.";

pub async fn process_image(path: &str, llm: &dyn LlmProvider) -> anyhow::Result<String> {
    let p = Path::new(path);
    let meta = std::fs::metadata(p).with_context(|| format!("cannot stat image file: {path}"))?;
    if meta.len() > MAX_IMAGE_BYTES {
        bail!("image file '{}' is {} bytes, exceeds 25 MB size limit", path, meta.len());
    }
    let bytes = std::fs::read(p).with_context(|| format!("failed to read image file: {path}"))?;
    let mime = infer_mime(path);
    let b64 = Base64::encode_string(&bytes);
    let data_url = format!("data:{mime};base64,{b64}");
    let user_content = format!("{VISION_PROMPT}\n\n[image: {data_url}]");
    let req = ChatRequest {
        messages: vec![minion_llm::ChatMessage { role: ChatRole::User, content: user_content }],
        model: None,
        temperature: Some(0.2),
        max_tokens: Some(1024),
        json_mode: false,
        system: Some(
            "You are a presentation assistant. Describe images accurately and concisely.".into(),
        ),
    };
    let response = llm.chat(req).await.context("vision LLM call failed")?;
    Ok(format!(
        "[Image: {}]\n\n{}",
        p.file_name().unwrap_or_default().to_string_lossy(),
        response.content
    ))
}

fn infer_mime(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else {
        "image/png"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn infer_mime_png() {
        assert_eq!(infer_mime("photo.png"), "image/png");
    }
    #[test]
    fn infer_mime_jpeg() {
        assert_eq!(infer_mime("photo.jpg"), "image/jpeg");
    }
    #[test]
    fn infer_mime_webp() {
        assert_eq!(infer_mime("photo.webp"), "image/webp");
    }
    #[test]
    fn infer_mime_unknown_falls_back_to_png() {
        assert_eq!(infer_mime("photo.bmp"), "image/png");
    }
}
