//! Embedding providers.
//!
//! Only Ollama is wired today. The trait is deliberately narrow so we
//! can plug in llama-server, OpenAI, or a local ONNX model later
//! without touching the chunker or store.

use crate::error::{RagError, RagResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn name(&self) -> &str;
    /// Expected dimension of the returned vectors. Used by the store
    /// to catch misconfiguration at insert time.
    fn dimension(&self) -> usize;
    async fn embed(&self, text: &str) -> RagResult<Vec<f32>>;
    /// Default implementation calls `embed` serially. Providers that
    /// support real batch endpoints can override.
    async fn embed_batch(&self, texts: &[String]) -> RagResult<Vec<Vec<f32>>> {
        let mut out = Vec::with_capacity(texts.len());
        for t in texts {
            out.push(self.embed(t).await?);
        }
        Ok(out)
    }
}

// =====================================================================
// Ollama
// =====================================================================

pub struct OllamaEmbedder {
    base_url: String,
    model: String,
    dimension: usize,
    client: reqwest::Client,
}

impl OllamaEmbedder {
    /// `base_url` defaults to `http://localhost:11434` in most installs.
    /// `dimension` must match the chosen embedding model (e.g. 768 for
    /// `nomic-embed-text`, 384 for `all-minilm`, 1024 for
    /// `mxbai-embed-large`).
    pub fn new(base_url: impl Into<String>, model: impl Into<String>, dimension: usize) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            dimension,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest build"),
        }
    }
}

#[derive(Serialize)]
struct EmbedReq<'a> {
    model: &'a str,
    prompt: &'a str,
}

#[derive(Deserialize)]
struct EmbedResp {
    embedding: Vec<f32>,
}

#[async_trait]
impl EmbeddingProvider for OllamaEmbedder {
    fn name(&self) -> &str {
        "ollama"
    }
    fn dimension(&self) -> usize {
        self.dimension
    }
    async fn embed(&self, text: &str) -> RagResult<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(&EmbedReq {
                model: &self.model,
                prompt: text,
            })
            .send()
            .await?;
        if !resp.status().is_success() {
            let s = resp.status();
            let b = resp.text().await.unwrap_or_default();
            return Err(RagError::Embedding(format!("ollama {} returned {}: {}", self.model, s, b)));
        }
        let v: EmbedResp = resp.json().await?;
        if v.embedding.len() != self.dimension {
            return Err(RagError::DimensionMismatch {
                expected: self.dimension,
                got: v.embedding.len(),
            });
        }
        Ok(v.embedding)
    }
}

/// Normalize a vector to unit length so cosine similarity reduces to
/// dot product. Returns the input unchanged if its magnitude is zero.
pub fn normalize(v: &mut [f32]) {
    let mag_sq: f32 = v.iter().map(|x| x * x).sum();
    if mag_sq <= f32::EPSILON {
        return;
    }
    let mag = mag_sq.sqrt();
    for x in v.iter_mut() {
        *x /= mag;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_unit_vector_unchanged() {
        let mut v = vec![1.0, 0.0, 0.0];
        normalize(&mut v);
        assert!((v[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn normalize_handles_zero() {
        let mut v = vec![0.0, 0.0, 0.0];
        normalize(&mut v); // must not panic
        assert_eq!(v, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn normalize_scales_to_unit() {
        let mut v = vec![3.0, 4.0];
        normalize(&mut v);
        let len: f32 = (v[0] * v[0] + v[1] * v[1]).sqrt();
        assert!((len - 1.0).abs() < 1e-6);
    }
}
