//! Shared request/response types for the LLM abstraction.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Which backend family an endpoint talks to.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    /// Native Ollama API at `/api/generate` and `/api/chat`.
    Ollama,
    /// Any OpenAI-compatible `/v1/chat/completions` endpoint (llama.cpp, LM
    /// Studio, vLLM, oobabooga, KoboldCpp, Jan.ai, GPT4All, ...).
    OpenaiCompatible,
    /// Anthropic Messages API at `/v1/messages`.
    Anthropic,
    /// Vanilla OpenAI (routed through the OpenAI-compatible provider).
    Openai,
    /// Google Gemini `generateContent` API.
    GoogleGemini,
    /// AirLLM — routed through the OpenAI-compatible provider via its HTTP wrapper.
    Airllm,
}

/// Configuration for a single endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointConfig {
    pub provider_type: ProviderType,
    pub base_url: String,
    pub api_key: Option<String>,
    pub default_model: String,
    #[serde(default)]
    pub extra_headers: HashMap<String, String>,
}

impl EndpointConfig {
    /// Build a minimal config with no API key and no extra headers.
    pub fn new<S1: Into<String>, S2: Into<String>>(
        provider_type: ProviderType,
        base_url: S1,
        default_model: S2,
    ) -> Self {
        Self {
            provider_type,
            base_url: base_url.into(),
            api_key: None,
            default_model: default_model.into(),
            extra_headers: HashMap::new(),
        }
    }

    /// Attach an API key.
    pub fn with_api_key<S: Into<String>>(mut self, api_key: S) -> Self {
        self.api_key = Some(api_key.into());
        self
    }
}

/// Role of a chat message.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

impl ChatRole {
    /// String representation used by OpenAI-compatible / Ollama APIs.
    pub fn as_str(self) -> &'static str {
        match self {
            ChatRole::System => "system",
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
        }
    }
}

/// A single chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

impl ChatMessage {
    pub fn system<S: Into<String>>(content: S) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }
    pub fn user<S: Into<String>>(content: S) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }
    pub fn assistant<S: Into<String>>(content: S) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
        }
    }
}

/// A chat-completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    /// Overrides the endpoint's default model if set.
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    /// Ask the provider to produce structured JSON output (uses provider's
    /// native JSON mode when available).
    #[serde(default)]
    pub json_mode: bool,
    /// Shortcut to prepend a system message. Providers that separate the
    /// system channel (Anthropic, Gemini) forward this there.
    pub system: Option<String>,
}

impl ChatRequest {
    /// Simple "single user turn" request.
    pub fn user_turn<S: Into<String>>(content: S) -> Self {
        Self {
            messages: vec![ChatMessage::user(content)],
            model: None,
            temperature: None,
            max_tokens: None,
            json_mode: false,
            system: None,
        }
    }
}

/// An OpenAI-style `choices[]` entry. Kept as a public type for callers who
/// want to surface multiple completions in the future.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

/// Token usage reported by the provider, when available.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

/// A normalized chat response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub usage: Option<TokenUsage>,
}

/// "Give me JSON matching this schema" request (e.g., extracting lab values
/// from a PDF page).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonExtractRequest {
    /// Describes the task for the model.
    pub system_prompt: String,
    /// The document text to process.
    pub user_input: String,
    /// Example of the desired JSON format.
    pub example_json: String,
    pub model: Option<String>,
    /// Defaults to 0.0 in the provider default implementation.
    pub temperature: Option<f32>,
}

/// Response from a JSON extraction call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonExtractResponse {
    /// Exact LLM output (before cleanup).
    pub raw_text: String,
    /// Attempted JSON parse. `Null` if parsing failed.
    pub parsed: serde_json::Value,
    pub model: String,
    pub usage: Option<TokenUsage>,
}

/// Summary information about an available model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
}
