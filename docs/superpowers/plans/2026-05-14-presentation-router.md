# Presentation Module — Sub-Plan 2b: LLM Router + Context Manager

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the LLM routing layer that directs each agent task to the right model (Ollama or OpenAI), and a context manager that tracks token budgets and uses RAG to compress large inputs.

**Architecture:** PresentationRouter wraps the existing minion-llm provider factory with task-aware model selection. ContextManager wraps minion-rag with a token budget gate — inputs that fit go through unchanged, oversized inputs are RAG-compressed to the most relevant chunks.

**Tech Stack:** Rust, minion-llm (LlmProvider, create_provider, EndpointConfig), minion-rag (RagPipeline, OllamaEmbedder), tokio.

---

## Prerequisites

Sub-plan 1 (Foundation) must be complete. Confirm the following exist before starting:

- `crates/minion-presentation/src/lib.rs` with `pub use schema::types::*`
- `crates/minion-llm/src/lib.rs` re-exports: `create_provider`, `LlmProvider`, `EndpointConfig`, `ProviderType`
- `crates/minion-rag/src/lib.rs` re-exports: `RagPipeline`, `SearchHit`

Verify imports once before Task 1:

```bash
grep "pub use" /home/dk/Documents/git/minion/crates/minion-llm/src/lib.rs
grep "pub use" /home/dk/Documents/git/minion/crates/minion-rag/src/lib.rs
```

Expected: `create_provider` and `LlmProvider` are at crate root (`minion_llm::create_provider`). `RagPipeline` is at crate root (`minion_rag::RagPipeline`).

---

## Task 1: RouterConfig + PresentationRouter

**File to create:** `crates/minion-presentation/src/router.rs`

**File to create (tests):** `crates/minion-presentation/tests/router_tests.rs`

### Step 1.1 — Write the tests first (TDD)

- [ ] Create `crates/minion-presentation/tests/router_tests.rs` with the following tests:

