//! Book library management

use crate::{BookFormat, BookMetadata, Error, Result};
use std::path::{Path, PathBuf};

/// A book in the library
#[derive(Debug, Clone)]
pub struct Book {
    /// Unique identifier
    pub id: String,

    /// File path
    pub file_path: PathBuf,

    /// File format
    pub format: BookFormat,

    /// Book metadata
    pub metadata: BookMetadata,

    /// Cover image path
    pub cover_path: Option<PathBuf>,

    /// Total pages/locations
    pub total_pages: Option<u32>,

    /// Date added to library
    pub date_added: chrono::DateTime<chrono::Utc>,

    /// Last opened date
    pub last_opened: Option<chrono::DateTime<chrono::Utc>>,

    /// User rating (1-5)
    pub rating: Option<u8>,

    /// Favorite flag
    pub is_favorite: bool,

    /// Tags
    pub tags: Vec<String>,
}

/// Library manager
pub struct Library {
    #[allow(dead_code)]
    root_path: PathBuf,
    books: Vec<Book>,
}

impl Library {
    /// Create a new library
    pub fn new(root_path: &Path) -> Result<Self> {
        std::fs::create_dir_all(root_path)?;

        Ok(Self {
            root_path: root_path.to_path_buf(),
            books: Vec::new(),
        })
    }

    /// Import a book file
    pub fn import(&mut self, path: &Path) -> Result<&Book> {
        let format = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(BookFormat::from_extension)
            .ok_or_else(|| Error::Format("Unsupported format".to_string()))?;

        let metadata = extract_metadata(path, format)?;

        let book = Book {
            id: uuid::Uuid::new_v4().to_string(),
            file_path: path.to_path_buf(),
            format,
            metadata,
            cover_path: None,
            total_pages: None,
            date_added: chrono::Utc::now(),
            last_opened: None,
            rating: None,
            is_favorite: false,
            tags: Vec::new(),
        };

        self.books.push(book);
        Ok(self.books.last().unwrap())
    }

    /// List all books
    pub fn list(&self) -> &[Book] {
        &self.books
    }

    /// Find a book by ID
    pub fn get(&self, id: &str) -> Option<&Book> {
        self.books.iter().find(|b| b.id == id)
    }

    /// Remove a book from library
    pub fn remove(&mut self, id: &str) -> bool {
        if let Some(pos) = self.books.iter().position(|b| b.id == id) {
            self.books.remove(pos);
            true
        } else {
            false
        }
    }
}

/// Extract metadata from a book file
fn extract_metadata(path: &Path, format: BookFormat) -> Result<BookMetadata> {
    match format {
        BookFormat::Epub => extract_epub_metadata(path),
        _ => {
            // Fallback to filename-based metadata
            let filename = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            Ok(BookMetadata {
                title: filename,
                subtitle: None,
                authors: vec![],
                publisher: None,
                publish_date: None,
                isbn: None,
                language: None,
                description: None,
            })
        }
    }
}

/// Extract metadata from EPUB file
fn extract_epub_metadata(path: &Path) -> Result<BookMetadata> {
    let doc = epub::doc::EpubDoc::new(path).map_err(|e| Error::Parse(e.to_string()))?;

    // Helper to convert metadata to string
    let get_str = |name: &str| -> Option<String> { doc.mdata(name).map(|m| m.value.clone()) };

    let title = get_str("title").unwrap_or_default();
    let authors = get_str("creator").map(|a| vec![a]).unwrap_or_default();

    Ok(BookMetadata {
        title,
        subtitle: None,
        authors,
        publisher: get_str("publisher"),
        publish_date: get_str("date"),
        isbn: get_str("identifier"),
        language: get_str("language"),
        description: get_str("description"),
    })
}
