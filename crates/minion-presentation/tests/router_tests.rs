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
