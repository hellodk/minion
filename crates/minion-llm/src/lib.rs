//! MINION LLM provider abstraction.
//!
//! Unified interface over Ollama, OpenAI-compatible, Anthropic, Gemini
//! and any other backend that speaks chat completions.

pub mod anthropic;
pub mod error;
pub mod gemini;
pub mod ollama;
pub mod openai;
pub mod provider;
pub mod types;

pub use error::{LlmError, LlmResult};
pub use provider::{create_provider, LlmProvider};
pub use types::{
    ChatChoice, ChatMessage, ChatRequest, ChatResponse, ChatRole, EndpointConfig,
    JsonExtractRequest, JsonExtractResponse, ModelInfo, ProviderType, TokenUsage,
};
