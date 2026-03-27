//! MINION Search Engine
//!
//! Full-text search using Tantivy.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{
    Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value, INDEXED, STORED,
    STRING,
};
use tantivy::{doc, Index, IndexWriter, ReloadPolicy, TantivyDocument};
use thiserror::Error;
use tracing::{debug, info};

#[derive(Error, Debug)]
pub enum Error {
    #[error("Index error: {0}")]
    Index(String),

    #[error("Search error: {0}")]
    Search(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Schema error: {0}")]
    Schema(String),

    #[error("Query error: {0}")]
    Query(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A document to be indexed in the search engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchDocument {
    /// Unique identifier for the document.
    pub id: String,
    /// Title of the document.
    pub title: String,
    /// Body/content of the document.
    pub body: String,
    /// Tags associated with the document.
    pub tags: Vec<String>,
    /// Source module the document came from (e.g. "files", "reader", "blog").
    pub source: String,
    /// Creation timestamp as a Unix epoch (seconds since 1970-01-01 UTC).
    pub created_at: i64,
}

/// A search result containing the matched document, relevance score, and optional snippet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The matched document.
    pub document: SearchDocument,
    /// Relevance score assigned by Tantivy.
    pub score: f32,
    /// An optional highlighted snippet from the body.
    pub snippet: Option<String>,
}

/// Holds the Tantivy field handles for convenience.
#[derive(Debug, Clone)]
struct SchemaFields {
    id: Field,
    title: Field,
    body: Field,
    tags: Field,
    source: Field,
    timestamp: Field,
}

/// The main search engine backed by Tantivy.
pub struct SearchIndex {
    index: Index,
    #[allow(dead_code)]
    schema: Schema,
    fields: SchemaFields,
}

impl SearchIndex {
    /// Build the shared Tantivy schema used by all `SearchIndex` instances.
    fn build_schema() -> (Schema, SchemaFields) {
        let mut schema_builder = Schema::builder();

        // id: stored + indexed as a single token (STRING)
        let id = schema_builder.add_text_field("id", STRING | STORED);

        // title: full-text indexed + stored
        let text_indexing = TextFieldIndexing::default()
            .set_tokenizer("default")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);
        let text_options = TextOptions::default()
            .set_indexing_options(text_indexing.clone())
            .set_stored();

        let title = schema_builder.add_text_field("title", text_options.clone());

        // body: full-text indexed + stored
        let body = schema_builder.add_text_field("body", text_options);

        // tags: full-text indexed + stored (joined as a space-separated string)
        let tag_indexing = TextFieldIndexing::default()
            .set_tokenizer("default")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);
        let tag_options = TextOptions::default()
            .set_indexing_options(tag_indexing)
            .set_stored();
        let tags = schema_builder.add_text_field("tags", tag_options);

        // source: stored + indexed as a single token (STRING)
        let source = schema_builder.add_text_field("source", STRING | STORED);

        // timestamp: i64, stored + indexed
        let timestamp = schema_builder.add_i64_field("timestamp", INDEXED | STORED);

        let schema = schema_builder.build();
        let fields = SchemaFields {
            id,
            title,
            body,
            tags,
            source,
            timestamp,
        };

        (schema, fields)
    }

    /// Create or open a Tantivy index stored at the given filesystem `path`.
    pub fn new(path: &Path) -> Result<Self> {
        let (schema, fields) = Self::build_schema();

        std::fs::create_dir_all(path)
            .map_err(|e| Error::Index(format!("Failed to create index directory: {e}")))?;

        let dir = tantivy::directory::MmapDirectory::open(path)
            .map_err(|e| Error::Index(format!("Failed to open mmap directory: {e}")))?;

        let index = Index::open_or_create(dir, schema.clone())
            .map_err(|e| Error::Index(format!("Failed to open or create index: {e}")))?;

        info!("Opened search index at {}", path.display());

        Ok(Self {
            index,
            schema,
            fields,
        })
    }

