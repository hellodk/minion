//! ContextManager — token-budget tracking and RAG-based context compression.
//!
//! Wraps a [`RagPipeline`] to provide two higher-level services:
//! 1. **Token budgeting**: track how many tokens have been consumed
//!    and report how many remain before the context window is exhausted.
//! 2. **Compression**: when raw text exceeds a given budget, retrieve
//!    the most relevant chunks from the indexed document and return a
//!    trimmed version that fits within the budget.

use std::sync::Arc;

use minion_rag::{RagPipeline, SearchHit};

pub struct ContextManager {
    rag: Option<Arc<RagPipeline>>,
    max_context_tokens: usize,
    used_tokens: usize,
}

impl ContextManager {
    const DEFAULT_MAX_TOKENS: usize = 8_000;

    pub fn new(rag: Arc<RagPipeline>) -> Self {
        Self {
            rag: Some(rag),
            max_context_tokens: Self::DEFAULT_MAX_TOKENS,
            used_tokens: 0,
        }
    }

    #[cfg(test)]
    pub fn new_for_testing() -> Self {
        Self { rag: None, max_context_tokens: Self::DEFAULT_MAX_TOKENS, used_tokens: 0 }
    }

    /// Rough estimate: 1 token ≈ 4 characters (the standard heuristic).
    pub fn estimate_tokens(text: &str) -> usize {
        text.chars().count() / 4
    }

    /// Index a document so it can later be used for compression queries.
    pub async fn index_document(&self, doc_id: &str, content: &str) -> anyhow::Result<()> {
        let rag = self.rag.as_ref().ok_or_else(|| anyhow::anyhow!("no RAG pipeline"))?;
        rag.index(doc_id, None, None, content)
            .await
            .map(|_| ())
            .map_err(anyhow::Error::from)
    }

    /// Return `full_text` unchanged if it fits inside `budget_tokens`.
    /// Otherwise retrieve the top relevant chunks from the RAG index and
    /// truncate the joined result to `budget_tokens`.
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
        let hits: Vec<SearchHit> =
            rag.search(query, 10, None).await.map_err(anyhow::Error::from)?;
        let joined = hits.into_iter().map(|h| h.chunk.text).collect::<Vec<_>>().join("\n\n");
        let char_budget = budget_tokens * 4;
        Ok(joined.chars().take(char_budget).collect())
    }

    /// Record that `tokens` tokens have been consumed from the budget.
    /// Saturates at `max_context_tokens` — never goes negative.
    pub fn record_usage(&mut self, tokens: usize) {
        self.used_tokens = self.used_tokens.saturating_add(tokens);
        if self.used_tokens > self.max_context_tokens {
            self.used_tokens = self.max_context_tokens;
        }
    }

    /// How many tokens remain before the context window is full.
    pub fn remaining(&self) -> usize {
        self.max_context_tokens.saturating_sub(self.used_tokens)
    }
}
