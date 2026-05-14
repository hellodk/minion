use minion_llm::streaming::{parse_sse_line, StreamEvent};

#[test]
fn parses_data_line() {
    let line = r#"data: {"id":"x","choices":[{"delta":{"content":"Hello"}}]}"#;
    let event = parse_sse_line(line);
    assert!(matches!(event, Some(StreamEvent::Token(t)) if t == "Hello"));
}

#[test]
fn parses_done_sentinel() {
    let line = "data: [DONE]";
    let event = parse_sse_line(line);
    assert!(matches!(event, Some(StreamEvent::Done)));
}

#[test]
fn ignores_comment_lines() {
    let line = ": keep-alive";
    let event = parse_sse_line(line);
    assert!(event.is_none());
}
