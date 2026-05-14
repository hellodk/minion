//! LLM router for presentation tasks.
//!
//! Routes presentation tasks to appropriate LLM providers (Ollama or OpenAI) and models
//! based on task requirements (text vs. vision capabilities).

use minion_llm::{create_provider, EndpointConfig, LlmProvider, ProviderType};

/// Presentation task types that may require different model capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingTask {
    /// Extract research data from documents
    ResearchExtraction,
    /// Generate narrative content for slides
    NarrativeGeneration,
    /// Plan slide content structure
    SlideContentPlanning,
    /// Generate SVG diagrams (vision task)
    SvgGeneration,
    /// Create chart/diagram DSL
    ChartDiagramDsl,
    /// OCR and describe images (vision task)
    OcrImageDescription,
    /// Critique design elements
    DesignCritique,
}

impl RoutingTask {
    /// Returns true if this task requires vision capabilities.
    pub fn needs_vision(self) -> bool {
        matches!(self, Self::SvgGeneration | Self::OcrImageDescription)
    }
}

/// LLM provider selection for routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouterProvider {
    /// Use Ollama as the backend provider
    Ollama,
    /// Use OpenAI as the backend provider
    OpenAI,
}

/// Configuration for the presentation router.
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Which provider to use (Ollama or OpenAI)
    pub provider: RouterProvider,
    /// Base URL for Ollama endpoint (ignored if provider is OpenAI)
    pub ollama_base_url: String,
    /// API key for OpenAI (required if provider is OpenAI)
    pub openai_api_key: Option<String>,
    /// Text model for Ollama
    pub ollama_text_model: String,
    /// Vision model for Ollama
    pub ollama_vision_model: String,
    /// Text model for OpenAI
    pub openai_text_model: String,
    /// Vision model for OpenAI
    pub openai_vision_model: String,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            provider: RouterProvider::Ollama,
            ollama_base_url: "http://localhost:11434".to_string(),
            openai_api_key: None,
            ollama_text_model: "llama3.2:latest".to_string(),
            ollama_vision_model: "llava:latest".to_string(),
            openai_text_model: "gpt-4o-mini".to_string(),
            openai_vision_model: "gpt-4o".to_string(),
        }
    }
}

/// Routes presentation tasks to appropriate LLM models and providers.
pub struct PresentationRouter {
    config: RouterConfig,
}

impl PresentationRouter {
    /// Creates a new router with the given configuration.
    pub fn new(config: RouterConfig) -> Self {
        Self { config }
    }

    /// Returns the model name for the given task.
    pub fn model_for(&self, task: RoutingTask) -> String {
        match &self.config.provider {
            RouterProvider::Ollama => {
                if task.needs_vision() {
                    self.config.ollama_vision_model.clone()
                } else {
                    self.config.ollama_text_model.clone()
                }
            }
            RouterProvider::OpenAI => {
                if task.needs_vision() {
                    self.config.openai_vision_model.clone()
                } else {
                    self.config.openai_text_model.clone()
                }
            }
        }
    }

    /// Returns a configured LLM provider for the given task.
    pub fn provider_for(&self, task: RoutingTask) -> Box<dyn LlmProvider> {
        let model = self.model_for(task);
        let endpoint = match &self.config.provider {
            RouterProvider::Ollama => EndpointConfig::new(
                ProviderType::Ollama,
                &self.config.ollama_base_url,
                model,
            ),
            RouterProvider::OpenAI => {
                let mut cfg = EndpointConfig::new(
                    ProviderType::Openai,
                    "https://api.openai.com/v1",
                    model,
                );
                if let Some(key) = &self.config.openai_api_key {
                    cfg = cfg.with_api_key(key.clone());
                }
                cfg
            }
        };
        create_provider(endpoint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_config_default_uses_ollama() {
        let config = RouterConfig::default();
        assert_eq!(config.provider, RouterProvider::Ollama);
        assert_eq!(config.ollama_text_model, "llama3.2:latest");
        assert_eq!(config.ollama_vision_model, "llava:latest");
    }

    #[test]
    fn routing_task_needs_vision() {
        assert!(!RoutingTask::ResearchExtraction.needs_vision());
        assert!(!RoutingTask::NarrativeGeneration.needs_vision());
        assert!(!RoutingTask::SlideContentPlanning.needs_vision());
        assert!(RoutingTask::SvgGeneration.needs_vision());
        assert!(!RoutingTask::ChartDiagramDsl.needs_vision());
        assert!(RoutingTask::OcrImageDescription.needs_vision());
        assert!(!RoutingTask::DesignCritique.needs_vision());
    }
}
