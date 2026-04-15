//! Ollama native API provider.
//!
//! Talks to `{base_url}/api/chat` with the Ollama JSON schema, not the
//! OpenAI-compatible one. Use [`crate::openai::OpenAICompatibleProvider`] if
//! you want to hit Ollama's `/v1/chat/completions` shim instead.

use crate::{
    provider::{build_http_client, trim_base},
    ChatMessage, ChatRequest, ChatResponse, ChatRole, EndpointConfig, LlmError, LlmProvider,
    LlmResult, ModelInfo, TokenUsage,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::debug;

/// Provider for Ollama's native HTTP API.
pub struct OllamaProvider {
    config: EndpointConfig,
    client: reqwest::Client,
}

impl OllamaProvider {
    /// Create a new Ollama provider.
    ///
    /// The HTTP client is built lazily if header configuration fails; errors
    /// are surfaced on first use. This keeps the constructor infallible so it
    /// can be used from the [`crate::create_provider`] factory.
    pub fn new(config: EndpointConfig) -> Self {
        let client =
            build_http_client(&config.extra_headers).unwrap_or_else(|_| reqwest::Client::new());
        Self { config, client }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", trim_base(&self.config.base_url), path)
    }

    /// Build the JSON body we send to `/api/chat`.
    pub(crate) fn build_chat_body(&self, req: &ChatRequest) -> Value {
        let model = req
            .model
            .clone()
            .unwrap_or_else(|| self.config.default_model.clone());

        // Prepend a system message if the request carries one in the shortcut
        // field *and* the first message isn't already a system message.
        let mut messages: Vec<&ChatMessage> = Vec::with_capacity(req.messages.len() + 1);
        let prepend_system = req
            .system
            .as_ref()
            .filter(|_| !matches!(req.messages.first().map(|m| m.role), Some(ChatRole::System)));
        let owned_system;
        if let Some(sys) = prepend_system {
            owned_system = ChatMessage::system(sys.clone());
            messages.push(&owned_system);
        }
        messages.extend(req.messages.iter());

        let messages_json: Vec<Value> = messages
            .iter()
            .map(|m| json!({ "role": m.role.as_str(), "content": m.content }))
            .collect();

        let mut options = serde_json::Map::new();
        if let Some(t) = req.temperature {
            options.insert("temperature".into(), json!(t));
        }
        if let Some(mt) = req.max_tokens {
            // Ollama uses `num_predict` for max new tokens.
            options.insert("num_predict".into(), json!(mt));
        }

        let mut body = json!({
            "model": model,
            "messages": messages_json,
            "stream": false,
        });
        if req.json_mode {
            body["format"] = json!("json");
        }
        if !options.is_empty() {
            body["options"] = Value::Object(options);
        }
        body
    }
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    model: String,
    message: OllamaChatMessage,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatMessage {
    #[allow(dead_code)]
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaTag>,
}

#[derive(Debug, Deserialize)]
struct OllamaTag {
    name: String,
    #[serde(default)]
    #[allow(dead_code)]
    model: Option<String>,
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    async fn chat(&self, req: ChatRequest) -> LlmResult<ChatResponse> {
        let url = self.url("/api/chat");
        let body = self.build_chat_body(&req);
        debug!(target: "minion_llm::ollama", %url, "POST /api/chat");

        let resp = self.client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::ProviderHttp { status, body });
        }
        let parsed: OllamaChatResponse = resp.json().await?;
        let usage = match (parsed.prompt_eval_count, parsed.eval_count) {
            (Some(p), Some(c)) => Some(TokenUsage {
                prompt_tokens: p,
                completion_tokens: c,
            }),
            _ => None,
        };
        Ok(ChatResponse {
            content: parsed.message.content,
            model: parsed.model,
            usage,
        })
    }

    async fn health_check(&self) -> LlmResult<bool> {
        let url = self.url("/api/tags");
        debug!(target: "minion_llm::ollama", %url, "GET /api/tags (health)");
        let resp = self.client.get(&url).send().await?;
        Ok(resp.status().is_success())
    }

    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
        let url = self.url("/api/tags");
        debug!(target: "minion_llm::ollama", %url, "GET /api/tags");
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::ProviderHttp { status, body });
        }
        let parsed: OllamaTagsResponse = resp.json().await?;
        Ok(parsed
            .models
            .into_iter()
            .map(|t| ModelInfo {
                id: t.name.clone(),
                name: t.name,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProviderType;

    fn cfg() -> EndpointConfig {
        EndpointConfig::new(ProviderType::Ollama, "http://localhost:11434", "llama3.1")
    }

    #[test]
    fn builds_url_without_trailing_slash() {
        let p = OllamaProvider::new(cfg());
        assert_eq!(p.url("/api/chat"), "http://localhost:11434/api/chat");

        let cfg2 = EndpointConfig::new(ProviderType::Ollama, "http://localhost:11434/", "llama3.1");
        let p2 = OllamaProvider::new(cfg2);
        assert_eq!(p2.url("/api/chat"), "http://localhost:11434/api/chat");
    }

    #[test]
    fn builds_chat_body_with_defaults() {
        let p = OllamaProvider::new(cfg());
        let req = ChatRequest::user_turn("hi");
        let body = p.build_chat_body(&req);
        assert_eq!(body["model"], "llama3.1");
        assert_eq!(body["stream"], false);
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "hi");
        assert!(body.get("format").is_none());
        assert!(body.get("options").is_none());
    }

    #[test]
    fn builds_chat_body_with_json_mode_and_options() {
        let p = OllamaProvider::new(cfg());
        let req = ChatRequest {
            messages: vec![ChatMessage::user("hi")],
            model: Some("qwen2.5".into()),
            temperature: Some(0.2),
            max_tokens: Some(512),
            json_mode: true,
            system: Some("be helpful".into()),
        };
        let body = p.build_chat_body(&req);
        assert_eq!(body["model"], "qwen2.5");
        assert_eq!(body["format"], "json");
        let t = body["options"]["temperature"].as_f64().unwrap();
        assert!((t - 0.2).abs() < 1e-5, "temperature was {t}");
        assert_eq!(body["options"]["num_predict"], 512);
        // system prepended
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "be helpful");
        assert_eq!(body["messages"][1]["role"], "user");
    }

    #[test]
    fn does_not_duplicate_system_when_already_present() {
        let p = OllamaProvider::new(cfg());
        let req = ChatRequest {
            messages: vec![ChatMessage::system("already here"), ChatMessage::user("hi")],
            model: None,
            temperature: None,
            max_tokens: None,
            json_mode: false,
            system: Some("ignored".into()),
        };
        let body = p.build_chat_body(&req);
        assert_eq!(body["messages"].as_array().unwrap().len(), 2);
        assert_eq!(body["messages"][0]["content"], "already here");
    }
}
