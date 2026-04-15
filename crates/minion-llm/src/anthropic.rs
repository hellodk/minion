//! Anthropic Messages API provider.

use crate::{
    provider::{build_http_client, trim_base},
    ChatRequest, ChatResponse, ChatRole, EndpointConfig, LlmError, LlmProvider, LlmResult,
    ModelInfo, TokenUsage,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::debug;

const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Provider for Anthropic's `/v1/messages` API.
pub struct AnthropicProvider {
    config: EndpointConfig,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(config: EndpointConfig) -> Self {
        let client =
            build_http_client(&config.extra_headers).unwrap_or_else(|_| reqwest::Client::new());
        Self { config, client }
    }

    fn url(&self, path: &str) -> String {
        // Default to the public API host if the caller didn't specify one.
        let base = if self.config.base_url.is_empty() {
            "https://api.anthropic.com"
        } else {
            trim_base(&self.config.base_url)
        };
        format!("{base}{path}")
    }

    /// Build the JSON body we send to `/v1/messages`.
    ///
    /// Anthropic takes the system prompt on a top-level `system` field, not
    /// as a message, so we split any leading system messages out.
    pub(crate) fn build_chat_body(&self, req: &ChatRequest) -> Value {
        let model = req
            .model
            .clone()
            .unwrap_or_else(|| self.config.default_model.clone());

        // Collect the system text: explicit shortcut + any leading system msgs.
        let mut system_parts: Vec<String> = Vec::new();
        if let Some(s) = &req.system {
            system_parts.push(s.clone());
        }
        let mut iter = req.messages.iter().peekable();
        while let Some(m) = iter.peek() {
            if m.role == ChatRole::System {
                system_parts.push(m.content.clone());
                iter.next();
            } else {
                break;
            }
        }

        // Remaining messages (user/assistant), passed through as-is.
        let messages_json: Vec<Value> = iter
            .map(|m| json!({ "role": m.role.as_str(), "content": m.content }))
            .collect();

        let mut body = json!({
            "model": model,
            "max_tokens": req.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            "messages": messages_json,
        });
        if !system_parts.is_empty() {
            body["system"] = json!(system_parts.join("\n\n"));
        }
        if let Some(t) = req.temperature {
            body["temperature"] = json!(t);
        }
        // Anthropic doesn't have a JSON-mode flag, but callers can coerce via
        // their system prompt (our default `extract_json` does this).
        let _ = req.json_mode;
        body
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicMessagesResponse {
    #[serde(default)]
    model: Option<String>,
    content: Vec<AnthropicContent>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContent {
    Text {
        text: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn chat(&self, req: ChatRequest) -> LlmResult<ChatResponse> {
        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or_else(|| LlmError::missing_config("Anthropic requires api_key"))?;
        let url = self.url("/v1/messages");
        let body = self.build_chat_body(&req);
        debug!(target: "minion_llm::anthropic", %url, "POST /v1/messages");

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::ProviderHttp { status, body });
        }
        let parsed: AnthropicMessagesResponse = resp.json().await?;
        let text = parsed
            .content
            .into_iter()
            .filter_map(|c| match c {
                AnthropicContent::Text { text } => Some(text),
                AnthropicContent::Other => None,
            })
            .collect::<Vec<_>>()
            .join("");
        if text.is_empty() {
            return Err(LlmError::invalid_response(
                "no text content in Anthropic response",
            ));
        }
        let model_out = parsed.model.unwrap_or_else(|| {
            req.model
                .clone()
                .unwrap_or_else(|| self.config.default_model.clone())
        });
        let usage = parsed.usage.map(|u| TokenUsage {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
        });
        Ok(ChatResponse {
            content: text,
            model: model_out,
            usage,
        })
    }

    async fn health_check(&self) -> LlmResult<bool> {
        // Anthropic has no cheap ping endpoint; issue a minimal request and
        // treat any *authentication-accepted* response (or a well-formed
        // error) as "reachable". We send a 1-token request to stay cheap.
        let api_key = match self.config.api_key.as_deref() {
            Some(k) => k,
            None => return Err(LlmError::missing_config("Anthropic requires api_key")),
        };
        let url = self.url("/v1/messages");
        let body = json!({
            "model": self.config.default_model,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "ping"}],
        });
        let resp = self
            .client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;
        // Count 2xx as healthy. 401/403/429/400 still mean "the server exists
        // and is talking to us", but we treat those as not-healthy so the
        // caller knows their config is wrong.
        Ok(resp.status().is_success())
    }

    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
        // Anthropic has no public "list models" endpoint we can rely on; ship
        // a curated list of currently-shipping model IDs.
        let models = [
            "claude-opus-4-20250514",
            "claude-sonnet-4-20250514",
            "claude-3-7-sonnet-20250219",
            "claude-3-5-sonnet-20241022",
            "claude-3-5-haiku-20241022",
            "claude-3-opus-20240229",
            "claude-3-sonnet-20240229",
            "claude-3-haiku-20240307",
        ];
        Ok(models
            .iter()
            .map(|id| ModelInfo {
                id: (*id).to_string(),
                name: (*id).to_string(),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChatMessage, ProviderType};

    fn cfg() -> EndpointConfig {
        EndpointConfig::new(
            ProviderType::Anthropic,
            "https://api.anthropic.com",
            "claude-sonnet-4-20250514",
        )
        .with_api_key("sk-ant-test")
    }

    #[test]
    fn builds_url() {
        let p = AnthropicProvider::new(cfg());
        assert_eq!(
            p.url("/v1/messages"),
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn empty_base_url_falls_back_to_public_host() {
        let c = EndpointConfig::new(ProviderType::Anthropic, "", "claude-sonnet-4-20250514")
            .with_api_key("sk");
        let p = AnthropicProvider::new(c);
        assert_eq!(
            p.url("/v1/messages"),
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn builds_body_with_system_shortcut() {
        let p = AnthropicProvider::new(cfg());
        let req = ChatRequest {
            messages: vec![ChatMessage::user("hi")],
            model: None,
            temperature: Some(0.3),
            max_tokens: Some(1024),
            json_mode: false,
            system: Some("be concise".into()),
        };
        let body = p.build_chat_body(&req);
        assert_eq!(body["model"], "claude-sonnet-4-20250514");
        assert_eq!(body["max_tokens"], 1024);
        let t = body["temperature"].as_f64().unwrap();
        assert!((t - 0.3).abs() < 1e-5, "temperature was {t}");
        assert_eq!(body["system"], "be concise");
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "hi");
    }

    #[test]
    fn extracts_leading_system_messages_into_top_level_field() {
        let p = AnthropicProvider::new(cfg());
        let req = ChatRequest {
            messages: vec![
                ChatMessage::system("sys1"),
                ChatMessage::system("sys2"),
                ChatMessage::user("hi"),
                ChatMessage::assistant("hello"),
                ChatMessage::user("ok"),
            ],
            model: Some("claude-3-5-haiku-20241022".into()),
            temperature: None,
            max_tokens: None,
            json_mode: false,
            system: Some("shortcut".into()),
        };
        let body = p.build_chat_body(&req);
        assert_eq!(body["model"], "claude-3-5-haiku-20241022");
        assert_eq!(body["system"], "shortcut\n\nsys1\n\nsys2");
        assert_eq!(body["max_tokens"], DEFAULT_MAX_TOKENS);
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[1]["role"], "assistant");
        assert_eq!(msgs[2]["role"], "user");
    }

    #[tokio::test]
    async fn chat_without_api_key_errors() {
        let mut c = cfg();
        c.api_key = None;
        let p = AnthropicProvider::new(c);
        let err = p.chat(ChatRequest::user_turn("hi")).await.unwrap_err();
        matches!(err, LlmError::MissingConfig(_));
    }

    #[tokio::test]
    async fn list_models_is_non_empty() {
        let p = AnthropicProvider::new(cfg());
        let models = p.list_models().await.unwrap();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id.starts_with("claude-")));
    }
}
