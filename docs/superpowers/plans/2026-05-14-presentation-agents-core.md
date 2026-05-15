# Presentation Module — Sub-Plan 2c: Agent Infrastructure + Core Agents

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the agent event infrastructure and implement the first three pipeline agents — Research, Storyteller, and SlidePlanner — that together transform raw input into a fully structured Deck skeleton.

**Architecture:** Agents communicate via a tokio broadcast channel carrying `AgentEvent` structs. Each agent takes a typed input, calls `extract_json` on the LLM router's provider, and streams progress events. `SlidePlannerAgent` produces the Deck skeleton with visual placeholders that VisualAgent (sub-plan 2d) will fill.

**Tech Stack:** Rust, tokio (broadcast, JoinSet, Mutex), minion-llm (`extract_json`), serde_json, minion-presentation schema types.

---

## Prerequisites

Sub-plans 2a (Foundation) and 2b (Router + ContextManager) must be complete. Confirm before starting:

```bash
grep "pub mod" /home/dk/Documents/git/minion/crates/minion-presentation/src/lib.rs
# Expected: pub mod context; pub mod db; pub mod migrations; pub mod router; pub mod schema;
```

Also confirm the router and context modules exist:

```bash
ls /home/dk/Documents/git/minion/crates/minion-presentation/src/router.rs
ls /home/dk/Documents/git/minion/crates/minion-presentation/src/context.rs
```

---

## Task 1: AgentEvent + channel infrastructure (`agents/mod.rs`)

**Files to create:**
- `crates/minion-presentation/src/agents/mod.rs`

**Files to modify:**
- `crates/minion-presentation/src/lib.rs` — add `pub mod agents;`

### Step 1.1 — Write the tests first (TDD)

- [ ] Create `crates/minion-presentation/tests/agent_tests.rs` with this initial section:

```rust
// crates/minion-presentation/tests/agent_tests.rs

use minion_presentation::agents::{AgentEvent, next_seq};
use std::sync::atomic::AtomicU32;

// ── AgentEvent serialization ──────────────────────────────────────────────────

#[test]
fn agent_event_started_serializes_with_kind_tag() {
    let ev = AgentEvent::Started {
        seq: 1,
        agent: "research".to_string(),
    };
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["kind"], "started");
    assert_eq!(json["seq"], 1);
    assert_eq!(json["agent"], "research");
}

#[test]
fn agent_event_progress_serializes_with_kind_tag() {
    let ev = AgentEvent::Progress {
        seq: 2,
        agent: "storyteller".to_string(),
        data: "processing section 1".to_string(),
    };
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["kind"], "progress");
    assert_eq!(json["data"], "processing section 1");
}

#[test]
fn agent_event_completed_serializes_with_kind_tag() {
    let ev = AgentEvent::Completed {
        seq: 3,
        agent: "research".to_string(),
    };
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["kind"], "completed");
}

#[test]
fn agent_event_error_serializes_with_kind_tag() {
    let ev = AgentEvent::Error {
        seq: 4,
        agent: "slide_planner".to_string(),
        message: "LLM timeout".to_string(),
        recoverable: true,
    };
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["kind"], "error");
    assert_eq!(json["recoverable"], true);
    assert_eq!(json["message"], "LLM timeout");
}

#[test]
fn agent_event_stream_complete_serializes() {
    let ev = AgentEvent::StreamComplete {
        seq: 5,
        deck_id: "deck-abc-123".to_string(),
    };
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["kind"], "stream_complete");
    assert_eq!(json["deck_id"], "deck-abc-123");
    // stream_complete has no "agent" field
    assert!(json.get("agent").is_none());
}

#[test]
fn agent_event_stream_error_serializes() {
    let ev = AgentEvent::StreamError {
        seq: 6,
        message: "connection refused".to_string(),
    };
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["kind"], "stream_error");
    assert!(json.get("agent").is_none());
}

#[test]
fn agent_event_slide_ready_contains_patch() {
    use minion_presentation::schema::types::{DeckPatch, SectionId, SlideId, Slide, LayoutKind};
    let section_id = SectionId::new();
    let slide = Slide::new(section_id.clone(), 0.0, 0.0, LayoutKind::Title);
    let patch = DeckPatch::UpsertSlide { section_id, slide };
    let ev = AgentEvent::SlideReady {
        seq: 7,
        agent: "slide_planner".to_string(),
        slide_index: 0,
        patch,
    };
    let json = serde_json::to_value(&ev).unwrap();
    assert_eq!(json["kind"], "slide_ready");
    assert_eq!(json["slide_index"], 0);
    // patch is present and has the right op
    assert_eq!(json["patch"]["op"], "upsert_slide");
}

// ── next_seq ──────────────────────────────────────────────────────────────────

#[test]
fn next_seq_increments_monotonically() {
    let counter = AtomicU32::new(0);
    let a = next_seq(&counter);
    let b = next_seq(&counter);
    let c = next_seq(&counter);
    assert_eq!(a, 0);
    assert_eq!(b, 1);
    assert_eq!(c, 2);
}

#[test]
fn next_seq_starts_from_current_value() {
    let counter = AtomicU32::new(10);
    assert_eq!(next_seq(&counter), 10);
    assert_eq!(next_seq(&counter), 11);
}
```

- [ ] Run `cargo test -p minion-presentation --test agent_tests 2>&1 | head -30` — expect compile errors (module not yet defined). Confirms the test file is wired.

### Step 1.2 — Create the agents directory and `mod.rs`

- [ ] Create `crates/minion-presentation/src/agents/mod.rs`:

```rust
//! Agent event infrastructure for the presentation generation pipeline.
//!
//! Agents communicate over a tokio broadcast channel. Each pipeline step emits
//! [`AgentEvent`] values that the Tauri command layer forwards to the frontend
//! via `presentation://agent-event/{session_id}`.

pub mod research;
pub mod slide_planner;
pub mod storyteller;

use crate::schema::types::DeckPatch;
use std::sync::atomic::{AtomicU32, Ordering};

// ── AgentEvent ────────────────────────────────────────────────────────────────

/// Events emitted by pipeline agents over the broadcast channel.
///
/// The `kind` tag and `snake_case` rename must stay in sync with the TypeScript
/// union type in `ui/src/lib/presentation-api.ts`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentEvent {
    /// Agent has started work.
    Started { seq: u32, agent: String },
    /// Incremental progress update (free-form `data` string for the UI).
    Progress { seq: u32, agent: String, data: String },
    /// A single slide is ready and its patch can be applied to the live deck.
    SlideReady { seq: u32, agent: String, slide_index: u32, patch: DeckPatch },
    /// Agent completed successfully.
    Completed { seq: u32, agent: String },
    /// Agent encountered an error. `recoverable` = true means the pipeline may continue.
    Error { seq: u32, agent: String, message: String, recoverable: bool },
    /// Entire pipeline finished; `deck_id` identifies the persisted deck.
    StreamComplete { seq: u32, deck_id: String },
    /// Unrecoverable pipeline error.
    StreamError { seq: u32, message: String },
}

// ── Channel type alias ────────────────────────────────────────────────────────

/// Broadcast sender for agent events. Use `.subscribe()` to get a receiver.
pub type EventTx = tokio::sync::broadcast::Sender<AgentEvent>;

// ── Sequence counter helper ───────────────────────────────────────────────────

/// Atomically increment `counter` and return the **previous** value as the
/// next sequence number. Thread-safe across the agent pipeline.
pub fn next_seq(counter: &AtomicU32) -> u32 {
    counter.fetch_add(1, Ordering::Relaxed)
}

// ── Agent name constants ──────────────────────────────────────────────────────

/// Canonical agent name strings used in [`AgentEvent`] fields. These must
/// match the `AgentName` union in `presentation-api.ts`.
pub mod agent_name {
    pub const RESEARCH: &str = "research";
    pub const STORYTELLER: &str = "storyteller";
    pub const SLIDE_PLANNER: &str = "slide_planner";
    pub const VISUAL: &str = "visual";
    pub const DESIGN_CRITIC: &str = "design_critic";
}
```

### Step 1.3 — Wire lib.rs

- [ ] Edit `crates/minion-presentation/src/lib.rs` to add `pub mod agents;`. The file should become:

```rust
pub mod agents;
pub mod context;
pub mod db;
pub mod migrations;
pub mod router;
pub mod schema;

pub use schema::types::*;
```

### Step 1.4 — Run the tests

- [ ] Run: `cargo test -p minion-presentation --test agent_tests 2>&1`

  All 10 tests in the infrastructure section must pass. Fix any issues.

- [ ] Also run the full test suite to confirm no regressions:

```bash
cargo test -p minion-presentation 2>&1
```

---

## Task 2: ResearchAgent (`agents/research.rs`)

**File to create:** `crates/minion-presentation/src/agents/research.rs`

### Step 2.1 — Write the tests first (TDD)

- [ ] Append to `crates/minion-presentation/tests/agent_tests.rs`:

```rust
// ── ResearchAgent tests ───────────────────────────────────────────────────────