```rust
use minion_presentation::router::{PresentationRouter, RouterConfig, RouterProvider, RoutingTask};

#[test]
fn ollama_text_task_uses_text_model() {
    let config = RouterConfig {
        provider: RouterProvider::Ollama,
        ollama_base_url: "http://localhost:11434".to_string(),
        openai_api_key: None,
        ollama_text_model: "llama3.2:latest".to_string(),
        ollama_vision_model: "llava:latest".to_string(),
        openai_text_model: "gpt-4o-mini".to_string(),
        openai_vision_model: "gpt-4o".to_string(),
    };
    let router = PresentationRouter::new(config);
    let model = router.model_for(RoutingTask::ResearchExtraction);
    assert_eq!(model, "llama3.2:latest");
}

#[test]
fn ollama_vision_task_uses_vision_model() {
    let config = RouterConfig {
        provider: RouterProvider::Ollama,
        ollama_base_url: "http://localhost:11434".to_string(),
        openai_api_key: None,
        ollama_text_model: "llama3.2:latest".to_string(),
        ollama_vision_model: "llava:latest".to_string(),
        openai_text_model: "gpt-4o-mini".to_string(),
        openai_vision_model: "gpt-4o".to_string(),
    };
    let router = PresentationRouter::new(config);
    let model = router.model_for(RoutingTask::SvgGeneration);
    assert_eq!(model, "llava:latest");
    let model2 = router.model_for(RoutingTask::OcrImageDescription);
    assert_eq!(model2, "llava:latest");
}

#[test]
fn openai_text_task_uses_text_model() {
    let config = RouterConfig {
        provider: RouterProvider::OpenAI,
        ollama_base_url: "http://localhost:11434".to_string(),
        openai_api_key: Some("sk-test".to_string()),
        ollama_text_model: "llama3.2:latest".to_string(),
        ollama_vision_model: "llava:latest".to_string(),
        openai_text_model: "gpt-4o-mini".to_string(),
        openai_vision_model: "gpt-4o".to_string(),
    };
    let router = PresentationRouter::new(config);
    let model = router.model_for(RoutingTask::NarrativeGeneration);
    assert_eq!(model, "gpt-4o-mini");
    let model2 = router.model_for(RoutingTask::SlideContentPlanning);
    assert_eq!(model2, "gpt-4o-mini");
    let model3 = router.model_for(RoutingTask::ChartDiagramDsl);
    assert_eq!(model3, "gpt-4o-mini");
    let model4 = router.model_for(RoutingTask::DesignCritique);
    assert_eq!(model4, "gpt-4o-mini");
}

#[test]
fn openai_vision_task_uses_vision_model() {
    let config = RouterConfig {
        provider: RouterProvider::OpenAI,
        ollama_base_url: "http://localhost:11434".to_string(),
        openai_api_key: Some("sk-test".to_string()),
        ollama_text_model: "llama3.2:latest".to_string(),
        ollama_vision_model: "llava:latest".to_string(),
        openai_text_model: "gpt-4o-mini".to_string(),
        openai_vision_model: "gpt-4o".to_string(),
    };
    let router = PresentationRouter::new(config);
    let model = router.model_for(RoutingTask::SvgGeneration);
    assert_eq!(model, "gpt-4o");
    let model2 = router.model_for(RoutingTask::OcrImageDescription);
    assert_eq!(model2, "gpt-4o");
}

#[test]
fn provider_for_ollama_returns_ollama_provider() {
    let config = RouterConfig {
        provider: RouterProvider::Ollama,
        ollama_base_url: "http://localhost:11434".to_string(),
        openai_api_key: None,
        ollama_text_model: "llama3.2:latest".to_string(),
        ollama_vision_model: "llava:latest".to_string(),
        openai_text_model: "gpt-4o-mini".to_string(),
        openai_vision_model: "gpt-4o".to_string(),
    };
    let router = PresentationRouter::new(config);
    let provider = router.provider_for(RoutingTask::ResearchExtraction);
    // OllamaProvider::name() returns "ollama"
    assert!(provider.name().contains("ollama"), "expected ollama provider, got: {}", provider.name());
}

#[test]
fn provider_for_openai_returns_openai_compatible_provider() {
    let config = RouterConfig {
        provider: RouterProvider::OpenAI,
        ollama_base_url: "http://localhost:11434".to_string(),
        openai_api_key: Some("sk-test".to_string()),
        ollama_text_model: "llama3.2:latest".to_string(),
        ollama_vision_model: "llava:latest".to_string(),
        openai_text_model: "gpt-4o-mini".to_string(),
        openai_vision_model: "gpt-4o".to_string(),
    };
    let router = PresentationRouter::new(config);
    let provider = router.provider_for(RoutingTask::SlideContentPlanning);
    // OpenAICompatibleProvider::name() returns "openai" or "openai-compatible"
    assert!(
        provider.name().contains("openai"),
        "expected openai provider, got: {}",
        provider.name()
    );
}
```

- [ ] Run `cargo test -p minion-presentation --test router_tests 2>&1 | head -30` — expect compile errors (types not yet defined). This confirms the test file is wired correctly.

### Step 1.2 — Implement router.rs

- [ ] Create `crates/minion-presentation/src/router.rs`:

