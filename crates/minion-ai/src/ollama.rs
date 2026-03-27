//! Ollama client for local LLM inference

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::{AIConfig, Error, Result};

/// Ollama client
pub struct OllamaClient {
    client: Client,
    base_url: String,
    default_model: String,
}

/// Generation request
#[derive(Debug, Serialize)]
pub struct GenerateRequest {
    pub model: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<GenerateOptions>,
}

/// Generation options
#[derive(Debug, Serialize)]
pub struct GenerateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_predict: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_ctx: Option<i32>,
}

/// Generation response
#[derive(Debug, Deserialize)]
pub struct GenerateResponse {
    pub model: String,
    pub response: String,
    pub done: bool,
    #[serde(default)]
    pub total_duration: u64,
    #[serde(default)]
    pub eval_count: u32,
}

/// Embedding request
#[derive(Debug, Serialize)]
pub struct EmbedRequest {
    pub model: String,
    pub prompt: String,
}

/// Embedding response
#[derive(Debug, Deserialize)]
pub struct EmbedResponse {
    pub embedding: Vec<f32>,
}

impl OllamaClient {
    /// Create a new Ollama client
    pub fn new(config: &AIConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: format!("http://{}:{}", config.ollama_host, config.ollama_port),
            default_model: config.default_model.clone(),
        }
    }

    /// Generate text completion
    pub async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse> {
        let url = format!("{}/api/generate", self.base_url);

        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(Error::Ollama(error));
        }

        let result: GenerateResponse = response.json().await?;
        Ok(result)
    }

    /// Generate text with simple prompt
    pub async fn complete(&self, prompt: &str) -> Result<String> {
        let request = GenerateRequest {
            model: self.default_model.clone(),
            prompt: prompt.to_string(),
            system: None,
            template: None,
            stream: false,
            options: None,
        };

        let response = self.generate(request).await?;
        Ok(response.response)
    }

    /// Generate embeddings
    pub async fn embed(&self, model: &str, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.base_url);

        let request = EmbedRequest {
            model: model.to_string(),
            prompt: text.to_string(),
        };

        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(Error::Embedding(error));
        }

        let result: EmbedResponse = response.json().await?;
        Ok(result.embedding)
    }

    /// Check if Ollama is available
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.base_url);

        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// List available models
    pub async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.base_url);

        let response = self.client.get(&url).send().await?;

        #[derive(Deserialize)]
        struct TagsResponse {
            models: Vec<ModelInfo>,
        }

        #[derive(Deserialize)]
        struct ModelInfo {
            name: String,
        }

        let result: TagsResponse = response.json().await?;
        Ok(result.models.into_iter().map(|m| m.name).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_client_creation() {
        let config = AIConfig::default();
        let client = OllamaClient::new(&config);

        // Verify base_url is constructed correctly
        assert_eq!(client.base_url, "http://127.0.0.1:11434");
        assert_eq!(client.default_model, "llama3.2:3b");
    }

    #[test]
    fn test_ollama_client_custom_config() {
        let config = AIConfig {
            ollama_host: "localhost".to_string(),
            ollama_port: 8080,
            default_model: "mistral:7b".to_string(),
            embedding_model: "all-minilm".to_string(),
            timeout_seconds: 60,
        };
        let client = OllamaClient::new(&config);

        assert_eq!(client.base_url, "http://localhost:8080");
        assert_eq!(client.default_model, "mistral:7b");
    }

    #[test]
    fn test_generate_request_serialization() {
        let request = GenerateRequest {
            model: "llama3.2:3b".to_string(),
            prompt: "Hello, world!".to_string(),
            system: Some("You are a helpful assistant.".to_string()),
            template: None,
            stream: false,
            options: Some(GenerateOptions {
                temperature: Some(0.7),
                top_p: Some(0.9),
                top_k: Some(40),
                num_predict: Some(100),
                num_ctx: Some(2048),
            }),
        };

        let json = serde_json::to_string(&request).expect("Failed to serialize");
        assert!(json.contains("\"model\":\"llama3.2:3b\""));
        assert!(json.contains("\"prompt\":\"Hello, world!\""));
        assert!(json.contains("\"system\":"));
        assert!(json.contains("\"temperature\":0.7"));
        assert!(!json.contains("\"template\":")); // None should be skipped
    }

    #[test]
    fn test_generate_request_minimal() {
        let request = GenerateRequest {
            model: "test".to_string(),
            prompt: "test prompt".to_string(),
            system: None,
            template: None,
            stream: false,
            options: None,
        };

        let json = serde_json::to_string(&request).expect("Failed to serialize");
        // None fields should not be present
        assert!(!json.contains("\"system\""));
        assert!(!json.contains("\"template\""));
        assert!(!json.contains("\"options\""));
    }

    #[test]
    fn test_generate_options_partial() {
        let options = GenerateOptions {
            temperature: Some(0.5),
            top_p: None,
            top_k: None,
            num_predict: Some(50),
            num_ctx: None,
        };

        let json = serde_json::to_string(&options).expect("Failed to serialize");
        assert!(json.contains("\"temperature\":0.5"));
        assert!(json.contains("\"num_predict\":50"));
        assert!(!json.contains("\"top_p\""));
        assert!(!json.contains("\"top_k\""));
        assert!(!json.contains("\"num_ctx\""));
    }

    #[test]
    fn test_generate_response_deserialization() {
        let json = r#"{
            "model": "llama3.2:3b",
            "response": "Hello! How can I help you?",
            "done": true,
            "total_duration": 1234567890,
            "eval_count": 15
        }"#;

        let response: GenerateResponse = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(response.model, "llama3.2:3b");
        assert_eq!(response.response, "Hello! How can I help you?");
        assert!(response.done);
        assert_eq!(response.total_duration, 1234567890);
        assert_eq!(response.eval_count, 15);
    }

    #[test]
    fn test_generate_response_with_defaults() {
        // Test that defaults work when fields are missing
        let json = r#"{
            "model": "test",
            "response": "test response",
            "done": true
        }"#;

        let response: GenerateResponse = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(response.total_duration, 0); // default
        assert_eq!(response.eval_count, 0); // default
    }

    #[test]
    fn test_embed_request_serialization() {
        let request = EmbedRequest {
            model: "nomic-embed-text".to_string(),
            prompt: "Test text to embed".to_string(),
        };

        let json = serde_json::to_string(&request).expect("Failed to serialize");
        assert!(json.contains("\"model\":\"nomic-embed-text\""));
        assert!(json.contains("\"prompt\":\"Test text to embed\""));
    }

    #[test]
    fn test_embed_response_deserialization() {
        let json = r#"{
            "embedding": [0.1, 0.2, 0.3, 0.4, 0.5]
        }"#;

        let response: EmbedResponse = serde_json::from_str(json).expect("Failed to deserialize");

        assert_eq!(response.embedding.len(), 5);
        assert!((response.embedding[0] - 0.1).abs() < 0.001);
        assert!((response.embedding[4] - 0.5).abs() < 0.001);
    }

    // Integration tests - these require a running Ollama instance
    // Skip with: cargo test -- --ignored

    #[tokio::test]
    #[ignore = "requires running Ollama instance"]
    async fn test_ollama_health_check() {
        let config = AIConfig::default();
        let client = OllamaClient::new(&config);

        let result = client.health_check().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires running Ollama instance"]
    async fn test_ollama_list_models() {
        let config = AIConfig::default();
        let client = OllamaClient::new(&config);

        let result = client.list_models().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires running Ollama instance and model"]
    async fn test_ollama_complete() {
        let config = AIConfig::default();
        let client = OllamaClient::new(&config);

        let result = client.complete("Say hello in one word.").await;
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }
}