mod research_tests {
    use minion_presentation::agents::research::{ResearchAgent, ResearchOutput};
    use minion_presentation::agents::{AgentEvent, EventTx, next_seq};
    use minion_presentation::router::{PresentationRouter, RouterConfig, RouterProvider};
    use minion_presentation::context::ContextManager;
    use minion_presentation::schema::types::GenerationConfig;
    use minion_llm::{
        LlmProvider, JsonExtractRequest, JsonExtractResponse, LlmResult,
        ChatRequest, ChatResponse, ModelInfo,
    };
    use std::sync::{Arc, Mutex, atomic::AtomicU32};
    use async_trait::async_trait;

    // ── MockLlmProvider ───────────────────────────────────────────────────────

    /// A mock provider that returns a pre-baked JSON response for extract_json.
    struct MockLlmProvider {
        json_response: String,
    }

    #[async_trait]
    impl LlmProvider for MockLlmProvider {
        fn name(&self) -> &str { "mock" }

        async fn chat(&self, _req: ChatRequest) -> LlmResult<ChatResponse> {
            Ok(ChatResponse {
                content: self.json_response.clone(),
                model: "mock-model".to_string(),
                usage: None,
            })
        }

        async fn health_check(&self) -> LlmResult<bool> { Ok(true) }
        async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> { Ok(vec![]) }
    }

    fn make_config() -> GenerationConfig {
        GenerationConfig {
            theme_name: None,
            audience: "engineers".to_string(),
            tone: "technical".to_string(),
            language: "en-US".to_string(),
            target_duration_mins: Some(20),
            slide_count_hint: None,
            presentation_context: minion_presentation::schema::types::PresentationContext::LiveTalk,
        }
    }

    fn make_router() -> Arc<PresentationRouter> {
        Arc::new(PresentationRouter::new(RouterConfig::default()))
    }

    fn make_context() -> Arc<Mutex<ContextManager>> {
        // ContextManager::new requires a RagPipeline — for agent tests we use
        // a thin wrapper that bypasses RAG. See note below.
        //
        // NOTE: ResearchAgent receives Arc<Mutex<ContextManager>> but only
        // calls record_usage (not compress_to_budget), so the RAG pipeline is
        // never exercised in these unit tests. We construct via
        // ContextManager::new_for_testing() — a constructor that accepts no
        // RAG pipeline and panics if compress_to_budget is called.
        Arc::new(Mutex::new(ContextManager::new_for_testing()))
    }

    #[tokio::test]
    async fn research_agent_parses_output_correctly() {
        let mock_json = r#"{
            "audience": "senior engineers",
            "tone": "authoritative, concise",
            "language": "en-US",
            "key_themes": ["scalability", "migration", "reliability"],
            "facts": [
                {"claim": "99.9% uptime achieved", "source": "doc page 3", "confidence": 0.95},
                {"claim": "50ms p99 latency", "source": "bench results", "confidence": 0.88}
            ],
            "suggested_section_count": 4,
            "target_duration_mins": 15
        }"#;

        let provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider {
            json_response: mock_json.to_string(),
        });

        let (tx, _rx) = tokio::sync::broadcast::channel::<AgentEvent>(64);
        let counter = AtomicU32::new(0);

        let agent = ResearchAgent::new_with_provider(provider);
        let config = make_config();
        let result = agent
            .run("some corpus text".to_string(), &config, &tx, &counter)
            .await
            .expect("research should succeed");

        assert_eq!(result.audience, "senior engineers");
        assert_eq!(result.tone, "authoritative, concise");
        assert_eq!(result.language, "en-US");
        assert_eq!(result.key_themes.len(), 3);
        assert!(result.key_themes.contains(&"scalability".to_string()));
        assert_eq!(result.facts.len(), 2);
        assert_eq!(result.facts[0].claim, "99.9% uptime achieved");
        assert!((result.facts[0].confidence - 0.95).abs() < 1e-4);
        assert_eq!(result.suggested_section_count, 4);
        assert_eq!(result.target_duration_mins, Some(15));
    }

    #[tokio::test]
    async fn research_agent_emits_started_and_completed_events() {
        let mock_json = r#"{
            "audience": "all",
            "tone": "casual",
            "language": "en-US",
            "key_themes": ["speed"],
            "facts": [],
            "suggested_section_count": 3,
            "target_duration_mins": null
        }"#;

        let provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider {
            json_response: mock_json.to_string(),
        });

        let (tx, mut rx) = tokio::sync::broadcast::channel::<AgentEvent>(64);
        let counter = AtomicU32::new(0);

        let agent = ResearchAgent::new_with_provider(provider);
        let config = make_config();
        agent
            .run("corpus".to_string(), &config, &tx, &counter)
            .await
            .expect("run");

        // Drain all events
        let mut events = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            events.push(ev);
        }

        let kinds: Vec<&str> = events
            .iter()
            .map(|e| match e {
                AgentEvent::Started { .. } => "started",
                AgentEvent::Progress { .. } => "progress",
                AgentEvent::Completed { .. } => "completed",
                _ => "other",
            })
            .collect();

        assert!(kinds.contains(&"started"), "must emit Started");
        assert!(kinds.contains(&"completed"), "must emit Completed");
        // First event must be Started
        assert_eq!(kinds[0], "started");
        // Last event must be Completed
        assert_eq!(kinds[kinds.len() - 1], "completed");
    }

    #[tokio::test]
    async fn research_agent_emits_progress_per_fact() {
        let mock_json = r#"{
            "audience": "all",
            "tone": "casual",
            "language": "en-US",
            "key_themes": [],
            "facts": [
                {"claim": "fact A", "source": "src1", "confidence": 0.8},
                {"claim": "fact B", "source": "src2", "confidence": 0.7},
                {"claim": "fact C", "source": "src3", "confidence": 0.9}
            ],
            "suggested_section_count": 2,
            "target_duration_mins": null
        }"#;

        let provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider {
            json_response: mock_json.to_string(),
        });

        let (tx, mut rx) = tokio::sync::broadcast::channel::<AgentEvent>(64);
        let counter = AtomicU32::new(0);

        let agent = ResearchAgent::new_with_provider(provider);
        let config = make_config();
        agent.run("corpus".to_string(), &config, &tx, &counter).await.expect("run");

        let mut progress_count = 0usize;
        while let Ok(ev) = rx.try_recv() {
            if matches!(ev, AgentEvent::Progress { .. }) {
                progress_count += 1;
            }
        }
        // One Progress event per fact
        assert_eq!(progress_count, 3, "expect one Progress per fact");
    }
}
```

- [ ] Run `cargo test -p minion-presentation --test agent_tests research_tests 2>&1 | head -40` — expect compile errors (ResearchAgent not yet defined).

**Note on `ContextManager::new_for_testing()`:** The tests use a `new_for_testing()` constructor. Add this to `context.rs` as part of this task:

```rust
/// Construct a `ContextManager` with no RAG pipeline for unit tests.
/// Panics if `compress_to_budget` is called — use only in tests that
/// only exercise `record_usage` and `remaining`.
#[cfg(test)]
pub fn new_for_testing() -> Self {
    // We use a sentinel: rag field set to a dummy pipeline that panics on search.
    // Simpler approach: since all test agents only call record_usage, we can
    // construct a real ContextManager with a real RagPipeline backed by an
    // in-memory store if minion-rag is available, or we skip the field entirely
    // by making rag: Option<Arc<RagPipeline>>.
    //
    // IMPLEMENTATION NOTE: See Step 2.2 for the full design — ContextManager
    // gains an Option<Arc<RagPipeline>> internal field so that new_for_testing()
    // passes None and compress_to_budget returns Err if called without a pipeline.
    todo!("implemented in Step 2.2")
}
```

### Step 2.2 — Add `ContextManager::new_for_testing` to `context.rs`

- [ ] Edit `crates/minion-presentation/src/context.rs` to make the `rag` field `Option<Arc<RagPipeline>>` and add the test constructor. Change:

```rust
// Before:
pub struct ContextManager {
    rag: Arc<RagPipeline>,
    max_context_tokens: usize,
    used_tokens: usize,
}
```

to:

```rust
// After:
pub struct ContextManager {
    rag: Option<Arc<RagPipeline>>,
    max_context_tokens: usize,
    used_tokens: usize,
}
```

Update `new` to wrap the pipeline in `Some`:

```rust
pub fn new(rag: Arc<RagPipeline>) -> Self {
    Self {
        rag: Some(rag),
        max_context_tokens: Self::DEFAULT_MAX_TOKENS,
        used_tokens: 0,
    }
}
```

Add the test constructor after `new`:

```rust
/// Construct a `ContextManager` without a RAG pipeline. Only valid for unit
/// tests where `compress_to_budget` is never called.
#[cfg(test)]
pub fn new_for_testing() -> Self {
    Self {
        rag: None,
        max_context_tokens: Self::DEFAULT_MAX_TOKENS,
        used_tokens: 0,
    }
}
```

Update `compress_to_budget` to handle `rag: None`:

```rust
pub async fn compress_to_budget(
    &mut self,
    query: &str,
    full_text: &str,
    budget_tokens: usize,
) -> anyhow::Result<String> {
    if Self::estimate_tokens(full_text) <= budget_tokens {
        return Ok(full_text.to_string());
    }

    let rag = self.rag.as_ref().ok_or_else(|| {
        anyhow::anyhow!("ContextManager has no RAG pipeline (new_for_testing was used)")
    })?;

    let hits: Vec<SearchHit> = rag
        .search(query, 10, None)
        .await
        .map_err(anyhow::Error::from)?;

    let joined = hits
        .into_iter()
        .map(|h| h.chunk.text)
        .collect::<Vec<_>>()
        .join("\n\n");

    let char_budget = budget_tokens * 4;
    let truncated: String = joined.chars().take(char_budget).collect();
    Ok(truncated)
}
```

- [ ] Run existing context tests to confirm no regressions: `cargo test -p minion-presentation context_tests 2>&1`

### Step 2.3 — Implement `agents/research.rs`

- [ ] Create `crates/minion-presentation/src/agents/research.rs`:

```rust
//! Research agent — extracts structured facts from raw corpus text.

