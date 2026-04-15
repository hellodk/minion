//! The [`LlmProvider`] trait and dispatch factory.

use crate::{
    ChatMessage, ChatRequest, ChatResponse, ChatRole, EndpointConfig, JsonExtractRequest,
    JsonExtractResponse, LlmResult, ModelInfo, ProviderType,
};
use async_trait::async_trait;

/// A generic LLM backend.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Display name for logs.
    fn name(&self) -> &str;

    /// Standard chat completion.
    async fn chat(&self, req: ChatRequest) -> LlmResult<ChatResponse>;

    /// Structured JSON extraction (preferred for document parsing).
    ///
    /// The default implementation wraps [`chat`](Self::chat) with a JSON-mode
    /// prompt and attempts to parse the response. Providers can override this
    /// when they have a richer native structured-output API.
    async fn extract_json(&self, req: JsonExtractRequest) -> LlmResult<JsonExtractResponse> {
        let system = format!(
            "{}\n\nReturn ONLY valid JSON. No prose. No markdown fences. \
             Match this format exactly:\n{}",
            req.system_prompt, req.example_json
        );
        let chat_req = ChatRequest {
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: req.user_input,
            }],
            model: req.model,
            temperature: Some(req.temperature.unwrap_or(0.0)),
            max_tokens: None,
            json_mode: true,
            system: Some(system),
        };
        let resp = self.chat(chat_req).await?;
        let cleaned = strip_markdown_fences(&resp.content);
        let parsed = serde_json::from_str(&cleaned).unwrap_or(serde_json::Value::Null);
        Ok(JsonExtractResponse {
            raw_text: resp.content,
            parsed,
            model: resp.model,
            usage: resp.usage,
        })
    }

    /// Test if the endpoint is reachable.
    async fn health_check(&self) -> LlmResult<bool>;

    /// List available models (best effort).
    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>>;
}

/// Strip surrounding ``` fences and an optional leading `json` tag from an
/// LLM response so it can be parsed as JSON.
pub(crate) fn strip_markdown_fences(s: &str) -> String {
    let trimmed = s.trim();

    // Remove leading ```json or ``` (optionally followed by a language tag).
    let without_opening = if let Some(rest) = trimmed.strip_prefix("```json") {
        rest.trim_start_matches(['\n', '\r', ' ', '\t'])
    } else if let Some(rest) = trimmed.strip_prefix("```") {
        // Could be ```<lang>\n... — skip until newline.
        if let Some(nl) = rest.find('\n') {
            &rest[nl + 1..]
        } else {
            rest
        }
    } else {
        trimmed
    };

    // Remove trailing ```.
    let without_closing = without_opening
        .trim_end_matches(['\n', '\r', ' ', '\t'])
        .strip_suffix("```")
        .unwrap_or(without_opening);

    without_closing.trim().to_string()
}

/// Factory: create the right provider based on config.
pub fn create_provider(config: EndpointConfig) -> Box<dyn LlmProvider> {
    match config.provider_type {
        ProviderType::Ollama => Box::new(crate::ollama::OllamaProvider::new(config)),
        ProviderType::OpenaiCompatible | ProviderType::Openai | ProviderType::Airllm => {
            Box::new(crate::openai::OpenAICompatibleProvider::new(config))
        }
        ProviderType::Anthropic => Box::new(crate::anthropic::AnthropicProvider::new(config)),
        ProviderType::GoogleGemini => Box::new(crate::gemini::GeminiProvider::new(config)),
    }
}

/// Build a [`reqwest::Client`] with a consistent timeout and default headers.
pub(crate) fn build_http_client(
    extra_headers: &std::collections::HashMap<String, String>,
) -> LlmResult<reqwest::Client> {
    let mut builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(60));

    if !extra_headers.is_empty() {
        let mut header_map = reqwest::header::HeaderMap::new();
        for (k, v) in extra_headers {
            let name = reqwest::header::HeaderName::from_bytes(k.as_bytes()).map_err(|e| {
                crate::LlmError::invalid_request(format!("Invalid header name '{k}': {e}"))
            })?;
            let value = reqwest::header::HeaderValue::from_str(v).map_err(|e| {
                crate::LlmError::invalid_request(format!("Invalid header value for '{k}': {e}"))
            })?;
            header_map.insert(name, value);
        }
        builder = builder.default_headers(header_map);
    }

    builder.build().map_err(crate::LlmError::from)
}

/// Trim a trailing slash from a URL so we can concatenate path segments safely.
pub(crate) fn trim_base(base: &str) -> &str {
    base.trim_end_matches('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_json_fences() {
        let s = "```json\n{\"a\":1}\n```";
        assert_eq!(strip_markdown_fences(s), "{\"a\":1}");
    }

    #[test]
    fn strips_plain_fences() {
        let s = "```\n{\"a\":1}\n```";
        assert_eq!(strip_markdown_fences(s), "{\"a\":1}");
    }

    #[test]
    fn strips_lang_tag_fences() {
        let s = "```javascript\n{\"a\":1}\n```";
        assert_eq!(strip_markdown_fences(s), "{\"a\":1}");
    }

    #[test]
    fn leaves_bare_text_alone() {
        let s = "{\"a\":1}";
        assert_eq!(strip_markdown_fences(s), "{\"a\":1}");
    }

    #[test]
    fn handles_surrounding_whitespace() {
        let s = "\n  ```json\n{\"a\":1}\n```  \n";
        assert_eq!(strip_markdown_fences(s), "{\"a\":1}");
    }

    #[test]
    fn trim_base_strips_trailing_slash() {
        assert_eq!(trim_base("http://x/"), "http://x");
        assert_eq!(trim_base("http://x"), "http://x");
        assert_eq!(trim_base("http://x///"), "http://x");
    }
}