```rust
//! LLM router — maps agent tasks to the right provider and model.

use minion_llm::{create_provider, EndpointConfig, LlmProvider, ProviderType};

// ── Task taxonomy ─────────────────────────────────────────────────────────────

/// The kind of work an agent step performs. Vision-capable tasks are routed to
/// the vision model; all others go to the text model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingTask {
    ResearchExtraction,
    NarrativeGeneration,
    SlideContentPlanning,
    /// Preferred: vision-capable model.
    SvgGeneration,
    ChartDiagramDsl,
    /// Preferred: vision-capable model.
    OcrImageDescription,
    DesignCritique,
}

impl RoutingTask {
    /// Returns `true` for tasks where a vision-capable model is preferred.
    pub fn needs_vision(self) -> bool {
        matches!(self, Self::SvgGeneration | Self::OcrImageDescription)
    }
}

// ── Config ────────────────────────────────────────────────────────────────────

/// Which provider backend the router should use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouterProvider {
    Ollama,
    OpenAI,
}

/// Configuration for [`PresentationRouter`].
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Which backend to use.
    pub provider: RouterProvider,
    /// Ollama base URL (ignored when provider is OpenAI).
    pub ollama_base_url: String,
    /// OpenAI API key (required when provider is OpenAI).
    pub openai_api_key: Option<String>,
    /// Ollama model for text tasks.
    pub ollama_text_model: String,
    /// Ollama model for vision tasks.
    pub ollama_vision_model: String,
    /// OpenAI model for text tasks.
    pub openai_text_model: String,
    /// OpenAI model for vision tasks.
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

// ── Router ────────────────────────────────────────────────────────────────────

/// Routes each [`RoutingTask`] to the appropriate [`LlmProvider`] and model.
pub struct PresentationRouter {
    config: RouterConfig,
}

impl PresentationRouter {
    /// Create a new router from the given config.
    pub fn new(config: RouterConfig) -> Self {
        Self { config }
    }

    /// Return the model name that would be selected for `task` (useful for
    /// logging/tracing without constructing a full provider).
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

    /// Build and return a [`LlmProvider`] configured for `task`.
    ///
    /// Each call constructs a new provider instance (providers are cheap to
    /// create — they hold no persistent connections).
    pub fn provider_for(&self, task: RoutingTask) -> Box<dyn LlmProvider> {
        let model = self.model_for(task);
        let endpoint = match &self.config.provider {
            RouterProvider::Ollama => EndpointConfig {
                provider_type: ProviderType::Ollama,
                base_url: self.config.ollama_base_url.clone(),
                api_key: None,
                default_model: model,
                extra_headers: Default::default(),
            },
            RouterProvider::OpenAI => EndpointConfig {
                provider_type: ProviderType::OpenAI,
                base_url: "https://api.openai.com".to_string(),
                api_key: self.config.openai_api_key.clone(),
                default_model: model,
                extra_headers: Default::default(),
            },
        };
        create_provider(endpoint)
    }
}
```

- [ ] Verify the `EndpointConfig` field names match the actual struct. Run:

```bash
grep -n "pub " /home/dk/Documents/git/minion/crates/minion-llm/src/types.rs | head -40
```

  Adjust field names in `router.rs` if they differ (e.g. `extra_headers` may be named differently or absent — remove it if not present).

- [ ] Run tests: `cargo test -p minion-presentation --test router_tests 2>&1`

  All 6 tests must pass. Fix any compile errors before moving on.

---

## Task 2: ContextManager

**File to create:** `crates/minion-presentation/src/context.rs`

### Step 2.1 — Write the tests first (TDD)

- [ ] Append to `crates/minion-presentation/tests/router_tests.rs` (or create a separate `context_tests.rs` — either is fine):

```rust
// ── ContextManager tests ──────────────────────────────────────────────────────

// Note: ContextManager requires a RagPipeline. For unit tests we construct one
// with an in-memory SQLite store and a fake embedder that returns zero vectors.
// The compress_to_budget path we care about for "under budget" needs no real
// embedder because it returns early. The "over budget" path calls rag.search(),
// so we verify it returns *something shorter* rather than checking exact content.

#[cfg(test)]
mod context_tests {
    use minion_presentation::context::ContextManager;
    use minion_rag::{OllamaEmbedder, RagPipeline};
    use minion_rag::store::VectorStore;
    use std::sync::Arc;

    fn make_pipeline() -> Arc<RagPipeline> {
        // In-memory SQLite store (empty path = in-memory for test purposes).
        // If VectorStore::open requires a file path, use tempfile::NamedTempFile.
        let store = VectorStore::open(":memory:", 384)
            .expect("in-memory store");
        // OllamaEmbedder with a non-existent URL — only used when search() is
        // called, which happens only in the "over budget" branch.
        let embedder = Arc::new(OllamaEmbedder::new(
            "http://localhost:19999".to_string(), // intentionally unreachable
            "nomic-embed-text".to_string(),
        ));
        Arc::new(RagPipeline::new(store, embedder))
    }

    #[test]
    fn estimate_tokens_empty() {
        assert_eq!(ContextManager::estimate_tokens(""), 0);
    }

    #[test]
    fn estimate_tokens_four_chars_is_one_token() {
        // "abcd" = 4 chars → 1 token
        assert_eq!(ContextManager::estimate_tokens("abcd"), 1);
    }

    #[test]
    fn estimate_tokens_rounds_down() {
        // "abc" = 3 chars → 0 tokens (integer division)
        assert_eq!(ContextManager::estimate_tokens("abc"), 0);
        // "abcdefg" = 7 chars → 1 token
        assert_eq!(ContextManager::estimate_tokens("abcdefg"), 1);
    }

    #[tokio::test]
    async fn compress_returns_full_text_when_under_budget() {
        let pipeline = make_pipeline();
        let mut mgr = ContextManager::new(pipeline);
        let text = "Hello world"; // 11 chars ≈ 2 tokens
        let result = mgr
            .compress_to_budget("hello", text, 100)
            .await
            .expect("compress");
        assert_eq!(result, text, "text under budget must be returned unchanged");
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
        mgr.record_usage(100_000); // far more than max_context_tokens
        assert_eq!(mgr.remaining(), 0);
    }
}
```