use std::sync::{
    Arc,
    atomic::AtomicU32,
};

use anyhow::Context as _;
use minion_llm::{JsonExtractRequest, LlmProvider};
use serde::{Deserialize, Serialize};

use crate::{
    agents::{agent_name, next_seq, AgentEvent, EventTx},
    schema::types::GenerationConfig,
};

// ── Output types ──────────────────────────────────────────────────────────────

/// A single concrete fact extracted from the corpus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchFact {
    pub claim: String,
    pub source: String,
    pub confidence: f32,
}

/// Structured output of the research phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchOutput {
    pub audience: String,
    pub tone: String,
    pub language: String,
    pub key_themes: Vec<String>,
    pub facts: Vec<ResearchFact>,
    pub suggested_section_count: u32,
    pub target_duration_mins: Option<u32>,
}

// ── LLM prompts ───────────────────────────────────────────────────────────────

const SYSTEM_PROMPT: &str = "\
You are a research analyst extracting structured information from content to be used in a presentation. \
Analyze the provided content and extract: target audience, tone, language, key themes, concrete facts \
with sources, suggested number of sections, and estimated presentation duration.\n\
Return ONLY valid JSON matching the provided schema.";

const EXAMPLE_JSON: &str = r#"{
  "audience": "engineering leadership",
  "tone": "authoritative, concise",
  "language": "en-US",
  "key_themes": ["scalability", "migration"],
  "facts": [{"claim": "...", "source": "doc page 1", "confidence": 0.9}],
  "suggested_section_count": 5,
  "target_duration_mins": 15
}"#;

// ── Agent ─────────────────────────────────────────────────────────────────────

/// Extracts research findings from a corpus for later pipeline stages.
pub struct ResearchAgent {
    provider: Arc<dyn LlmProvider>,
}

impl ResearchAgent {
    /// Construct with a router-selected provider.
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    /// Construct with an explicit provider (useful for tests with mock providers).
    pub fn new_with_provider(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    /// Run the research extraction.
    ///
    /// Emits: `Started` → one `Progress` per extracted fact → `Completed`.
    pub async fn run(
        &self,
        corpus: String,
        _config: &GenerationConfig,
        event_tx: &EventTx,
        seq: &AtomicU32,
    ) -> anyhow::Result<ResearchOutput> {
        // Emit Started
        let _ = event_tx.send(AgentEvent::Started {
            seq: next_seq(seq),
            agent: agent_name::RESEARCH.to_string(),
        });

        let user_input = format!("Analyze this content for a presentation:\n\n{corpus}");

        let req = JsonExtractRequest {
            system_prompt: SYSTEM_PROMPT.to_string(),
            user_input,
            example_json: EXAMPLE_JSON.to_string(),
            model: None,
            temperature: Some(0.0),
        };

        let resp = self
            .provider
            .extract_json(req)
            .await
            .context("ResearchAgent: extract_json failed")?;

        let output: ResearchOutput = serde_json::from_value(resp.parsed)
            .context("ResearchAgent: failed to deserialize ResearchOutput from LLM JSON")?;

        // Emit one Progress per fact
        for fact in &output.facts {
            let _ = event_tx.send(AgentEvent::Progress {
                seq: next_seq(seq),
                agent: agent_name::RESEARCH.to_string(),
                data: format!("fact: {} (confidence {:.2})", fact.claim, fact.confidence),
            });
        }

        // Emit Completed
        let _ = event_tx.send(AgentEvent::Completed {
            seq: next_seq(seq),
            agent: agent_name::RESEARCH.to_string(),
        });

        Ok(output)
    }
}
```

### Step 2.4 — Run all agent tests so far

- [ ] Run: `cargo test -p minion-presentation --test agent_tests 2>&1`

  The 10 infrastructure tests + 3 research tests must all pass.

- [ ] If `ContextManager::new_for_testing` was added, also re-run context tests:

```bash
cargo test -p minion-presentation 2>&1
```

---

## Task 3: StorytellerAgent (`agents/storyteller.rs`)

**File to create:** `crates/minion-presentation/src/agents/storyteller.rs`

### Step 3.1 — Write the tests first (TDD)

- [ ] Append to `crates/minion-presentation/tests/agent_tests.rs`:

```rust
// ── StorytellerAgent tests ────────────────────────────────────────────────────

mod storyteller_tests {
    use minion_presentation::agents::storyteller::{StorytellerAgent, StorytellerOutput};
    use minion_presentation::agents::research::ResearchOutput;
    use minion_presentation::agents::{AgentEvent, next_seq};
    use minion_llm::{
        LlmProvider, JsonExtractRequest, JsonExtractResponse, LlmResult,
        ChatRequest, ChatResponse, ModelInfo,
    };
    use std::sync::{Arc, atomic::AtomicU32};
    use async_trait::async_trait;

    struct MockLlmProvider {
        json_response: String,
    }

    #[async_trait]
    impl LlmProvider for MockLlmProvider {
        fn name(&self) -> &str { "mock" }
        async fn chat(&self, _req: ChatRequest) -> LlmResult<ChatResponse> {
            Ok(ChatResponse {
                content: self.json_response.clone(),
                model: "mock-model".to_string(),
                usage: None,
            })
        }
        async fn health_check(&self) -> LlmResult<bool> { Ok(true) }
        async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> { Ok(vec![]) }
    }

    fn make_research_output() -> ResearchOutput {
        ResearchOutput {
            audience: "product managers".to_string(),
            tone: "inspiring".to_string(),
            language: "en-US".to_string(),
            key_themes: vec!["growth".to_string(), "innovation".to_string()],
            facts: vec![],
            suggested_section_count: 3,
            target_duration_mins: Some(10),
        }
    }

    #[tokio::test]
    async fn storyteller_agent_parses_output_correctly() {
        let mock_json = r#"{
            "title": "The Road to 10x Growth",
            "hook": "What if one decision could change everything?",
            "sections": [
                {"title": "The Problem", "slide_count": 3, "purpose": "establish pain", "pacing": "slow, deliberate"},
                {"title": "Our Solution", "slide_count": 4, "purpose": "reveal answer", "pacing": "building momentum"},
                {"title": "The Future", "slide_count": 2, "purpose": "inspire action", "pacing": "fast, exciting"}
            ],
            "closing_cta": "Join us — ship by Q3",
            "camera_narrative": "Start wide, zoom in on key moments, pull back for the finale"
        }"#;

        let provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider {
            json_response: mock_json.to_string(),
        });

        let (tx, _rx) = tokio::sync::broadcast::channel::<AgentEvent>(64);
        let counter = AtomicU32::new(0);

        let agent = StorytellerAgent::new_with_provider(provider);
        let research = make_research_output();
        let result = agent
            .run(&research, &tx, &counter)
            .await
            .expect("storyteller should succeed");

