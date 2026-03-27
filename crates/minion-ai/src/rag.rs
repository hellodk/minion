//! Retrieval Augmented Generation (RAG) pipeline

use crate::embeddings::{cosine_similarity, EmbeddingGenerator};
use crate::ollama::OllamaClient;
use crate::{AIConfig, Result};

/// RAG configuration
#[derive(Debug, Clone)]
pub struct RAGConfig {
    /// Number of chunks to retrieve
    pub top_k: usize,
    /// Minimum similarity threshold
    pub min_similarity: f32,
    /// Maximum context length
    pub max_context_length: usize,
}

impl Default for RAGConfig {
    fn default() -> Self {
        Self {
            top_k: 5,
            min_similarity: 0.5,
            max_context_length: 4000,
        }
    }
}

/// A document chunk with embedding
#[derive(Debug, Clone)]
pub struct Chunk {
    pub id: String,
    pub content: String,
    pub embedding: Vec<f32>,
    pub metadata: std::collections::HashMap<String, String>,
}

/// RAG search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk: Chunk,
    pub score: f32,
}

/// RAG answer
#[derive(Debug)]
pub struct RAGAnswer {
    pub answer: String,
    pub sources: Vec<String>,
    pub confidence: f32,
}

/// Simple in-memory RAG pipeline
pub struct RAGPipeline {
    embedder: EmbeddingGenerator,
    llm: OllamaClient,
    chunks: Vec<Chunk>,
    config: RAGConfig,
}

impl RAGPipeline {
    /// Create a new RAG pipeline
    pub fn new(ai_config: &AIConfig, rag_config: RAGConfig) -> Self {
        Self {
            embedder: EmbeddingGenerator::new(ai_config),
            llm: OllamaClient::new(ai_config),
            chunks: Vec::new(),
            config: rag_config,
        }
    }

    /// Add a chunk to the index
    pub async fn add_chunk(
        &mut self,
        id: &str,
        content: &str,
        metadata: std::collections::HashMap<String, String>,
    ) -> Result<()> {
        let embedding = self.embedder.embed(content).await?;

        self.chunks.push(Chunk {
            id: id.to_string(),
            content: content.to_string(),
            embedding,
            metadata,
        });

        Ok(())
    }

    /// Search for similar chunks
    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let query_embedding = self.embedder.embed(query).await?;

        let mut results: Vec<SearchResult> = self
            .chunks
            .iter()
            .map(|chunk| {
                let score = cosine_similarity(&query_embedding, &chunk.embedding);
                SearchResult {
                    chunk: chunk.clone(),
                    score,
                }
            })
            .filter(|r| r.score >= self.config.min_similarity)
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        results.truncate(self.config.top_k);

