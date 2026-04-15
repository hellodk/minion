//! Google Gemini `generateContent` provider.

use crate::{
    provider::{build_http_client, trim_base},
    ChatMessage, ChatRequest, ChatResponse, ChatRole, EndpointConfig, LlmError, LlmProvider,
    LlmResult, ModelInfo, TokenUsage,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::debug;

/// Provider for Google Gemini's REST API.
pub struct GeminiProvider {
    config: EndpointConfig,
    client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(config: EndpointConfig) -> Self {
        let client =
            build_http_client(&config.extra_headers).unwrap_or_else(|_| reqwest::Client::new());
        Self { config, client }
    }

    fn base(&self) -> &str {
        if self.config.base_url.is_empty() {
            "https://generativelanguage.googleapis.com"
        } else {
            trim_base(&self.config.base_url)
        }
    }

    /// Build the `generateContent` URL for `model`, appending the API key.
    pub(crate) fn generate_content_url(&self, model: &str) -> String {
        let key = self.config.api_key.as_deref().unwrap_or("");
        format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            self.base(),
            model,
            key
        )
    }

    pub(crate) fn list_models_url(&self) -> String {
        let key = self.config.api_key.as_deref().unwrap_or("");
        format!("{}/v1beta/models?key={}", self.base(), key)
    }

    /// Build the JSON body we send to `generateContent`.
    pub(crate) fn build_body(&self, req: &ChatRequest) -> Value {
        // Gemini roles are "user" and "model".
        fn role_for(r: ChatRole) -> &'static str {
            match r {
                ChatRole::User => "user",
                ChatRole::Assistant => "model",
                // Gemini has no "system" role inside `contents` — the caller
                // should have used `ChatRequest.system`. Treat leftover system
                // messages as user as a best-effort fallback; `build_body`
                // actually extracts them above, so this arm is unreachable in
                // practice.
                ChatRole::System => "user",
            }
        }

        // Split out system messages.
        let mut system_text: Vec<String> = Vec::new();
        if let Some(s) = &req.system {
            system_text.push(s.clone());
        }
        let mut non_system: Vec<&ChatMessage> = Vec::with_capacity(req.messages.len());
        for m in &req.messages {
            if m.role == ChatRole::System {
                system_text.push(m.content.clone());
            } else {
                non_system.push(m);
            }
        }

        let contents: Vec<Value> = non_system
            .iter()
            .map(|m| {
                json!({
                    "role": role_for(m.role),
                    "parts": [{ "text": m.content }],
                })
            })
            .collect();

        let mut body = json!({ "contents": contents });
        if !system_text.is_empty() {
            body["systemInstruction"] = json!({
                "parts": [{ "text": system_text.join("\n\n") }],
            });
        }

        let mut gen_cfg = serde_json::Map::new();
        if let Some(t) = req.temperature {
            gen_cfg.insert("temperature".into(), json!(t));
        }
        if let Some(mt) = req.max_tokens {
            gen_cfg.insert("maxOutputTokens".into(), json!(mt));
        }
        if req.json_mode {
            gen_cfg.insert("responseMimeType".into(), json!("application/json"));
        }
        if !gen_cfg.is_empty() {
            body["generationConfig"] = Value::Object(gen_cfg);
        }
        body
    }
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
    #[serde(default, rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsage>,
    #[serde(default, rename = "modelVersion")]
    model_version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    #[serde(default)]
    content: Option<GeminiContent>,
}

#[derive(Debug, Deserialize)]
struct GeminiContent {
    #[serde(default)]
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Deserialize)]
struct GeminiPart {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiUsage {
    #[serde(default, rename = "promptTokenCount")]
    prompt_token_count: u32,
    #[serde(default, rename = "candidatesTokenCount")]
    candidates_token_count: u32,
}

#[derive(Debug, Deserialize)]
struct GeminiModelsResponse {
    #[serde(default)]
    models: Vec<GeminiModel>,
}

#[derive(Debug, Deserialize)]
struct GeminiModel {
    name: String,
    #[serde(default, rename = "displayName")]
    display_name: Option<String>,
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    fn name(&self) -> &str {
        "google_gemini"
    }