        assert_eq!(result.title, "The Road to 10x Growth");
        assert_eq!(result.hook, "What if one decision could change everything?");
        assert_eq!(result.sections.len(), 3);
        assert_eq!(result.sections[0].title, "The Problem");
        assert_eq!(result.sections[0].slide_count, 3);
        assert_eq!(result.sections[1].purpose, "reveal answer");
        assert_eq!(result.closing_cta, "Join us — ship by Q3");
        assert!(!result.camera_narrative.is_empty());
    }

    #[tokio::test]
    async fn storyteller_agent_emits_started_and_completed() {
        let mock_json = r#"{
            "title": "T",
            "hook": "H",
            "sections": [{"title": "S1", "slide_count": 2, "purpose": "p", "pacing": "medium"}],
            "closing_cta": "go",
            "camera_narrative": "zoom"
        }"#;

        let provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider {
            json_response: mock_json.to_string(),
        });

        let (tx, mut rx) = tokio::sync::broadcast::channel::<AgentEvent>(64);
        let counter = AtomicU32::new(0);

        let agent = StorytellerAgent::new_with_provider(provider);
        let research = make_research_output();
        agent.run(&research, &tx, &counter).await.expect("run");

        let mut events = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            events.push(ev);
        }

        assert!(!events.is_empty());
        assert!(matches!(events[0], AgentEvent::Started { .. }));
        assert!(matches!(events[events.len() - 1], AgentEvent::Completed { .. }));
    }

    #[tokio::test]
    async fn storyteller_emits_progress_per_section() {
        let mock_json = r#"{
            "title": "T",
            "hook": "H",
            "sections": [
                {"title": "Intro", "slide_count": 1, "purpose": "p", "pacing": "slow"},
                {"title": "Core", "slide_count": 3, "purpose": "p", "pacing": "medium"},
                {"title": "End", "slide_count": 1, "purpose": "p", "pacing": "fast"}
            ],
            "closing_cta": "done",
            "camera_narrative": "pan"
        }"#;

        let provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider {
            json_response: mock_json.to_string(),
        });

        let (tx, mut rx) = tokio::sync::broadcast::channel::<AgentEvent>(64);
        let counter = AtomicU32::new(0);

        let agent = StorytellerAgent::new_with_provider(provider);
        let research = make_research_output();
        agent.run(&research, &tx, &counter).await.expect("run");

        let mut progress_count = 0usize;
        while let Ok(ev) = rx.try_recv() {
            if matches!(ev, AgentEvent::Progress { .. }) {
                progress_count += 1;
            }
        }
        // One Progress per section
        assert_eq!(progress_count, 3);
    }
}
```

- [ ] Run `cargo test -p minion-presentation --test agent_tests storyteller_tests 2>&1 | head -30` — expect compile errors.

### Step 3.2 — Implement `agents/storyteller.rs`

- [ ] Create `crates/minion-presentation/src/agents/storyteller.rs`:

```rust
//! Storyteller agent — builds a narrative structure from research findings.

use std::sync::{Arc, atomic::AtomicU32};

use anyhow::Context as _;
use minion_llm::{JsonExtractRequest, LlmProvider};
use serde::{Deserialize, Serialize};

use crate::agents::{agent_name, next_seq, AgentEvent, EventTx};
use super::research::ResearchOutput;

// ── Output types ──────────────────────────────────────────────────────────────

/// A single narrative section with pacing guidance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorySection {
    pub title: String,
    pub slide_count: u32,
    pub purpose: String,
    pub pacing: String,
}

/// Structured narrative plan produced by the storyteller phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorytellerOutput {
    pub title: String,
    pub hook: String,
    pub sections: Vec<StorySection>,
    pub closing_cta: String,
    pub camera_narrative: String,
}

// ── LLM prompts ───────────────────────────────────────────────────────────────

const SYSTEM_PROMPT: &str = "\
You are a master storyteller and presentation architect. Given research findings about content, \
create a compelling narrative structure with clear sections, emotional arc, and a memorable hook. \
Each section should have a clear purpose and pacing. Return ONLY valid JSON.";

const EXAMPLE_JSON: &str = r#"{
  "title": "Scaling Without Limits",
  "hook": "What if your biggest constraint disappeared overnight?",
  "sections": [
    {"title": "The Status Quo", "slide_count": 3, "purpose": "establish context", "pacing": "slow, grounding"},
    {"title": "The Breakthrough", "slide_count": 4, "purpose": "reveal solution", "pacing": "building energy"},
    {"title": "What's Next", "slide_count": 2, "purpose": "inspire action", "pacing": "fast, urgent"}
  ],
  "closing_cta": "Let's build this together",
  "camera_narrative": "Open wide on the problem landscape, zoom in on the breakthrough moment, pull back for the vision"
}"#;

// ── Agent ─────────────────────────────────────────────────────────────────────

/// Transforms research output into a narrative structure for the presentation.
pub struct StorytellerAgent {
    provider: Arc<dyn LlmProvider>,
}

impl StorytellerAgent {
    /// Construct with a router-selected provider.
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    /// Construct with an explicit provider (useful for tests with mock providers).
    pub fn new_with_provider(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    /// Run the storytelling phase.
    ///
    /// Emits: `Started` → one `Progress` per section → `Completed`.
    pub async fn run(
        &self,
        research: &ResearchOutput,
        event_tx: &EventTx,
        seq: &AtomicU32,
    ) -> anyhow::Result<StorytellerOutput> {
        // Emit Started
        let _ = event_tx.send(AgentEvent::Started {
            seq: next_seq(seq),
            agent: agent_name::STORYTELLER.to_string(),
        });

        let user_input = format!(
            "Create a narrative structure for a presentation based on this research:\n\n\
             Audience: {}\nTone: {}\nLanguage: {}\nKey Themes: {}\n\
             Suggested Sections: {}\nTarget Duration: {} mins\n\
             Key Facts:\n{}",
            research.audience,
            research.tone,
            research.language,
            research.key_themes.join(", "),
            research.suggested_section_count,
            research.target_duration_mins.map_or("unspecified".to_string(), |d| d.to_string()),
            research
                .facts
                .iter()
                .map(|f| format!("  - {} ({})", f.claim, f.source))
                .collect::<Vec<_>>()
                .join("\n"),
        );

        let req = JsonExtractRequest {
            system_prompt: SYSTEM_PROMPT.to_string(),
            user_input,
            example_json: EXAMPLE_JSON.to_string(),
            model: None,
            temperature: Some(0.3),
        };

        let resp = self
            .provider
            .extract_json(req)
            .await
            .context("StorytellerAgent: extract_json failed")?;

        let output: StorytellerOutput = serde_json::from_value(resp.parsed)
            .context("StorytellerAgent: failed to deserialize StorytellerOutput")?;

        // Emit one Progress per section
        for section in &output.sections {
            let _ = event_tx.send(AgentEvent::Progress {
                seq: next_seq(seq),
                agent: agent_name::STORYTELLER.to_string(),
                data: format!(
                    "section: \"{}\" ({} slides, {})",
                    section.title, section.slide_count, section.pacing
                ),
            });
        }

        // Emit Completed
        let _ = event_tx.send(AgentEvent::Completed {
            seq: next_seq(seq),
            agent: agent_name::STORYTELLER.to_string(),
        });

        Ok(output)
    }
}
```

### Step 3.3 — Run tests

- [ ] Run: `cargo test -p minion-presentation --test agent_tests 2>&1`

  All infrastructure + research + storyteller tests must pass.

---

## Task 4: SlidePlannerAgent (`agents/slide_planner.rs`)

**File to create:** `crates/minion-presentation/src/agents/slide_planner.rs`

This is the most complex agent. It issues one LLM call per narrative section (concurrently, max 3 at a time using `tokio::JoinSet`), assembles the responses into a full `Deck`, and emits a `SlideReady` event for each slide as it's added.

### Canvas layout arithmetic

Slides within a section are placed horizontally:
- **Slide width:** 1920 units (matches `Slide::new` default)
- **Horizontal gap between slides:** 180 units
- **Horizontal stride per slide:** `1920 + 180 = 2100` units
- **Canvas X for slide `i` in a section:** `section_canvas_x + (i as f64 * 2100.0)`

Sections are stacked vertically:
- **Section stride (vertical):** 1280 units
- **Canvas Y for section `j`:** `j as f64 * 1280.0`
- **Section canvas X:** `0.0` for all sections (slides within each section extend rightward)

Example for 2 sections with 3 and 2 slides respectively:
```
Section 0 (y=0):    slide(x=0,y=0)  slide(x=2100,y=0)  slide(x=4200,y=0)
Section 1 (y=1280): slide(x=0,y=1280)  slide(x=2100,y=1280)
```

### Layout selection mapping

The LLM response uses string layout names that map to `LayoutKind` variants. The agent uses this mapping when constructing slides:

| JSON string      | `LayoutKind` variant      |
|------------------|---------------------------|
| `"title"`        | `LayoutKind::Title`        |
| `"kpi"`          | `LayoutKind::Kpi`          |
| `"comparison"`   | `LayoutKind::Comparison`   |
| `"process"`      | `LayoutKind::Process`      |
| `"architecture"` | `LayoutKind::Architecture` |
| `"quote"`        | `LayoutKind::Quote`        |
| `"timeline"`     | `LayoutKind::Timeline`     |
| `"storytelling"` | `LayoutKind::Storytelling` |
| anything else    | `LayoutKind::Storytelling` (fallback) |

### Visual placeholder convention

Slides with a non-null `visual_spec` get a placeholder text element as their first content element. This element uses `ElementContent::Text` with the markdown body:

```
[[VISUAL_PLACEHOLDER: {visual_spec}]]
```

The VisualAgent (sub-plan 2d) searches all slides for elements matching this pattern and replaces them with actual SVG/chart/diagram content.

### Step 4.1 — Write the tests first (TDD)

- [ ] Append to `crates/minion-presentation/tests/agent_tests.rs`:

```rust
// ── SlidePlannerAgent tests ───────────────────────────────────────────────────

mod slide_planner_tests {
    use minion_presentation::agents::slide_planner::SlidePlannerAgent;
    use minion_presentation::agents::storyteller::{StorySection, StorytellerOutput};
    use minion_presentation::agents::{AgentEvent, next_seq};
    use minion_presentation::schema::types::{LayoutKind, DeckPatch};
    use minion_llm::{
        LlmProvider, JsonExtractRequest, JsonExtractResponse, LlmResult,
        ChatRequest, ChatResponse, ModelInfo,
    };
    use std::sync::{Arc, atomic::AtomicU32};
    use async_trait::async_trait;

