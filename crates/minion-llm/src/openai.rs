//! OpenAI-compatible chat provider.
//!
//! Covers vanilla OpenAI and every other service speaking its `/chat/completions`
//! dialect (llama.cpp, LM Studio, vLLM, oobabooga, KoboldCpp, Jan.ai, GPT4All,
//! AirLLM via wrapper, Groq, Together, OpenRouter, …).
//!
//! Users are expected to supply the `v1` segment in `base_url` themselves
//! (e.g. `https://api.openai.com/v1`). That way unusual deployments — e.g.
//! Ollama's OpenAI shim at `/v1` or LM Studio's `http://localhost:1234/v1` —
//! all work with the same provider.

use crate::{
    provider::{build_http_client, trim_base},
    ChatMessage, ChatRequest, ChatResponse, ChatRole, EndpointConfig, LlmError, LlmProvider,
    LlmResult, ModelInfo, TokenUsage,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::debug;

/// Provider that speaks OpenAI's chat/completions dialect.
pub struct OpenAICompatibleProvider {
    config: EndpointConfig,
    client: reqwest::Client,
}

impl OpenAICompatibleProvider {
    pub fn new(config: EndpointConfig) -> Self {
        let client =
            build_http_client(&config.extra_headers).unwrap_or_else(|_| reqwest::Client::new());
        Self { config, client }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", trim_base(&self.config.base_url), path)
    }

    /// Build the JSON body we send to `/chat/completions`.
    pub(crate) fn build_chat_body(&self, req: &ChatRequest) -> Value {
        let model = req
            .model
            .clone()
            .unwrap_or_else(|| self.config.default_model.clone());

        let mut messages: Vec<ChatMessage> = Vec::with_capacity(req.messages.len() + 1);
        if let Some(sys) = &req.system {
            if !matches!(req.messages.first().map(|m| m.role), Some(ChatRole::System)) {
                messages.push(ChatMessage::system(sys.clone()));
            }
        }
        messages.extend(req.messages.iter().cloned());

        let messages_json: Vec<Value> = messages
            .iter()
            .map(|m| json!({ "role": m.role.as_str(), "content": m.content }))
            .collect();

        let mut body = json!({
            "model": model,
            "messages": messages_json,
            "stream": false,
        });
        if let Some(t) = req.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(mt) = req.max_tokens {
            body["max_tokens"] = json!(mt);
        }
        if req.json_mode {
            body["response_format"] = json!({ "type": "json_object" });
        }
        body
    }

    fn apply_auth(&self, mut rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(key) = &self.config.api_key {
            rb = rb.bearer_auth(key);
        }
        rb
    }
}

#[derive(Debug, Deserialize)]
struct OpenAIChatResponse {
    #[serde(default)]
    model: Option<String>,
    choices: Vec<OpenAIChoice>,
    #[serde(default)]
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    #[serde(default)]
    message: Option<OpenAIMessage>,
    #[serde(default)]
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIMessage {
    #[serde(default)]
    #[allow(dead_code)]
    role: Option<String>,
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAIModel {
    id: String,
}

#[async_trait]
impl LlmProvider for OpenAICompatibleProvider {
    fn name(&self) -> &str {
        "openai_compatible"
    }

    async fn chat(&self, req: ChatRequest) -> LlmResult<ChatResponse> {
        let url = self.url("/chat/completions");
        let body = self.build_chat_body(&req);
        debug!(target: "minion_llm::openai", %url, "POST /chat/completions");

        let rb = self.client.post(&url).json(&body);
        let rb = self.apply_auth(rb);
        let resp = rb.send().await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::ProviderHttp { status, body });
        }
        let parsed: OpenAIChatResponse = resp.json().await?;
        let model_out = parsed.model.unwrap_or_else(|| {
            req.model
                .clone()
                .unwrap_or_else(|| self.config.default_model.clone())
        });
        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message)
            .and_then(|m| m.content)
            .ok_or_else(|| LlmError::invalid_response("no choices[].message.content"))?;
        let usage = parsed.usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
        });
        Ok(ChatResponse {
            content,
            model: model_out,
            usage,
        })
    }

    async fn health_check(&self) -> LlmResult<bool> {
        let url = self.url("/models");
        debug!(target: "minion_llm::openai", %url, "GET /models (health)");
        let rb = self.client.get(&url);
        let rb = self.apply_auth(rb);
        let resp = rb.send().await?;
        Ok(resp.status().is_success())
    }

    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
        let url = self.url("/models");
        debug!(target: "minion_llm::openai", %url, "GET /models");
        let rb = self.client.get(&url);
        let rb = self.apply_auth(rb);
        let resp = rb.send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::ProviderHttp { status, body });
        }
        let parsed: OpenAIModelsResponse = resp.json().await?;
        Ok(parsed
            .data
            .into_iter()
            .map(|m| ModelInfo {
                id: m.id.clone(),
                name: m.id,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProviderType;

    fn cfg() -> EndpointConfig {
        EndpointConfig::new(
            ProviderType::OpenaiCompatible,
            "https://api.openai.com/v1",
            "gpt-4o",
        )
        .with_api_key("sk-test")
    }

    #[test]
    fn builds_url() {
        let p = OpenAICompatibleProvider::new(cfg());
        assert_eq!(
            p.url("/chat/completions"),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn builds_chat_body_basic() {
        let p = OpenAICompatibleProvider::new(cfg());
        let body = p.build_chat_body(&ChatRequest::user_turn("hi"));
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["stream"], false);
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "hi");
        assert!(body.get("response_format").is_none());
    }

    #[test]
    fn builds_chat_body_with_json_mode_and_params() {
        let p = OpenAICompatibleProvider::new(cfg());
        let req = ChatRequest {
            messages: vec![ChatMessage::user("extract")],
            model: Some("gpt-4o-mini".into()),
            temperature: Some(0.0),
            max_tokens: Some(2048),
            json_mode: true,
            system: Some("you are a robot".into()),
        };
        let body = p.build_chat_body(&req);
        assert_eq!(body["model"], "gpt-4o-mini");
        assert_eq!(body["temperature"], 0.0);
        assert_eq!(body["max_tokens"], 2048);
        assert_eq!(body["response_format"]["type"], "json_object");
        // System prepended.
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "you are a robot");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "extract");
    }

    #[test]
    fn does_not_duplicate_existing_system_message() {
        let p = OpenAICompatibleProvider::new(cfg());
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

    #[test]
    fn airllm_routes_through_openai_compatible() {
        let p = OpenAICompatibleProvider::new(EndpointConfig::new(
            ProviderType::Airllm,
            "http://localhost:8000/v1",
            "llama3",
        ));
        let body = p.build_chat_body(&ChatRequest::user_turn("hi"));
        assert_eq!(body["model"], "llama3");
    }
}
