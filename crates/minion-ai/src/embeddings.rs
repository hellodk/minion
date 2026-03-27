//! Embedding generation and vector operations

use crate::ollama::OllamaClient;
use crate::{AIConfig, Result};

/// Embedding generator
pub struct EmbeddingGenerator {
    client: OllamaClient,
    model: String,
    dimension: usize,
}

impl EmbeddingGenerator {
    /// Create a new embedding generator
    pub fn new(config: &AIConfig) -> Self {
        Self {
            client: OllamaClient::new(config),
            model: config.embedding_model.clone(),
            dimension: 768, // Default for nomic-embed-text
        }
    }

    /// Generate embedding for a single text
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.client.embed(&self.model, text).await
    }

    /// Generate embeddings for multiple texts
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());

        for text in texts {
            let embedding = self.embed(text).await?;
            results.push(embedding);
        }

        Ok(results)
    }

    /// Get the embedding dimension
    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

/// Calculate cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Calculate euclidean distance between two vectors
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c)).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_scaled() {
        // Same direction, different magnitudes should still have similarity 1.0
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![2.0, 4.0, 6.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![1.0, 2.0, 3.0];
        let zero = vec![0.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &zero), 0.0);
        assert_eq!(cosine_similarity(&zero, &a), 0.0);
        assert_eq!(cosine_similarity(&zero, &zero), 0.0);
    }

    #[test]
    fn test_euclidean_distance_same_point() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        assert!((euclidean_distance(&a, &b)).abs() < 0.001);
    }

    #[test]
    fn test_euclidean_distance_unit_distance() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((euclidean_distance(&a, &b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_euclidean_distance_3d() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![3.0, 4.0, 0.0];
        assert!((euclidean_distance(&a, &b) - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_euclidean_distance_symmetry() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        assert!((euclidean_distance(&a, &b) - euclidean_distance(&b, &a)).abs() < 0.001);
    }

    #[test]
    fn test_embedding_generator_dimension() {
        let config = AIConfig::default();
        let generator = EmbeddingGenerator::new(&config);
        assert_eq!(generator.dimension(), 768);
    }

    #[test]
    fn test_cosine_similarity_high_dimensional() {
        // Test with higher dimensional vectors
        let a: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..100).map(|i| i as f32).collect();

        // Same vectors should have similarity 1.0
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_partial_overlap() {
        let a = vec![1.0, 1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 1.0, 0.0];

        // Partial overlap - should be between 0 and 1
        let sim = cosine_similarity(&a, &b);
        assert!(sim > 0.0 && sim < 1.0);
        // Expected: 1/sqrt(2)*sqrt(2) = 0.5
        assert!((sim - 0.5).abs() < 0.001);
    }
}