    /// Mock provider that returns a section JSON for each call.
    /// `response_idx` advances each call to support multi-section plans.
    struct MockSlidePlannerProvider {
        /// JSON responses to return per call, cycling through the list.
        responses: Vec<String>,
        call_count: std::sync::Mutex<usize>,
    }

    impl MockSlidePlannerProvider {
        fn new(responses: Vec<String>) -> Self {
            Self { responses, call_count: std::sync::Mutex::new(0) }
        }
    }

    #[async_trait]
    impl LlmProvider for MockSlidePlannerProvider {
        fn name(&self) -> &str { "mock-slide-planner" }

        async fn chat(&self, _req: ChatRequest) -> LlmResult<ChatResponse> {
            let idx = {
                let mut count = self.call_count.lock().unwrap();
                let i = *count % self.responses.len();
                *count += 1;
                i
            };
            Ok(ChatResponse {
                content: self.responses[idx].clone(),
                model: "mock".to_string(),
                usage: None,
            })
        }

        async fn health_check(&self) -> LlmResult<bool> { Ok(true) }
        async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> { Ok(vec![]) }
    }

    fn make_storyteller_output_2_sections() -> StorytellerOutput {
        StorytellerOutput {
            title: "Test Deck".to_string(),
            hook: "A great hook".to_string(),
            sections: vec![
                StorySection {
                    title: "Section One".to_string(),
                    slide_count: 2,
                    purpose: "intro".to_string(),
                    pacing: "slow".to_string(),
                },
                StorySection {
                    title: "Section Two".to_string(),
                    slide_count: 3,
                    purpose: "detail".to_string(),
                    pacing: "medium".to_string(),
                },
            ],
            closing_cta: "Act now".to_string(),
            camera_narrative: "zoom out".to_string(),
        }
    }

    fn section_json_2_slides() -> String {
        r#"{
            "slides": [
                {
                    "layout": "title",
                    "headline": "Big Idea",
                    "body": "Here is the idea",
                    "visual_spec": null,
                    "talking_points": ["point 1", "point 2"],
                    "canvas_x": 0,
                    "canvas_y": 0
                },
                {
                    "layout": "kpi",
                    "headline": "99.9% Uptime",
                    "body": "Our SLA commitment",
                    "visual_spec": "bar chart showing monthly uptime percentages",
                    "talking_points": ["we deliver reliability"],
                    "canvas_x": 2100,
                    "canvas_y": 0
                }
            ]
        }"#.to_string()
    }

    fn section_json_3_slides() -> String {
        r#"{
            "slides": [
                {
                    "layout": "storytelling",
                    "headline": "The Journey",
                    "body": "From zero to hero",
                    "visual_spec": null,
                    "talking_points": ["journey narrative"],
                    "canvas_x": 0,
                    "canvas_y": 1280
                },
                {
                    "layout": "comparison",
                    "headline": "Before vs After",
                    "body": "The delta",
                    "visual_spec": "comparison table",
                    "talking_points": ["contrast old and new"],
                    "canvas_x": 2100,
                    "canvas_y": 1280
                },
                {
                    "layout": "process",
                    "headline": "How It Works",
                    "body": "Step by step",
                    "visual_spec": null,
                    "talking_points": ["process overview"],
                    "canvas_x": 4200,
                    "canvas_y": 1280
                }
            ]
        }"#.to_string()
    }

    #[tokio::test]
    async fn slide_planner_builds_correct_slide_count() {
        let provider = Arc::new(MockSlidePlannerProvider::new(vec![
            section_json_2_slides(),
            section_json_3_slides(),
        ]));

        let (tx, _rx) = tokio::sync::broadcast::channel::<AgentEvent>(128);
        let counter = AtomicU32::new(0);
        let story = make_storyteller_output_2_sections();

        let agent = SlidePlannerAgent::new_with_provider(provider);
        let deck = agent.run(&story, &tx, &counter).await.expect("slide planner");

        // 2 + 3 = 5 total slides
        assert_eq!(deck.slide_count(), 5);
        // 2 sections
        assert_eq!(deck.sections.len(), 2);
    }

    #[tokio::test]
    async fn slide_planner_correct_canvas_positions() {
        let provider = Arc::new(MockSlidePlannerProvider::new(vec![
            section_json_2_slides(),
            section_json_3_slides(),
        ]));

        let (tx, _rx) = tokio::sync::broadcast::channel::<AgentEvent>(128);
        let counter = AtomicU32::new(0);
        let story = make_storyteller_output_2_sections();

        let agent = SlidePlannerAgent::new_with_provider(provider);
        let deck = agent.run(&story, &tx, &counter).await.expect("slide planner");

        let section_0 = &deck.sections[0];
        assert_eq!(section_0.slides[0].canvas_x, 0.0);
        assert_eq!(section_0.slides[0].canvas_y, 0.0);
        assert_eq!(section_0.slides[1].canvas_x, 2100.0);
        assert_eq!(section_0.slides[1].canvas_y, 0.0);

        let section_1 = &deck.sections[1];
        assert_eq!(section_1.slides[0].canvas_x, 0.0);
        assert_eq!(section_1.slides[0].canvas_y, 1280.0);
        assert_eq!(section_1.slides[1].canvas_x, 2100.0);
        assert_eq!(section_1.slides[1].canvas_y, 1280.0);
        assert_eq!(section_1.slides[2].canvas_x, 4200.0);
        assert_eq!(section_1.slides[2].canvas_y, 1280.0);
    }

    #[tokio::test]
    async fn slide_planner_correct_layout_kinds() {
        let provider = Arc::new(MockSlidePlannerProvider::new(vec![
            section_json_2_slides(),
            section_json_3_slides(),
        ]));

        let (tx, _rx) = tokio::sync::broadcast::channel::<AgentEvent>(128);
        let counter = AtomicU32::new(0);
        let story = make_storyteller_output_2_sections();

        let agent = SlidePlannerAgent::new_with_provider(provider);
        let deck = agent.run(&story, &tx, &counter).await.expect("slide planner");

        assert_eq!(deck.sections[0].slides[0].layout, LayoutKind::Title);
        assert_eq!(deck.sections[0].slides[1].layout, LayoutKind::Kpi);
        assert_eq!(deck.sections[1].slides[0].layout, LayoutKind::Storytelling);
        assert_eq!(deck.sections[1].slides[1].layout, LayoutKind::Comparison);
        assert_eq!(deck.sections[1].slides[2].layout, LayoutKind::Process);
    }

    #[tokio::test]
    async fn slide_planner_inserts_visual_placeholder_element() {
        let provider = Arc::new(MockSlidePlannerProvider::new(vec![
            section_json_2_slides(),
            section_json_3_slides(),
        ]));

        let (tx, _rx) = tokio::sync::broadcast::channel::<AgentEvent>(128);
        let counter = AtomicU32::new(0);
        let story = make_storyteller_output_2_sections();

        let agent = SlidePlannerAgent::new_with_provider(provider);
        let deck = agent.run(&story, &tx, &counter).await.expect("slide planner");

        use minion_presentation::schema::types::ElementContent;

        // Section 0, slide 1 has visual_spec = "bar chart showing monthly uptime percentages"
        let slide_with_visual = &deck.sections[0].slides[1];
        let has_placeholder = slide_with_visual.elements.iter().any(|el| {
            matches!(&el.content, ElementContent::Text { markdown } if markdown.starts_with("[[VISUAL_PLACEHOLDER:"))
        });
        assert!(has_placeholder, "slide with visual_spec must have a placeholder element");

        // Section 0, slide 0 has visual_spec = null → no placeholder
        let slide_no_visual = &deck.sections[0].slides[0];
        let has_no_placeholder = !slide_no_visual.elements.iter().any(|el| {
            matches!(&el.content, ElementContent::Text { markdown } if markdown.starts_with("[[VISUAL_PLACEHOLDER:"))
        });
        assert!(has_no_placeholder, "slide without visual_spec must not have a placeholder");
    }

    #[tokio::test]
    async fn slide_planner_emits_slide_ready_events() {
        let provider = Arc::new(MockSlidePlannerProvider::new(vec![
            section_json_2_slides(),
            section_json_3_slides(),
        ]));

        let (tx, mut rx) = tokio::sync::broadcast::channel::<AgentEvent>(128);
        let counter = AtomicU32::new(0);
        let story = make_storyteller_output_2_sections();

        let agent = SlidePlannerAgent::new_with_provider(provider);
        agent.run(&story, &tx, &counter).await.expect("slide planner");

        let mut slide_ready_count = 0usize;
        while let Ok(ev) = rx.try_recv() {
            if matches!(ev, AgentEvent::SlideReady { .. }) {
                slide_ready_count += 1;
            }
        }
        // 5 slides total → 5 SlideReady events
        assert_eq!(slide_ready_count, 5);
    }

    #[tokio::test]
    async fn slide_planner_emits_started_and_completed() {
        let provider = Arc::new(MockSlidePlannerProvider::new(vec![
            section_json_2_slides(),
        ]));

        // Use a 1-section story for simplicity
        let story = StorytellerOutput {
            title: "T".to_string(),
            hook: "H".to_string(),
            sections: vec![
                StorySection {
                    title: "Only Section".to_string(),
                    slide_count: 2,
                    purpose: "p".to_string(),
                    pacing: "slow".to_string(),
                },
            ],
            closing_cta: "go".to_string(),
            camera_narrative: "zoom".to_string(),
        };

        let (tx, mut rx) = tokio::sync::broadcast::channel::<AgentEvent>(64);
        let counter = AtomicU32::new(0);

        let agent = SlidePlannerAgent::new_with_provider(provider);
        agent.run(&story, &tx, &counter).await.expect("run");

        let mut events = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            events.push(ev);
        }

        assert!(matches!(events[0], AgentEvent::Started { .. }));
        assert!(matches!(events[events.len() - 1], AgentEvent::Completed { .. }));
    }
}
```

- [ ] Run `cargo test -p minion-presentation --test agent_tests slide_planner_tests 2>&1 | head -30` — expect compile errors.

### Step 4.2 — Implement `agents/slide_planner.rs`

- [ ] Create `crates/minion-presentation/src/agents/slide_planner.rs`:

```rust
//! Slide planner agent — assembles a full Deck skeleton from the narrative plan.
//!
//! Makes one LLM call per section (parallelised, max 3 concurrent). Each
//! section call returns a list of slides with layout, headline, body, optional
//! visual spec, and talking points. Visual specs become placeholder text
//! elements for the VisualAgent to replace in sub-plan 2d.