    async fn chat(&self, req: ChatRequest) -> LlmResult<ChatResponse> {
        if self.config.api_key.is_none() {
            return Err(LlmError::missing_config("Gemini requires api_key"));
        }
        let model = req
            .model
            .clone()
            .unwrap_or_else(|| self.config.default_model.clone());
        let url = self.generate_content_url(&model);
        let body = self.build_body(&req);
        debug!(target: "minion_llm::gemini", model = %model, "POST generateContent");

        let resp = self.client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::ProviderHttp { status, body });
        }
        let parsed: GeminiResponse = resp.json().await?;
        let text = parsed
            .candidates
            .into_iter()
            .filter_map(|c| c.content)
            .flat_map(|c| c.parts.into_iter())
            .filter_map(|p| p.text)
            .collect::<Vec<_>>()
            .join("");
        if text.is_empty() {
            return Err(LlmError::invalid_response("no text in Gemini response"));
        }
        let usage = parsed.usage_metadata.map(|u| TokenUsage {
            prompt_tokens: u.prompt_token_count,
            completion_tokens: u.candidates_token_count,
        });
        Ok(ChatResponse {
            content: text,
            model: parsed.model_version.unwrap_or(model),
            usage,
        })
    }

    async fn health_check(&self) -> LlmResult<bool> {
        if self.config.api_key.is_none() {
            return Ok(false);
        }
        let url = self.list_models_url();
        debug!(target: "minion_llm::gemini", "GET /v1beta/models (health)");
        let resp = self.client.get(&url).send().await?;
        Ok(resp.status().is_success())
    }

    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
        if self.config.api_key.is_none() {
            return Err(LlmError::missing_config("Gemini requires api_key"));
        }
        let url = self.list_models_url();
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::ProviderHttp { status, body });
        }
        let parsed: GeminiModelsResponse = resp.json().await?;
        Ok(parsed
            .models
            .into_iter()
            .map(|m| {
                // `name` is like "models/gemini-1.5-pro"; strip the prefix.
                let id = m
                    .name
                    .strip_prefix("models/")
                    .unwrap_or(&m.name)
                    .to_string();
                let display = m.display_name.unwrap_or_else(|| id.clone());
                ModelInfo { id, name: display }
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
            ProviderType::GoogleGemini,
            "https://generativelanguage.googleapis.com",
            "gemini-1.5-flash",
        )
        .with_api_key("AIza-test")
    }

    #[test]
    fn builds_generate_content_url() {
        let p = GeminiProvider::new(cfg());
        let url = p.generate_content_url("gemini-1.5-pro");
        assert_eq!(
            url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-pro:generateContent?key=AIza-test"
        );
    }

    #[test]
    fn empty_base_url_falls_back_to_default_host() {
        let c = EndpointConfig::new(ProviderType::GoogleGemini, "", "gemini-1.5-flash")
            .with_api_key("k");
        let p = GeminiProvider::new(c);
        assert!(p
            .generate_content_url("gemini-1.5-flash")
            .starts_with("https://generativelanguage.googleapis.com/"));
    }

    #[test]
    fn builds_body_with_system_instruction() {
        let p = GeminiProvider::new(cfg());
        let req = ChatRequest {
            messages: vec![
                ChatMessage::user("hi"),
                ChatMessage::assistant("hey"),
                ChatMessage::user("tell me a joke"),
            ],
            model: None,
            temperature: Some(0.7),
            max_tokens: Some(500),
            json_mode: false,
            system: Some("be funny".into()),
        };
        let body = p.build_body(&req);
        assert_eq!(body["systemInstruction"]["parts"][0]["text"], "be funny");
        let t = body["generationConfig"]["temperature"].as_f64().unwrap();
        assert!((t - 0.7).abs() < 1e-5, "temperature was {t}");
        assert_eq!(body["generationConfig"]["maxOutputTokens"], 500);
        let contents = body["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 3);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[1]["role"], "model");
        assert_eq!(contents[2]["role"], "user");
        assert_eq!(contents[0]["parts"][0]["text"], "hi");
        assert_eq!(contents[1]["parts"][0]["text"], "hey");
    }

    #[test]
    fn builds_body_with_json_mode() {
        let p = GeminiProvider::new(cfg());
        let req = ChatRequest {
            messages: vec![ChatMessage::user("extract")],
            model: None,
            temperature: None,
            max_tokens: None,
            json_mode: true,
            system: None,
        };
        let body = p.build_body(&req);
        assert_eq!(
            body["generationConfig"]["responseMimeType"],
            "application/json"
        );
        assert!(body.get("systemInstruction").is_none());
    }

    #[test]
    fn extracts_system_messages_from_conversation() {
        let p = GeminiProvider::new(cfg());
        let req = ChatRequest {
            messages: vec![
                ChatMessage::system("sys1"),
                ChatMessage::user("hi"),
                ChatMessage::system("sys2 mid-stream"),
            ],
            model: None,
            temperature: None,
            max_tokens: None,
            json_mode: false,
            system: Some("shortcut".into()),
        };
        let body = p.build_body(&req);
        assert_eq!(
            body["systemInstruction"]["parts"][0]["text"],
            "shortcut\n\nsys1\n\nsys2 mid-stream"
        );
        let contents = body["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[0]["parts"][0]["text"], "hi");
    }

    #[tokio::test]
    async fn chat_without_api_key_errors() {
        let c = EndpointConfig::new(ProviderType::GoogleGemini, "", "gemini-1.5-flash");
        let p = GeminiProvider::new(c);
        let err = p.chat(ChatRequest::user_turn("hi")).await.unwrap_err();
        matches!(err, LlmError::MissingConfig(_));
    }
}