    /// Create an in-memory Tantivy index (useful for testing).
    pub fn new_in_memory() -> Result<Self> {
        let (schema, fields) = Self::build_schema();

        let index = Index::create_in_ram(schema.clone());

        debug!("Created in-memory search index");

        Ok(Self {
            index,
            schema,
            fields,
        })
    }

    /// Obtain an `IndexWriter` with a reasonable default heap budget.
    fn writer(&self) -> Result<IndexWriter> {
        self.index
            .writer(50_000_000) // 50 MB heap
            .map_err(|e| Error::Index(format!("Failed to create index writer: {e}")))
    }

    /// Add a single document to the index.
    pub fn add_document(&self, document: &SearchDocument) -> Result<()> {
        let mut writer = self.writer()?;

        let tags_joined = document.tags.join(" ");

        writer
            .add_document(doc!(
                self.fields.id => document.id.as_str(),
                self.fields.title => document.title.as_str(),
                self.fields.body => document.body.as_str(),
                self.fields.tags => tags_joined.as_str(),
                self.fields.source => document.source.as_str(),
                self.fields.timestamp => document.created_at
            ))
            .map_err(|e| Error::Index(format!("Failed to add document: {e}")))?;

        writer
            .commit()
            .map_err(|e| Error::Index(format!("Failed to commit: {e}")))?;

        debug!(id = %document.id, "Indexed document");
        Ok(())
    }

    /// Add multiple documents in a single commit.
    pub fn add_documents(&self, docs: &[SearchDocument]) -> Result<()> {
        let mut writer = self.writer()?;

        for document in docs {
            let tags_joined = document.tags.join(" ");
            writer
                .add_document(doc!(
                    self.fields.id => document.id.as_str(),
                    self.fields.title => document.title.as_str(),
                    self.fields.body => document.body.as_str(),
                    self.fields.tags => tags_joined.as_str(),
                    self.fields.source => document.source.as_str(),
                    self.fields.timestamp => document.created_at
                ))
                .map_err(|e| Error::Index(format!("Failed to add document: {e}")))?;
        }

        writer
            .commit()
            .map_err(|e| Error::Index(format!("Failed to commit: {e}")))?;

        debug!(count = docs.len(), "Indexed batch of documents");
        Ok(())
    }