use std::sync::{Arc, atomic::AtomicU32};

use anyhow::Context as _;
use chrono::Utc;
use minion_llm::{JsonExtractRequest, LlmProvider};
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

use crate::{
    agents::{agent_name, next_seq, AgentEvent, EventTx},
    schema::types::{
        AnimEffect, AnimPhase, AnimTrigger, AspectRatio, Deck, DeckMeta, DeckPatch, Direction,
        Element, ElementAnimation, ElementContent, ElementId, ElementKind, ElementStyle,
        LayoutKind, MasterSlide, PresentationContext, Section, SectionId, Slide, TextDirection,
        Theme,
    },
};
use super::storyteller::{StorySection, StorytellerOutput};

// ── Intermediate JSON types from LLM ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct LlmSlide {
    layout: String,
    headline: String,
    body: String,
    visual_spec: Option<String>,
    talking_points: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LlmSectionResponse {
    slides: Vec<LlmSlide>,
}

// ── Canvas layout constants ───────────────────────────────────────────────────

/// Slide width in canvas units (matches `Slide::new` default).
const SLIDE_WIDTH: f64 = 1920.0;
/// Horizontal gap between slides in the same section.
const SLIDE_GAP: f64 = 180.0;
/// Horizontal stride per slide: width + gap.
const SLIDE_STRIDE_X: f64 = SLIDE_WIDTH + SLIDE_GAP; // 2100.0
/// Vertical stride between sections.
const SECTION_STRIDE_Y: f64 = 1280.0;

// ── LLM prompts ───────────────────────────────────────────────────────────────

const SYSTEM_PROMPT: &str = "\
You are a presentation slide designer. Given a narrative section description, generate the slides \
for that section. For each slide choose the best layout:\n\
  title        → opening/title slide\n\
  kpi          → statistics and numbers\n\
  comparison   → side-by-side comparisons\n\
  process      → step-by-step process\n\
  architecture → system/architecture diagrams\n\
  quote        → impactful quotes\n\
  timeline     → chronological sequences\n\
  storytelling → general narrative content\n\
Return ONLY valid JSON matching the schema.";

const EXAMPLE_JSON: &str = r#"{
  "slides": [
    {
      "layout": "title",
      "headline": "The Big Idea",
      "body": "A one-sentence summary",
      "visual_spec": null,
      "talking_points": ["open with the hook", "pause for effect"],
      "canvas_x": 0,
      "canvas_y": 0
    },
    {
      "layout": "kpi",
      "headline": "10x Faster",
      "body": "Benchmark results across 5 workloads",
      "visual_spec": "bar chart comparing old vs new latency at p50/p99",
      "talking_points": ["cite the benchmark study"],
      "canvas_x": 2100,
      "canvas_y": 0
    }
  ]
}"#;

// ── Layout string → LayoutKind ────────────────────────────────────────────────

fn parse_layout(s: &str) -> LayoutKind {
    match s {
        "title" => LayoutKind::Title,
        "kpi" => LayoutKind::Kpi,
        "comparison" => LayoutKind::Comparison,
        "process" => LayoutKind::Process,
        "architecture" => LayoutKind::Architecture,
        "quote" => LayoutKind::Quote,
        "timeline" => LayoutKind::Timeline,
        _ => LayoutKind::Storytelling,
    }
}

// ── Placeholder element constructor ──────────────────────────────────────────

fn placeholder_element(visual_spec: &str) -> Element {
    Element {
        id: ElementId::new(),
        kind: ElementKind::Text,
        content: ElementContent::Text {
            markdown: format!("[[VISUAL_PLACEHOLDER: {visual_spec}]]"),
        },
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
        z_index: 0,
        style: ElementStyle::default(),
        animation: ElementAnimation {
            entrance: None,
            exit: None,
            emphasis: None,
            trigger: AnimTrigger::OnSlideEnter,
        },
        user_asset_id: None,
        locked: false,
    }
}

// ── Headline/body text element constructor ────────────────────────────────────

fn text_element(markdown: String, x: f64, y: f64, width: f64, height: f64, z: u32) -> Element {
    Element {
        id: ElementId::new(),
        kind: ElementKind::Text,
        content: ElementContent::Text { markdown },
        x,
        y,
        width,
        height,
        z_index: z,
        style: ElementStyle::default(),
        animation: ElementAnimation {
            entrance: Some(AnimPhase {
                effect: AnimEffect::Fade,
                delay_ms: 0,
                duration_ms: 400,
                spring: None,
            }),
            exit: None,
            emphasis: None,
            trigger: AnimTrigger::OnSlideEnter,
        },
        user_asset_id: None,
        locked: false,
    }
}

// ── Agent ─────────────────────────────────────────────────────────────────────

/// Assembles a full `Deck` skeleton with placeholder visual elements.
pub struct SlidePlannerAgent {
    provider: Arc<dyn LlmProvider>,
}

