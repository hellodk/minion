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
