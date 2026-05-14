use minion_presentation::agents::{AgentEvent, next_seq};
use std::sync::atomic::AtomicU32;

#[test]
fn agent_event_started_serializes_with_kind_tag() {
    let ev = AgentEvent::Started { seq: 1, agent: "research".to_string() };
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["kind"], "started");
    assert_eq!(json["seq"], 1);
    assert_eq!(json["agent"], "research");
}

#[test]
fn agent_event_progress_serializes_with_kind_tag() {
    let ev = AgentEvent::Progress { seq: 2, agent: "storyteller".to_string(), data: "processing section 1".to_string() };
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["kind"], "progress");
    assert_eq!(json["data"], "processing section 1");
}

#[test]
fn agent_event_completed_serializes_with_kind_tag() {
    let ev = AgentEvent::Completed { seq: 3, agent: "research".to_string() };
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["kind"], "completed");
}

#[test]
fn agent_event_error_serializes_with_kind_tag() {
    let ev = AgentEvent::Error { seq: 4, agent: "slide_planner".to_string(), message: "LLM timeout".to_string(), recoverable: true };
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["kind"], "error");
    assert_eq!(json["recoverable"], true);
    assert_eq!(json["message"], "LLM timeout");
}

#[test]
fn agent_event_stream_complete_serializes() {
    let ev = AgentEvent::StreamComplete { seq: 5, deck_id: "deck-abc-123".to_string() };
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["kind"], "stream_complete");
    assert_eq!(json["deck_id"], "deck-abc-123");
    assert!(json.get("agent").is_none());
}

#[test]
fn agent_event_stream_error_serializes() {
    let ev = AgentEvent::StreamError { seq: 6, message: "connection refused".to_string() };
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["kind"], "stream_error");
    assert!(json.get("agent").is_none());
}

#[test]
fn agent_event_slide_ready_contains_patch() {
    use minion_presentation::schema::types::{DeckPatch, SectionId, Slide, LayoutKind};
    let section_id = SectionId::new();
    let slide = Slide::new(section_id.clone(), 0.0, 0.0, LayoutKind::Title);
    let patch = DeckPatch::UpsertSlide { section_id, slide };
    let ev = AgentEvent::SlideReady { seq: 7, agent: "slide_planner".to_string(), slide_index: 0, patch };
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["kind"], "slide_ready");
    assert_eq!(json["slide_index"], 0);
    assert_eq!(json["patch"]["op"], "upsert_slide");
}

#[test]
fn next_seq_increments_monotonically() {
    let counter = AtomicU32::new(0);
    assert_eq!(next_seq(&counter), 0);
    assert_eq!(next_seq(&counter), 1);
    assert_eq!(next_seq(&counter), 2);
}

#[test]
fn next_seq_starts_from_current_value() {
    let counter = AtomicU32::new(10);
    assert_eq!(next_seq(&counter), 10);
    assert_eq!(next_seq(&counter), 11);
}

mod research_tests {
    use std::sync::{Arc, atomic::AtomicU32};

    use async_trait::async_trait;
    use minion_llm::{ChatRequest, ChatResponse, LlmProvider, LlmResult, ModelInfo};
    use minion_presentation::{
        agents::{AgentEvent, EventTx},
        agents::research::ResearchAgent,
        schema::types::{GenerationConfig, PresentationContext},
    };

    struct MockLlm(String);

    #[async_trait]
    impl LlmProvider for MockLlm {
        fn name(&self) -> &str {
            "mock"
        }

        async fn chat(&self, _: ChatRequest) -> LlmResult<ChatResponse> {
            Ok(ChatResponse { content: self.0.clone(), model: "mock".into(), usage: None })
        }

        async fn health_check(&self) -> LlmResult<bool> {
            Ok(true)
        }

        async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
            Ok(vec![])
        }
    }

    fn config() -> GenerationConfig {
        GenerationConfig {
            theme_name: None,
            audience: "engineers".into(),
            tone: "technical".into(),
            language: "en-US".into(),
            target_duration_mins: Some(20),
            slide_count_hint: None,
            presentation_context: PresentationContext::LiveTalk,
        }
    }

    #[tokio::test]
    async fn research_agent_parses_output_correctly() {
        let json = r#"{"audience":"senior engineers","tone":"authoritative, concise","language":"en-US","key_themes":["scalability","migration","reliability"],"facts":[{"claim":"99.9% uptime achieved","source":"doc page 3","confidence":0.95},{"claim":"50ms p99 latency","source":"bench results","confidence":0.88}],"suggested_section_count":4,"target_duration_mins":15}"#;
        let (tx, _rx) = tokio::sync::broadcast::channel::<AgentEvent>(64);
        let counter = AtomicU32::new(0);
        let agent = ResearchAgent::new_with_provider(Arc::new(MockLlm(json.into())));
        let result = agent.run("corpus".into(), &config(), &tx, &counter).await.expect("run");
        assert_eq!(result.audience, "senior engineers");
        assert_eq!(result.key_themes.len(), 3);
        assert_eq!(result.facts.len(), 2);
        assert_eq!(result.suggested_section_count, 4);
        assert_eq!(result.target_duration_mins, Some(15));
    }

    #[tokio::test]
    async fn research_agent_emits_started_and_completed() {
        let json = r#"{"audience":"all","tone":"casual","language":"en-US","key_themes":["speed"],"facts":[],"suggested_section_count":3,"target_duration_mins":null}"#;
        let (tx, mut rx) = tokio::sync::broadcast::channel::<AgentEvent>(64);
        let counter = AtomicU32::new(0);
        let agent = ResearchAgent::new_with_provider(Arc::new(MockLlm(json.into())));
        agent.run("corpus".into(), &config(), &tx, &counter).await.expect("run");
        let mut events = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            events.push(ev);
        }
        let kinds: Vec<&str> = events
            .iter()
            .map(|e| match e {
                AgentEvent::Started { .. } => "started",
                AgentEvent::Completed { .. } => "completed",
                _ => "other",
            })
            .collect();
        assert!(kinds.contains(&"started"));
        assert!(kinds.contains(&"completed"));
        assert_eq!(kinds[0], "started");
    }

    #[tokio::test]
    async fn research_agent_emits_progress_per_fact() {
        let json = r#"{"audience":"all","tone":"casual","language":"en-US","key_themes":[],"facts":[{"claim":"fact A","source":"src1","confidence":0.8},{"claim":"fact B","source":"src2","confidence":0.7},{"claim":"fact C","source":"src3","confidence":0.9}],"suggested_section_count":2,"target_duration_mins":null}"#;
        let (tx, mut rx) = tokio::sync::broadcast::channel::<AgentEvent>(64);
        let counter = AtomicU32::new(0);
        let agent = ResearchAgent::new_with_provider(Arc::new(MockLlm(json.into())));
        agent.run("corpus".into(), &config(), &tx, &counter).await.expect("run");
        let mut progress = 0;
        while let Ok(ev) = rx.try_recv() {
            if matches!(ev, AgentEvent::Progress { .. }) {
                progress += 1;
            }
        }
        assert_eq!(progress, 3);
    }
}