impl SlidePlannerAgent {
    /// Construct with a router-selected provider.
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    /// Construct with an explicit provider (useful for tests).
    pub fn new_with_provider(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    /// Run the slide planning phase.
    ///
    /// Emits: `Started` → `SlideReady` per slide (as each section completes) → `Completed`.
    pub async fn run(
        &self,
        story: &StorytellerOutput,
        event_tx: &EventTx,
        seq: &AtomicU32,
    ) -> anyhow::Result<Deck> {
        let _ = event_tx.send(AgentEvent::Started {
            seq: next_seq(seq),
            agent: agent_name::SLIDE_PLANNER.to_string(),
        });

        // Plan one LLM call per section, max 3 concurrent.
        const MAX_CONCURRENT: usize = 3;
        let sections_data = story.sections.clone();
        let section_count = sections_data.len();

        // Collect (section_index, LlmSectionResponse) pairs.
        // We process in chunks of MAX_CONCURRENT.
        let mut section_results: Vec<(usize, LlmSectionResponse)> =
            Vec::with_capacity(section_count);

        let mut chunk_start = 0usize;
        while chunk_start < section_count {
            let chunk_end = (chunk_start + MAX_CONCURRENT).min(section_count);
            let mut join_set: JoinSet<anyhow::Result<(usize, LlmSectionResponse)>> =
                JoinSet::new();

            for idx in chunk_start..chunk_end {
                let section = sections_data[idx].clone();
                let provider = Arc::clone(&self.provider);
                let section_y = idx as f64 * SECTION_STRIDE_Y;

                join_set.spawn(async move {
                    let result = plan_section(&*provider, &section, idx, section_y).await?;
                    Ok((idx, result))
                });
            }

            while let Some(join_result) = join_set.join_next().await {
                let (idx, llm_section) =
                    join_result.context("SlidePlannerAgent: join error")??;
                section_results.push((idx, llm_section));
            }

            chunk_start = chunk_end;
        }

        // Sort by section index to restore order (JoinSet completes out of order).
        section_results.sort_by_key(|(idx, _)| *idx);

        // Assemble the Deck.
        let mut global_slide_index: u32 = 0;
        let mut sections: Vec<Section> = Vec::with_capacity(section_count);

        for (section_idx, llm_section) in section_results {
            let section_id = SectionId::new();
            let section_title = story.sections[section_idx].title.clone();
            let section_y = section_idx as f64 * SECTION_STRIDE_Y;
            let mut slides: Vec<Slide> = Vec::with_capacity(llm_section.slides.len());

            for (slide_idx, llm_slide) in llm_section.slides.iter().enumerate() {
                let canvas_x = slide_idx as f64 * SLIDE_STRIDE_X;
                let layout = parse_layout(&llm_slide.layout);
                let mut slide = Slide::new(section_id.clone(), canvas_x, section_y, layout);

                // Add headline element (top area)
                slide.elements.push(text_element(
                    format!("## {}", llm_slide.headline),
                    48.0, 80.0, 1824.0, 160.0, 1,
                ));

                // Add body element (middle area)
                slide.elements.push(text_element(
                    llm_slide.body.clone(),
                    48.0, 260.0, 1824.0, 600.0, 2,
                ));

                // Add visual placeholder if spec is present
                if let Some(spec) = &llm_slide.visual_spec {
                    slide.elements.push(placeholder_element(spec));
                }

                // Populate speaker notes
                slide.speaker_notes.talking_points = llm_slide.talking_points.clone();

                // Emit SlideReady with an UpsertSlide patch
                let patch = DeckPatch::UpsertSlide {
                    section_id: section_id.clone(),
                    slide: slide.clone(),
                };
                let _ = event_tx.send(AgentEvent::SlideReady {
                    seq: next_seq(seq),
                    agent: agent_name::SLIDE_PLANNER.to_string(),
                    slide_index: global_slide_index,
                    patch,
                });
                global_slide_index += 1;

                slides.push(slide);
            }

            sections.push(Section { id: section_id, title: section_title, slides });
        }

        // Assemble play_order from all slides in section order
        let play_order = sections
            .iter()
            .flat_map(|s| s.slides.iter().map(|sl| sl.id.clone()))
            .collect();

        let deck = Deck {
            meta: DeckMeta {
                title: story.title.clone(),
                author: String::new(),
                deck_revision: 1,
                schema_version: "1.0".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                aspect_ratio: AspectRatio::Ratio16x9,
                language: "en-US".to_string(),
                text_direction: TextDirection::Ltr,
                target_duration_mins: None,
                presentation_context: PresentationContext::LiveTalk,
            },
            theme: Theme::default(),
            master: MasterSlide { elements: vec![], background: None },
            assets: vec![],
            camera_path: vec![],
            sections,
            play_order,
        };

        let _ = event_tx.send(AgentEvent::Completed {
            seq: next_seq(seq),
            agent: agent_name::SLIDE_PLANNER.to_string(),
        });

        Ok(deck)
    }
}

// ── Section planning helper (called in JoinSet tasks) ─────────────────────────

async fn plan_section(
    provider: &dyn LlmProvider,
    section: &StorySection,
    section_idx: usize,
    section_y: f64,
) -> anyhow::Result<LlmSectionResponse> {
    let user_input = format!(
        "Generate slides for this presentation section:\n\n\
         Section title: {}\n\
         Purpose: {}\n\
         Pacing: {}\n\
         Target slide count: {}\n\
         Section index (for canvas_y = {:.0}): {}\n\n\
         Place slides at canvas_y = {:.0}, with canvas_x starting at 0 and \
         incrementing by 2100 per slide.",
        section.title,
        section.purpose,
        section.pacing,
        section.slide_count,
        section_y,
        section_idx,
        section_y,
    );

    let req = JsonExtractRequest {
        system_prompt: SYSTEM_PROMPT.to_string(),
        user_input,
        example_json: EXAMPLE_JSON.to_string(),
        model: None,
        temperature: Some(0.4),
    };

    let resp = provider
        .extract_json(req)
        .await
        .context("SlidePlannerAgent: extract_json failed for section")?;

    let section_resp: LlmSectionResponse = serde_json::from_value(resp.parsed)
        .context("SlidePlannerAgent: failed to deserialize section slides")?;

    Ok(section_resp)
}
```

### Step 4.3 — Run all tests

- [ ] Run: `cargo test -p minion-presentation --test agent_tests 2>&1`

  All tests (infrastructure + research + storyteller + slide_planner) must pass.

- [ ] Run full suite: `cargo test -p minion-presentation 2>&1`

---

## Task 5: Wire `lib.rs` + end-to-end pipeline smoke test

**Goal:** Verify the full pipeline (Research → Storyteller → SlidePlanner) executes in sequence with mock providers, producing a coherent Deck.

### Step 5.1 — Confirm `lib.rs` is correct

- [ ] Verify `crates/minion-presentation/src/lib.rs` contains:

```rust
pub mod agents;
pub mod context;
pub mod db;
pub mod migrations;
pub mod router;
pub mod schema;

pub use schema::types::*;
```

- [ ] Run: `cargo build -p minion-presentation 2>&1 | tail -10` — must compile clean.

### Step 5.2 — Add pipeline smoke test

- [ ] Append to `crates/minion-presentation/tests/agent_tests.rs`:

```rust
// ── Pipeline smoke test ───────────────────────────────────────────────────────

mod pipeline_smoke {
    use minion_presentation::agents::{
        AgentEvent,
        research::ResearchAgent,
        storyteller::StorytellerAgent,
        slide_planner::SlidePlannerAgent,
    };
    use minion_presentation::schema::types::GenerationConfig;
    use minion_llm::{
        LlmProvider, LlmResult, ChatRequest, ChatResponse, ModelInfo,
    };
    use std::sync::{Arc, atomic::AtomicU32};
    use async_trait::async_trait;
    use minion_presentation::schema::types::PresentationContext;

    struct MultiStageMock;

    #[async_trait]
    impl LlmProvider for MultiStageMock {
        fn name(&self) -> &str { "multi-stage-mock" }

        async fn chat(&self, req: ChatRequest) -> LlmResult<ChatResponse> {
            // Detect which stage is calling based on prompt content
            let prompt = req.messages.first().map(|m| m.content.as_str()).unwrap_or("");
            let system = req.system.as_deref().unwrap_or("");

            let response = if system.contains("research analyst") {
                // ResearchAgent call
                r#"{
                    "audience": "developers",
                    "tone": "technical",
                    "language": "en-US",
                    "key_themes": ["performance", "reliability"],
                    "facts": [{"claim": "5x speedup", "source": "bench", "confidence": 0.9}],
                    "suggested_section_count": 2,
                    "target_duration_mins": 10
                }"#
            } else if system.contains("master storyteller") {
                // StorytellerAgent call
                r#"{
                    "title": "Speed & Reliability",
                    "hook": "What if you could have both?",
                    "sections": [
                        {"title": "The Problem", "slide_count": 2, "purpose": "context", "pacing": "slow"},
                        {"title": "The Solution", "slide_count": 2, "purpose": "answer", "pacing": "energetic"}
                    ],
                    "closing_cta": "Ship it",
                    "camera_narrative": "zoom in on key moments"
                }"#
            } else {
                // SlidePlannerAgent call (one per section)
                r#"{
                    "slides": [
                        {
                            "layout": "title",
                            "headline": "Slide Heading",
                            "body": "Supporting detail",
                            "visual_spec": null,
                            "talking_points": ["key point"],
                            "canvas_x": 0,
                            "canvas_y": 0
                        },
                        {
                            "layout": "storytelling",
                            "headline": "More Detail",
                            "body": "More content",
                            "visual_spec": null,
                            "talking_points": ["another point"],
                            "canvas_x": 2100,
                            "canvas_y": 0
                        }
                    ]
                }"#
            };

