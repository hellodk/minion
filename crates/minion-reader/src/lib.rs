//! MINION Book Reader Module
//!
//! Premium reading experience with AI-powered features.

pub mod annotations;
pub mod formats;
pub mod knowledge;
pub mod library;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Library error: {0}")]
    Library(String),

    #[error("Format error: {0}")]
    Format(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] minion_db::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Supported book formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookFormat {
    Epub,
    Pdf,
    Mobi,
    Azw,
    Markdown,
    Html,
    Txt,
}

impl BookFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "epub" => Some(Self::Epub),
            "pdf" => Some(Self::Pdf),
            "mobi" => Some(Self::Mobi),
            "azw" | "azw3" => Some(Self::Azw),
            "md" | "markdown" => Some(Self::Markdown),
            "html" | "htm" => Some(Self::Html),
            "txt" => Some(Self::Txt),
            _ => None,
        }
    }
}

/// Book metadata
#[derive(Debug, Clone)]
pub struct BookMetadata {
    pub title: String,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub publisher: Option<String>,
    pub publish_date: Option<String>,
    pub isbn: Option<String>,
    pub language: Option<String>,
    pub description: Option<String>,
}

/// Reading position
#[derive(Debug, Clone)]
pub struct ReadingPosition {
    pub chapter_index: usize,
    pub position: String, // Format-specific position
    pub progress_percent: f32,
}

/// Annotation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationType {
    Highlight,
    Note,
    Bookmark,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_book_format_from_extension() {
        assert_eq!(BookFormat::from_extension("epub"), Some(BookFormat::Epub));
        assert_eq!(BookFormat::from_extension("EPUB"), Some(BookFormat::Epub));
        assert_eq!(BookFormat::from_extension("pdf"), Some(BookFormat::Pdf));
        assert_eq!(BookFormat::from_extension("mobi"), Some(BookFormat::Mobi));
        assert_eq!(BookFormat::from_extension("azw"), Some(BookFormat::Azw));
        assert_eq!(BookFormat::from_extension("azw3"), Some(BookFormat::Azw));
        assert_eq!(BookFormat::from_extension("md"), Some(BookFormat::Markdown));
        assert_eq!(
            BookFormat::from_extension("markdown"),
            Some(BookFormat::Markdown)
        );
        assert_eq!(BookFormat::from_extension("html"), Some(BookFormat::Html));
        assert_eq!(BookFormat::from_extension("htm"), Some(BookFormat::Html));
        assert_eq!(BookFormat::from_extension("txt"), Some(BookFormat::Txt));
        assert_eq!(BookFormat::from_extension("unknown"), None);
        assert_eq!(BookFormat::from_extension(""), None);
    }

    #[test]
    fn test_book_format_equality() {
        assert_eq!(BookFormat::Epub, BookFormat::Epub);
        assert_ne!(BookFormat::Epub, BookFormat::Pdf);
    }

    #[test]
    fn test_annotation_type_equality() {
        assert_eq!(AnnotationType::Highlight, AnnotationType::Highlight);
        assert_ne!(AnnotationType::Highlight, AnnotationType::Note);
        assert_ne!(AnnotationType::Note, AnnotationType::Bookmark);
    }

    #[test]
    fn test_book_metadata() {
        let metadata = BookMetadata {
            title: "Test Book".to_string(),
            subtitle: Some("A Subtitle".to_string()),
            authors: vec!["Author One".to_string(), "Author Two".to_string()],
            publisher: Some("Test Publisher".to_string()),
            publish_date: Some("2024".to_string()),
            isbn: Some("978-1234567890".to_string()),
            language: Some("en".to_string()),
            description: Some("A test book description.".to_string()),
        };

        assert_eq!(metadata.title, "Test Book");
        assert_eq!(metadata.authors.len(), 2);
        assert!(metadata.isbn.is_some());
    }

    #[test]
    fn test_book_metadata_minimal() {
        let metadata = BookMetadata {
            title: "Minimal Book".to_string(),
            subtitle: None,
            authors: vec![],
            publisher: None,
            publish_date: None,
            isbn: None,
            language: None,
            description: None,
        };

        assert_eq!(metadata.title, "Minimal Book");
        assert!(metadata.authors.is_empty());
        assert!(metadata.publisher.is_none());
    }

    #[test]
    fn test_book_metadata_clone() {
        let original = BookMetadata {
            title: "Test".to_string(),
            subtitle: None,
            authors: vec!["Author".to_string()],
            publisher: None,
            publish_date: None,
            isbn: None,
            language: None,
            description: None,
        };

        let cloned = original.clone();
        assert_eq!(cloned.title, original.title);
        assert_eq!(cloned.authors, original.authors);
    }

    #[test]
    fn test_reading_position() {
        let position = ReadingPosition {
            chapter_index: 5,
            position: "epub-cfi:123".to_string(),
            progress_percent: 42.5,
        };

        assert_eq!(position.chapter_index, 5);
        assert_eq!(position.position, "epub-cfi:123");
        assert!((position.progress_percent - 42.5).abs() < 0.001);
    }

    #[test]
    fn test_reading_position_clone() {
        let original = ReadingPosition {
            chapter_index: 1,
            position: "100".to_string(),
            progress_percent: 10.0,
        };

        let cloned = original.clone();
        assert_eq!(cloned.chapter_index, original.chapter_index);
        assert_eq!(cloned.position, original.position);
    }

    #[test]
    fn test_error_variants() {
        let library_err = Error::Library("test".to_string());
        assert!(library_err.to_string().contains("Library error"));

        let format_err = Error::Format("test".to_string());
        assert!(format_err.to_string().contains("Format error"));

        let parse_err = Error::Parse("test".to_string());
        assert!(parse_err.to_string().contains("Parse error"));
    }

    #[test]
    fn test_result_type() {
        let ok_result: Result<i32> = Ok(42);
        assert_eq!(ok_result.unwrap(), 42);

        let err_result: Result<i32> = Err(Error::Format("test".to_string()));
        assert!(err_result.is_err());
    }
}