        Ok(results)
    }

    /// Answer a question using RAG
    pub async fn answer(&self, question: &str) -> Result<RAGAnswer> {
        // Search for relevant chunks
        let results = self.search(question).await?;

        if results.is_empty() {
            return Ok(RAGAnswer {
                answer: "I don't have enough information to answer this question.".to_string(),
                sources: vec![],
                confidence: 0.0,
            });
        }

        // Build context
        let context: String = results
            .iter()
            .map(|r| format!("Source: {}\n{}\n", r.chunk.id, r.chunk.content))
            .collect::<Vec<_>>()
            .join("\n---\n");

        // Generate answer
        let prompt = format!(
            "Use the following context to answer the question. If the answer is not in the context, say so.\n\nCONTEXT:\n{}\n\nQUESTION: {}\n\nANSWER:",
            context, question
        );

        let answer = self.llm.complete(&prompt).await?;

        // Calculate confidence
        let avg_score: f32 = results.iter().map(|r| r.score).sum::<f32>() / results.len() as f32;

        Ok(RAGAnswer {
            answer,
            sources: results.into_iter().map(|r| r.chunk.id).collect(),
            confidence: avg_score,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rag_config_default() {
        let config = RAGConfig::default();

        assert_eq!(config.top_k, 5);
        assert!((config.min_similarity - 0.5).abs() < 0.001);
        assert_eq!(config.max_context_length, 4000);
    }

    #[test]
    fn test_chunk_creation() {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("source".to_string(), "test.txt".to_string());

        let chunk = Chunk {
            id: "chunk_1".to_string(),
            content: "This is test content".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
            metadata,
        };

        assert_eq!(chunk.id, "chunk_1");
        assert_eq!(chunk.content, "This is test content");
        assert_eq!(chunk.embedding.len(), 3);
        assert_eq!(chunk.metadata.get("source").unwrap(), "test.txt");
    }

    #[test]
    fn test_search_result() {
        let chunk = Chunk {
            id: "test".to_string(),
            content: "test".to_string(),
            embedding: vec![0.1],
            metadata: std::collections::HashMap::new(),
        };

        let result = SearchResult {
            chunk: chunk.clone(),
            score: 0.95,
        };

        assert_eq!(result.chunk.id, "test");
        assert!((result.score - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_rag_answer() {
        let answer = RAGAnswer {
            answer: "The answer is 42.".to_string(),
            sources: vec!["doc1".to_string(), "doc2".to_string()],
            confidence: 0.85,
        };

        assert_eq!(answer.answer, "The answer is 42.");
        assert_eq!(answer.sources.len(), 2);
        assert!((answer.confidence - 0.85).abs() < 0.001);
    }

    #[test]
    fn test_rag_pipeline_creation() {
        let ai_config = AIConfig::default();
        let rag_config = RAGConfig::default();

        let pipeline = RAGPipeline::new(&ai_config, rag_config);

        // Pipeline should be created without chunks
        assert!(pipeline.chunks.is_empty());
    }

    #[test]
    fn test_rag_config_custom() {
        let config = RAGConfig {
            top_k: 10,
            min_similarity: 0.7,
            max_context_length: 8000,
        };

        assert_eq!(config.top_k, 10);
        assert!((config.min_similarity - 0.7).abs() < 0.001);
        assert_eq!(config.max_context_length, 8000);
    }

    #[test]
    fn test_chunk_clone() {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("key".to_string(), "value".to_string());

        let chunk = Chunk {
            id: "original".to_string(),
            content: "content".to_string(),
            embedding: vec![1.0, 2.0, 3.0],
            metadata,
        };

        let cloned = chunk.clone();

        assert_eq!(cloned.id, chunk.id);
        assert_eq!(cloned.content, chunk.content);
        assert_eq!(cloned.embedding, chunk.embedding);
        assert_eq!(cloned.metadata, chunk.metadata);
    }

    #[test]
    fn test_search_result_clone() {
        let chunk = Chunk {
            id: "test".to_string(),
            content: "test".to_string(),
            embedding: vec![0.1],
            metadata: std::collections::HashMap::new(),
        };

        let result = SearchResult { chunk, score: 0.9 };

        let cloned = result.clone();

        assert_eq!(cloned.chunk.id, result.chunk.id);
        assert_eq!(cloned.score, result.score);
    }

    // Integration tests - require Ollama

    #[tokio::test]
    #[ignore = "requires running Ollama instance with embedding model"]
    async fn test_rag_add_chunk() {
        let ai_config = AIConfig::default();
        let rag_config = RAGConfig::default();

        let mut pipeline = RAGPipeline::new(&ai_config, rag_config);

        let result = pipeline
            .add_chunk(
                "doc1",
                "Rust is a systems programming language.",
                std::collections::HashMap::new(),
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(pipeline.chunks.len(), 1);
    }

    #[tokio::test]
    #[ignore = "requires running Ollama instance with embedding model"]
    async fn test_rag_search() {
        let ai_config = AIConfig::default();
        let rag_config = RAGConfig {
            top_k: 2,
            min_similarity: 0.0, // Accept all for testing
            max_context_length: 4000,
        };

        let mut pipeline = RAGPipeline::new(&ai_config, rag_config);

        // Add some chunks
        pipeline
            .add_chunk(
                "rust",
                "Rust is a systems programming language.",
                std::collections::HashMap::new(),
            )
            .await
            .unwrap();
        pipeline
            .add_chunk(
                "python",
                "Python is a dynamic programming language.",
                std::collections::HashMap::new(),
            )
            .await
            .unwrap();

        let results = pipeline.search("What is Rust?").await;

        assert!(results.is_ok());
        let results = results.unwrap();
        assert!(!results.is_empty());
    }
}