            Ok(ChatResponse {
                content: response.to_string(),
                model: "mock".to_string(),
                usage: None,
            })
        }

        async fn health_check(&self) -> LlmResult<bool> { Ok(true) }
        async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> { Ok(vec![]) }
    }

    #[tokio::test]
    async fn full_pipeline_research_storyteller_slide_planner() {
        let provider: Arc<dyn LlmProvider> = Arc::new(MultiStageMock);
        let (tx, mut rx) = tokio::sync::broadcast::channel::<AgentEvent>(256);
        let counter = AtomicU32::new(0);

        let config = GenerationConfig {
            theme_name: None,
            audience: "developers".to_string(),
            tone: "technical".to_string(),
            language: "en-US".to_string(),
            target_duration_mins: Some(10),
            slide_count_hint: None,
            presentation_context: PresentationContext::LiveTalk,
        };

        // Stage 1: Research
        let research_agent = ResearchAgent::new_with_provider(Arc::clone(&provider));
        let research_output = research_agent
            .run("sample corpus content".to_string(), &config, &tx, &counter)
            .await
            .expect("research stage");

        assert_eq!(research_output.audience, "developers");
        assert_eq!(research_output.suggested_section_count, 2);

        // Stage 2: Storyteller
        let story_agent = StorytellerAgent::new_with_provider(Arc::clone(&provider));
        let story_output = story_agent
            .run(&research_output, &tx, &counter)
            .await
            .expect("storyteller stage");

        assert_eq!(story_output.title, "Speed & Reliability");
        assert_eq!(story_output.sections.len(), 2);

        // Stage 3: SlidePlanner
        let planner_agent = SlidePlannerAgent::new_with_provider(Arc::clone(&provider));
        let deck = planner_agent
            .run(&story_output, &tx, &counter)
            .await
            .expect("slide planner stage");

        // 2 sections × 2 slides each = 4 slides
        assert_eq!(deck.slide_count(), 4);
        assert_eq!(deck.sections.len(), 2);
        assert_eq!(deck.meta.title, "Speed & Reliability");
        assert_eq!(deck.play_order.len(), 4);

        // Verify events were emitted
        let mut events = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            events.push(ev);
        }

        let slide_ready_count = events.iter()
            .filter(|e| matches!(e, AgentEvent::SlideReady { .. }))
            .count();
        assert_eq!(slide_ready_count, 4, "4 SlideReady events expected");

        let started_count = events.iter()
            .filter(|e| matches!(e, AgentEvent::Started { .. }))
            .count();
        assert_eq!(started_count, 3, "one Started per agent (3 agents)");

        let completed_count = events.iter()
            .filter(|e| matches!(e, AgentEvent::Completed { .. }))
            .count();
        assert_eq!(completed_count, 3, "one Completed per agent (3 agents)");
    }
}
```

### Step 5.3 — Run the full test suite

- [ ] Run: `cargo test -p minion-presentation 2>&1`

  Every test must pass: schema tests, router tests, context tests, agent infrastructure tests, research tests, storyteller tests, slide planner tests, pipeline smoke test.

### Step 5.4 — Clippy

- [ ] Run: `cargo clippy -p minion-presentation -- -D warnings 2>&1`

  Fix any warnings. Common issues to check:
  - Unused imports in agent modules
  - Needless `Arc::clone` calls that could be references
  - `allow(dead_code)` not needed if all public items are used by tests

### Step 5.5 — Workspace build

- [ ] Run: `cargo build --workspace 2>&1 | tail -20`

  Workspace must build clean.

### Step 5.6 — Commit

- [ ] Commit with:

```
feat(presentation): add agent event infrastructure and core agents (sub-plan 2c)

- AgentEvent enum with kind tag + snake_case serde (matches TS types)
- EventTx broadcast channel type + next_seq atomic helper
- ResearchAgent: extracts audience/tone/facts from corpus via extract_json
- StorytellerAgent: builds narrative structure from ResearchOutput
- SlidePlannerAgent: produces full Deck skeleton with JoinSet parallelism
  (max 3 concurrent section calls), visual placeholder elements, and
  SlideReady events per slide
- ContextManager gains new_for_testing() for unit tests without RAG
- 22+ tests across 5 test modules, all passing
```

---

## Acceptance Criteria

Before marking sub-plan 2c complete, all of the following must be true:

- [ ] `crates/minion-presentation/src/agents/mod.rs` exists with `AgentEvent`, `EventTx`, `next_seq`, `agent_name`
- [ ] `crates/minion-presentation/src/agents/research.rs` exists with `ResearchAgent`, `ResearchOutput`, `ResearchFact`
- [ ] `crates/minion-presentation/src/agents/storyteller.rs` exists with `StorytellerAgent`, `StorytellerOutput`, `StorySection`
- [ ] `crates/minion-presentation/src/agents/slide_planner.rs` exists with `SlidePlannerAgent`
- [ ] `crates/minion-presentation/src/lib.rs` has `pub mod agents;`
- [ ] `crates/minion-presentation/tests/agent_tests.rs` exists with all test modules
- [ ] `cargo test -p minion-presentation 2>&1` — all tests green (22+ tests)
- [ ] `cargo clippy -p minion-presentation -- -D warnings 2>&1` — zero warnings
- [ ] `cargo build --workspace 2>&1` — workspace builds clean
- [ ] `AgentEvent` JSON serialization verified: `kind` tag is snake_case, matches TypeScript union
- [ ] Canvas positions verified: slide X = `index * 2100.0`, section Y = `index * 1280.0`
- [ ] Visual placeholder format: `[[VISUAL_PLACEHOLDER: {spec}]]` (prefix exactly as shown)
- [ ] No `unwrap()` in non-test production code (use `?` and `context()` instead)
- [ ] Conventional commit created

---

## Self-Review Checklist

1. **`AgentEvent::SlideReady` `patch` field type** — `DeckPatch` from `schema/types.rs` (confirmed: `DeckPatch::UpsertSlide { section_id: SectionId, slide: Slide }`) ✓
2. **`Slide::new()` constructor** — exists at `schema/types.rs:582`, signature `new(section_id: SectionId, x: f64, y: f64, layout: LayoutKind) -> Self` ✓
3. **Canvas position arithmetic** — explicit constants: `SLIDE_STRIDE_X = 2100.0`, `SECTION_STRIDE_Y = 1280.0`; verified by test assertions ✓
4. **Mock `LlmProvider` in tests** — implements `LlmProvider` trait with all required methods (`name`, `chat`, `health_check`, `list_models`); uses `async_trait` ✓
5. **No `unwrap()` in production code** — all fallible calls use `?` with `anyhow::Context` ✓
6. **`AgentEvent` serde shape** — `#[serde(tag = "kind", rename_all = "snake_case")]` ensures `kind: "started"` etc.; `StreamComplete` and `StreamError` have no `agent` field (verified against TypeScript union) ✓
7. **`ContextManager::new_for_testing()`** — added as `#[cfg(test)]` constructor, `rag` field changed to `Option<Arc<RagPipeline>>` with graceful error on `compress_to_budget` when `None` ✓
8. **`SlidePlannerAgent` concurrency** — uses `tokio::JoinSet` with chunking at `MAX_CONCURRENT = 3`; results sorted by section index after JoinSet drains ✓
9. **Play order** — assembled from all slide IDs in section order after sections are constructed ✓

---

## Import Reference (verified against codebase)

| Symbol | Import path |
|---|---|
| `LlmProvider` | `minion_llm::LlmProvider` |
| `JsonExtractRequest` | `minion_llm::JsonExtractRequest` |
| `JsonExtractResponse` | `minion_llm::JsonExtractResponse` |
| `ChatRequest` | `minion_llm::ChatRequest` |
| `ChatResponse` | `minion_llm::ChatResponse` |
| `ModelInfo` | `minion_llm::ModelInfo` |
| `LlmResult` | `minion_llm::LlmResult` |
| `Deck`, `Slide`, `Section`, `SectionId`, `SlideId` | `minion_presentation::schema::types::*` |
| `DeckPatch` | `minion_presentation::schema::types::DeckPatch` |
| `LayoutKind` | `minion_presentation::schema::types::LayoutKind` |
| `ElementContent` | `minion_presentation::schema::types::ElementContent` |
| `GenerationConfig` | `minion_presentation::schema::types::GenerationConfig` |
| `PresentationRouter` | `minion_presentation::router::PresentationRouter` |
| `ContextManager` | `minion_presentation::context::ContextManager` |

`async_trait` is a workspace dependency (confirmed in `Cargo.toml`). All `minion_llm` symbols above are re-exported at crate root (`minion_llm/src/lib.rs` lines 15–22).