    /// Search the index for documents matching `query_str`, returning up to `limit` results.
    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| Error::Search(format!("Failed to create reader: {e}")))?;

        let searcher = reader.searcher();

        let query_parser = QueryParser::for_index(
            &self.index,
            vec![self.fields.title, self.fields.body, self.fields.tags],
        );

        let query = query_parser
            .parse_query(query_str)
            .map_err(|e| Error::Query(format!("Failed to parse query: {e}")))?;

        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(limit))
            .map_err(|e| Error::Search(format!("Search execution failed: {e}")))?;

        // Build a snippet generator for the body field.
        let snippet_generator =
            tantivy::SnippetGenerator::create(&searcher, &query, self.fields.body)
                .map_err(|e| Error::Search(format!("Failed to create snippet generator: {e}")))?;

        let mut results = Vec::with_capacity(top_docs.len());

        for (score, doc_address) in top_docs {
            let retrieved: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| Error::Search(format!("Failed to retrieve document: {e}")))?;

            let id = retrieved
                .get_first(self.fields.id)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let title = retrieved
                .get_first(self.fields.title)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let body = retrieved
                .get_first(self.fields.body)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let tags_raw = retrieved
                .get_first(self.fields.tags)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let tags: Vec<String> = if tags_raw.is_empty() {
                Vec::new()
            } else {
                tags_raw.split_whitespace().map(String::from).collect()
            };

            let source = retrieved
                .get_first(self.fields.source)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let created_at = retrieved
                .get_first(self.fields.timestamp)
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            let snippet_obj = snippet_generator.snippet_from_doc(&retrieved);
            let snippet_html = snippet_obj.to_html();
            let snippet = if snippet_html.is_empty() {
                None
            } else {
                Some(snippet_html)
            };

            results.push(SearchResult {
                document: SearchDocument {
                    id,
                    title,
                    body,
                    tags,
                    source,
                    created_at,
                },
                score,
                snippet,
            });
        }

        debug!(query = %query_str, hits = results.len(), "Search completed");
        Ok(results)
    }

    /// Delete all documents with the given `id` from the index.
    pub fn delete_document(&self, id: &str) -> Result<()> {
        let mut writer = self.writer()?;

        let term = tantivy::Term::from_field_text(self.fields.id, id);
        writer.delete_term(term);

        writer
            .commit()
            .map_err(|e| Error::Index(format!("Failed to commit delete: {e}")))?;

        debug!(id = %id, "Deleted document from index");
        Ok(())
    }

    /// Return the total number of documents currently in the index.
    pub fn document_count(&self) -> Result<u64> {
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| Error::Search(format!("Failed to create reader: {e}")))?;

        let searcher = reader.searcher();
        let count = searcher.num_docs();

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_index() {
        let err = Error::Index("test index error".to_string());
        assert!(err.to_string().contains("Index error"));
        assert!(err.to_string().contains("test index error"));
    }

    #[test]
    fn test_error_search() {
        let err = Error::Search("test search error".to_string());
        assert!(err.to_string().contains("Search error"));
    }

    #[test]
    fn test_result_type() {
        let ok_result: Result<i32> = Ok(42);
        assert_eq!(ok_result.unwrap(), 42);

        let err_result: Result<i32> = Err(Error::Index("test".to_string()));
        assert!(err_result.is_err());
    }

    #[test]
    fn test_error_debug() {
        let err = Error::Index("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Index"));
    }

    #[test]
    fn test_error_schema() {
        let err = Error::Schema("bad schema".to_string());
        assert!(err.to_string().contains("Schema error"));
        assert!(err.to_string().contains("bad schema"));
    }

    #[test]
    fn test_error_query() {
        let err = Error::Query("bad query".to_string());
        assert!(err.to_string().contains("Query error"));
        assert!(err.to_string().contains("bad query"));
    }

    fn sample_doc(
        id: &str,
        title: &str,
        body: &str,
        tags: Vec<&str>,
        source: &str,
    ) -> SearchDocument {
        SearchDocument {
            id: id.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            tags: tags.into_iter().map(String::from).collect(),
            source: source.to_string(),
            created_at: 1700000000,
        }
    }

    #[test]
    fn test_create_in_memory_index() {
        let index = SearchIndex::new_in_memory();
        assert!(index.is_ok());
        let index = index.unwrap();
        let count = index.document_count().unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_add_and_search_document() {
        let index = SearchIndex::new_in_memory().unwrap();
        let doc = sample_doc(
            "doc-1",
            "Rust Programming",
            "Rust is a systems programming language focused on safety and performance.",
            vec!["rust", "programming"],
            "blog",
        );

        index.add_document(&doc).unwrap();

        let results = index.search("rust", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].document.id, "doc-1");
        assert_eq!(results[0].document.title, "Rust Programming");
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_search_multiple_documents() {
        let index = SearchIndex::new_in_memory().unwrap();

        let docs = vec![
            sample_doc(
                "doc-1",
                "Introduction to Rust",
                "Rust is a modern systems language.",
                vec!["rust"],
                "blog",
            ),
            sample_doc(
                "doc-2",
                "Python Basics",
                "Python is an interpreted language popular for data science.",
                vec!["python"],
                "blog",
            ),
            sample_doc(
                "doc-3",
                "Advanced Rust Patterns",
                "Learn advanced patterns in Rust including lifetimes and traits.",
                vec!["rust", "advanced"],
                "blog",
            ),
        ];

        index.add_documents(&docs).unwrap();

        let results = index.search("rust", 10).unwrap();
        assert!(results.len() >= 2, "Expected at least 2 results for 'rust'");

        // All returned documents should be about Rust
        for r in &results {
            let text = format!(
                "{} {} {}",
                r.document.title.to_lowercase(),
                r.document.body.to_lowercase(),
                r.document.tags.join(" ").to_lowercase(),
            );
            assert!(
                text.contains("rust"),
                "Result should be related to rust: {}",
                text
            );
        }

        // Search for python should return only 1
        let py_results = index.search("python", 10).unwrap();
        assert_eq!(py_results.len(), 1);
        assert_eq!(py_results[0].document.id, "doc-2");
    }

    #[test]
    fn test_delete_document() {
        let index = SearchIndex::new_in_memory().unwrap();

        let doc = sample_doc(
            "doc-del",
            "Delete Me",
            "This document will be deleted from the index.",
            vec!["delete"],
            "test",
        );

        index.add_document(&doc).unwrap();
        assert_eq!(index.document_count().unwrap(), 1);

        index.delete_document("doc-del").unwrap();
        assert_eq!(index.document_count().unwrap(), 0);

        let results = index.search("deleted", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_document_count() {
        let index = SearchIndex::new_in_memory().unwrap();
        assert_eq!(index.document_count().unwrap(), 0);

        let docs: Vec<SearchDocument> = (0..5)
            .map(|i| {
                sample_doc(
                    &format!("doc-{i}"),
                    &format!("Document {i}"),
                    &format!("Body of document number {i}"),
                    vec!["test"],
                    "test",
                )
            })
            .collect();

        index.add_documents(&docs).unwrap();
        assert_eq!(index.document_count().unwrap(), 5);

        index.delete_document("doc-0").unwrap();
        assert_eq!(index.document_count().unwrap(), 4);
    }

    #[test]
    fn test_search_by_tag() {
        let index = SearchIndex::new_in_memory().unwrap();

        let docs = vec![
            sample_doc(
                "t-1",
                "Alpha Article",
                "General content about nothing specific.",
                vec!["finance", "investing"],
                "blog",
            ),
            sample_doc(
                "t-2",
                "Beta Article",
                "Another general article with different content.",
                vec!["fitness", "health"],
                "blog",
            ),
        ];

        index.add_documents(&docs).unwrap();

        let results = index.search("finance", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].document.id, "t-1");

        let results = index.search("fitness", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].document.id, "t-2");
    }

    #[test]
    fn test_search_no_results() {
        let index = SearchIndex::new_in_memory().unwrap();

        let doc = sample_doc(
            "nr-1",
            "Something Else",
            "Nothing related to the query at all.",
            vec!["misc"],
            "files",
        );

        index.add_document(&doc).unwrap();

        let results = index.search("xylophone", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_relevance_ordering() {
        let index = SearchIndex::new_in_memory().unwrap();

        let docs = vec![
            sample_doc(
                "rel-1",
                "Cooking Recipes",
                "A collection of cooking recipes from around the world.",
                vec!["cooking"],
                "blog",
            ),
            sample_doc(
                "rel-2",
                "Cooking Tips and Tricks for Cooking Enthusiasts",
                "Cooking is an art. Here are cooking tips, cooking techniques, and cooking secrets.",
                vec!["cooking", "tips"],
                "blog",
            ),
            sample_doc(
                "rel-3",
                "Travel Guide",
                "Explore amazing destinations with no relation to the query term at all.",
                vec!["travel"],
                "blog",
            ),
        ];

        index.add_documents(&docs).unwrap();

        let results = index.search("cooking", 10).unwrap();
        assert!(results.len() >= 2, "Expected at least 2 results");

        // Scores should be in descending order.
        for window in results.windows(2) {
            assert!(
                window[0].score >= window[1].score,
                "Results should be ordered by descending score: {} >= {}",
                window[0].score,
                window[1].score,
            );
        }

        // The travel document (rel-3) should not appear since it doesn't mention cooking.
        let ids: Vec<&str> = results.iter().map(|r| r.document.id.as_str()).collect();
        assert!(
            !ids.contains(&"rel-3"),
            "Irrelevant document should not appear in results"
        );

        // Both cooking-related docs should appear.
        assert!(ids.contains(&"rel-1"), "rel-1 should appear");
        assert!(ids.contains(&"rel-2"), "rel-2 should appear");
    }
}