- [ ] Confirm `VectorStore::open` signature. Run:

```bash
grep -n "pub fn open\|pub fn new" /home/dk/Documents/git/minion/crates/minion-rag/src/store.rs | head -10
grep -n "pub fn new" /home/dk/Documents/git/minion/crates/minion-rag/src/embeddings.rs 2>/dev/null | head -10
```

  Adjust `make_pipeline()` in the tests to match the actual constructor signatures. Common variants:
  - `VectorStore::open(path: &str, dim: usize)` — use `":memory:"` and `384`
  - `VectorStore::new(conn: Connection, dim: usize)` — open a rusqlite `Connection::open_in_memory()` first

  Also verify `OllamaEmbedder::new` signature and adjust accordingly.

- [ ] Run: `cargo test -p minion-presentation context_tests 2>&1 | head -40` — expect compile errors (ContextManager not yet defined). Confirms wiring.

### Step 2.2 — Implement context.rs

- [ ] Create `crates/minion-presentation/src/context.rs`:

```rust
//! Token-budget tracking and RAG-based context compression.

use std::sync::Arc;

use minion_rag::{RagPipeline, SearchHit};

/// Manages the token budget for a single agent pipeline run and compresses
/// oversized inputs using RAG retrieval.
pub struct ContextManager {
    rag: Arc<RagPipeline>,
    max_context_tokens: usize,
    used_tokens: usize,
}

impl ContextManager {
    /// Default maximum context tokens. Chosen to be safe for both
    /// `gpt-4o-mini` (128 k context) and `llama3.2` (8 k context).
    const DEFAULT_MAX_TOKENS: usize = 8_000;

    /// Create a new `ContextManager` backed by the given RAG pipeline.
    pub fn new(rag: Arc<RagPipeline>) -> Self {
        Self {
            rag,
            max_context_tokens: Self::DEFAULT_MAX_TOKENS,
            used_tokens: 0,
        }
    }

    /// Rough token estimate: `chars / 4` (integer division).
    ///
    /// Accurate enough for budget tracking; avoids a tokenizer dependency.
    pub fn estimate_tokens(text: &str) -> usize {
        text.chars().count() / 4
    }

    /// Index `content` into the RAG pipeline under `doc_id` for later
    /// retrieval via [`compress_to_budget`].
    pub async fn index_document(&self, doc_id: &str, content: &str) -> anyhow::Result<()> {
        self.rag
            .index(doc_id, None, None, content)
            .await
            .map(|_| ())
            .map_err(anyhow::Error::from)
    }

    /// Return `full_text` if it fits within `budget_tokens`. Otherwise use
    /// RAG to retrieve the most relevant chunks and truncate to fit.
    ///
    /// # Budget enforcement
    /// - If `estimate_tokens(full_text) <= budget_tokens` → return `full_text` unchanged.
    /// - Otherwise → `rag.search(query, 10, None)`, join `chunk.text` fields with `"\n\n"`,
    ///   then truncate the joined string to `budget_tokens * 4` characters.
    pub async fn compress_to_budget(
        &mut self,
        query: &str,
        full_text: &str,
        budget_tokens: usize,
    ) -> anyhow::Result<String> {
        if Self::estimate_tokens(full_text) <= budget_tokens {
            return Ok(full_text.to_string());
        }

        let hits: Vec<SearchHit> = self
            .rag
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

    /// Record that `tokens` were consumed by an agent call.
    /// Saturates at the maximum context limit (remaining never goes below 0).
    pub fn record_usage(&mut self, tokens: usize) {
        self.used_tokens = self.used_tokens.saturating_add(tokens);
        if self.used_tokens > self.max_context_tokens {
            self.used_tokens = self.max_context_tokens;
        }
    }

    /// Returns the number of tokens remaining in the budget.
    pub fn remaining(&self) -> usize {
        self.max_context_tokens.saturating_sub(self.used_tokens)
    }
}
```

