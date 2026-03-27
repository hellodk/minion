//! Knowledge base integration for books

// Knowledge base types

/// A chunk of book content for embedding
#[derive(Debug, Clone)]
pub struct BookChunk {
    pub id: String,
    pub book_id: String,
    pub chapter_index: usize,
    pub content: String,
    pub start_pos: usize,
    pub end_pos: usize,
}

/// Knowledge base for semantic search across books
pub struct BookKnowledgeBase {
    chunks: Vec<BookChunk>,
}

impl BookKnowledgeBase {
    pub fn new() -> Self {
        Self { chunks: Vec::new() }
    }

    /// Add chunks from a book
    pub fn add_book_chunks(&mut self, book_id: &str, content: &str, chapter_index: usize) {
        let chunk_size = 500; // characters per chunk
        let overlap = 50;

        let mut start = 0;
        while start < content.len() {
            let end = (start + chunk_size).min(content.len());
            let chunk_content = &content[start..end];

            self.chunks.push(BookChunk {
                id: uuid::Uuid::new_v4().to_string(),
                book_id: book_id.to_string(),
                chapter_index,
                content: chunk_content.to_string(),
                start_pos: start,
                end_pos: end,
            });

            start += chunk_size - overlap;
        }
    }

    /// Get all chunks for a book
    pub fn chunks_for_book(&self, book_id: &str) -> Vec<&BookChunk> {
        self.chunks
            .iter()
            .filter(|c| c.book_id == book_id)
            .collect()
    }

    /// Get total chunk count
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

impl Default for BookKnowledgeBase {
    fn default() -> Self {
        Self::new()
    }
}
