use minion_presentation::router::{PresentationRouter, RouterConfig, RouterProvider, RoutingTask};

#[test]
fn ollama_text_task_uses_text_model() {
    let config = RouterConfig {
        provider: RouterProvider::Ollama,
        ollama_base_url: "http://localhost:11434".into(),
        openai_api_key: None,
        ollama_text_model: "llama3.2:latest".into(),
        ollama_vision_model: "llava:latest".into(),
        openai_text_model: "gpt-4o-mini".into(),
        openai_vision_model: "gpt-4o".into(),
    };
    let router = PresentationRouter::new(config);
    assert_eq!(router.model_for(RoutingTask::ResearchExtraction), "llama3.2:latest");
}

#[test]
fn ollama_vision_task_uses_vision_model() {
    let config = RouterConfig {
        provider: RouterProvider::Ollama,
        ollama_base_url: "http://localhost:11434".into(),
        openai_api_key: None,
        ollama_text_model: "llama3.2:latest".into(),
        ollama_vision_model: "llava:latest".into(),
        openai_text_model: "gpt-4o-mini".into(),
        openai_vision_model: "gpt-4o".into(),
    };
    let router = PresentationRouter::new(config);
    assert_eq!(router.model_for(RoutingTask::SvgGeneration), "llava:latest");
    assert_eq!(router.model_for(RoutingTask::OcrImageDescription), "llava:latest");
}

#[test]
fn openai_text_task_uses_text_model() {
    let config = RouterConfig {
        provider: RouterProvider::OpenAI,
        ollama_base_url: "http://localhost:11434".into(),
        openai_api_key: Some("sk-test".into()),
        ollama_text_model: "llama3.2:latest".into(),
        ollama_vision_model: "llava:latest".into(),
        openai_text_model: "gpt-4o-mini".into(),
        openai_vision_model: "gpt-4o".into(),
    };
    let router = PresentationRouter::new(config);
    assert_eq!(router.model_for(RoutingTask::NarrativeGeneration), "gpt-4o-mini");
    assert_eq!(router.model_for(RoutingTask::SlideContentPlanning), "gpt-4o-mini");
    assert_eq!(router.model_for(RoutingTask::ChartDiagramDsl), "gpt-4o-mini");
    assert_eq!(router.model_for(RoutingTask::DesignCritique), "gpt-4o-mini");
}

#[test]
fn openai_vision_task_uses_vision_model() {
    let config = RouterConfig {
        provider: RouterProvider::OpenAI,
        ollama_base_url: "http://localhost:11434".into(),
        openai_api_key: Some("sk-test".into()),
        ollama_text_model: "llama3.2:latest".into(),
        ollama_vision_model: "llava:latest".into(),
        openai_text_model: "gpt-4o-mini".into(),
        openai_vision_model: "gpt-4o".into(),
    };
    let router = PresentationRouter::new(config);
    assert_eq!(router.model_for(RoutingTask::SvgGeneration), "gpt-4o");
    assert_eq!(router.model_for(RoutingTask::OcrImageDescription), "gpt-4o");
}

#[test]
fn provider_for_ollama_returns_ollama_provider() {
    let config = RouterConfig {
        provider: RouterProvider::Ollama,
        ollama_base_url: "http://localhost:11434".into(),
        openai_api_key: None,
        ollama_text_model: "llama3.2:latest".into(),
        ollama_vision_model: "llava:latest".into(),
        openai_text_model: "gpt-4o-mini".into(),
        openai_vision_model: "gpt-4o".into(),
    };
    let router = PresentationRouter::new(config);
    let provider = router.provider_for(RoutingTask::ResearchExtraction);
    assert!(
        provider.name().contains("ollama"),
        "expected ollama provider, got: {}",
        provider.name()
    );
}

#[test]
fn provider_for_openai_returns_openai_compatible_provider() {
    let config = RouterConfig {
        provider: RouterProvider::OpenAI,
        ollama_base_url: "http://localhost:11434".into(),
        openai_api_key: Some("sk-test".into()),
        ollama_text_model: "llama3.2:latest".into(),
        ollama_vision_model: "llava:latest".into(),
        openai_text_model: "gpt-4o-mini".into(),
        openai_vision_model: "gpt-4o".into(),
    };
    let router = PresentationRouter::new(config);
    let provider = router.provider_for(RoutingTask::SlideContentPlanning);
    assert!(
        provider.name().contains("openai"),
        "expected openai provider, got: {}",
        provider.name()
    );
}

#[test]
fn router_config_default_uses_ollama() {
    let config = RouterConfig::default();
    assert_eq!(config.provider, RouterProvider::Ollama);
    assert_eq!(config.ollama_text_model, "llama3.2:latest");
    assert_eq!(config.ollama_vision_model, "llava:latest");
}

// ── ContextManager tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod context_tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use minion_presentation::context::ContextManager;
    use minion_rag::{EmbeddingProvider, RagError, RagPipeline, VectorStore};
    use tempfile::tempdir;

    /// Minimal fake embedder — returns a fixed-dimension zero vector so
    /// we never need a running Ollama instance.
    struct FakeEmbedder {
        dim: usize,
    }

    #[async_trait]
    impl EmbeddingProvider for FakeEmbedder {
        fn name(&self) -> &str {
            "fake"
        }
        fn dimension(&self) -> usize {
            self.dim
        }
        async fn embed(&self, _text: &str) -> Result<Vec<f32>, RagError> {
            Ok(vec![0.0; self.dim])
        }
    }

    fn make_pipeline() -> Arc<RagPipeline> {
        let dir = tempdir().expect("tempdir");
        let store = VectorStore::open(&dir.path().join("ctx_test.db")).expect("VectorStore::open");
        // Leak the tempdir so the DB file lives as long as the pipeline.
        std::mem::forget(dir);
        let embedder = Arc::new(FakeEmbedder { dim: 8 });
        Arc::new(RagPipeline::new(store, embedder))
    }

    #[test]
    fn estimate_tokens_empty() {
        assert_eq!(ContextManager::estimate_tokens(""), 0);
    }

    #[test]
    fn estimate_tokens_four_chars_is_one_token() {
        assert_eq!(ContextManager::estimate_tokens("abcd"), 1);
    }

    #[test]
    fn estimate_tokens_rounds_down() {
        assert_eq!(ContextManager::estimate_tokens("abc"), 0);
        assert_eq!(ContextManager::estimate_tokens("abcdefg"), 1);
    }

    #[tokio::test]
    async fn compress_returns_full_text_when_under_budget() {
        let pipeline = make_pipeline();
        let mut mgr = ContextManager::new(pipeline);
        let text = "Hello world";
        let result = mgr.compress_to_budget("hello", text, 100).await.expect("compress");
        assert_eq!(result, text);
    }

    #[test]
    fn record_usage_reduces_remaining() {
        let pipeline = make_pipeline();
        let mut mgr = ContextManager::new(pipeline);
        let initial = mgr.remaining();
        mgr.record_usage(500);
        assert_eq!(mgr.remaining(), initial - 500);
    }

    #[test]
    fn record_usage_saturates_at_zero() {
        let pipeline = make_pipeline();
        let mut mgr = ContextManager::new(pipeline);
        mgr.record_usage(100_000);
        assert_eq!(mgr.remaining(), 0);
    }
}