- [ ] Run tests: `cargo test -p minion-presentation context_tests 2>&1`

  All tests in `context_tests` must pass. If `VectorStore` or `OllamaEmbedder` constructors differ from what's in the test helper, fix the test helper (`make_pipeline`). Do not change the `ContextManager` implementation logic.

---

## Task 3: Wire lib.rs + smoke test

### Step 3.1 — Add modules to lib.rs

- [ ] Edit `crates/minion-presentation/src/lib.rs` to add the two new modules:

```rust
pub mod context;
pub mod router;
```

  The file currently contains:
  ```
  pub mod db;
  pub mod migrations;
  pub mod schema;

  pub use schema::types::*;
  ```

  After editing it should be:
  ```rust
  pub mod context;
  pub mod db;
  pub mod migrations;
  pub mod router;
  pub mod schema;

  pub use schema::types::*;
  ```

  (Alphabetical order is conventional but not required — match the existing style.)

### Step 3.2 — Verify RouterConfig::default() compiles

- [ ] Run:

```bash
cargo test -p minion-presentation 2>&1
```

  All tests (router + context) must pass. Zero compilation errors.

- [ ] Run clippy to catch any obvious issues:

```bash
cargo clippy -p minion-presentation -- -D warnings 2>&1
```

  Fix any warnings before committing.

### Step 3.3 — Full workspace compile check

- [ ] Run:

```bash
cargo build --workspace 2>&1 | tail -20
```

  Confirm the workspace builds clean (no errors in any crate).

### Step 3.4 — Commit

- [ ] Commit with a conventional commit message:

```
feat(presentation): add LLM router and context manager (sub-plan 2b)

- PresentationRouter maps RoutingTask variants to Ollama or OpenAI
  providers using task-aware model selection (text vs. vision)
- ContextManager tracks token budget and compresses oversized inputs
  via RAG retrieval (chars/4 token estimate, saturating usage tracking)
- 6 router tests + 5 context tests, all passing
```

---

## Acceptance Criteria

Before marking this sub-plan complete, verify all of the following:

- [ ] `crates/minion-presentation/src/router.rs` exists and compiles
- [ ] `crates/minion-presentation/src/context.rs` exists and compiles
- [ ] `crates/minion-presentation/tests/router_tests.rs` exists with all tests passing
- [ ] `crates/minion-presentation/src/lib.rs` has `pub mod router; pub mod context;`
- [ ] `cargo test -p minion-presentation 2>&1` — all tests green
- [ ] `cargo clippy -p minion-presentation -- -D warnings 2>&1` — zero warnings
- [ ] `cargo build --workspace 2>&1` — workspace builds clean
- [ ] Conventional commit created

---

## Import Reference (verified against codebase)

| Symbol | Import path |
|---|---|
| `create_provider` | `minion_llm::create_provider` |
| `LlmProvider` | `minion_llm::LlmProvider` |
| `EndpointConfig` | `minion_llm::EndpointConfig` |
| `ProviderType` | `minion_llm::ProviderType` |
| `RagPipeline` | `minion_rag::RagPipeline` |
| `SearchHit` | `minion_rag::SearchHit` |

All symbols above are re-exported at crate root (confirmed in `minion-llm/src/lib.rs` line 16–22 and `minion-rag/src/lib.rs` line 24–26).
