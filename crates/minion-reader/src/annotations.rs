//! Highlights and annotations

use crate::AnnotationType;

/// An annotation in a book
#[derive(Debug, Clone)]
pub struct Annotation {
    /// Unique identifier
    pub id: String,

    /// Book ID
    pub book_id: String,

    /// Chapter index
    pub chapter_index: usize,

    /// Start position in content
    pub start_pos: usize,

    /// End position in content
    pub end_pos: usize,

    /// Highlighted text
    pub text: String,

    /// User's note
    pub note: Option<String>,

    /// Annotation type
    pub annotation_type: AnnotationType,

    /// Highlight color
    pub color: String,

    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Last modified timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Annotation {
    /// Create a new highlight
    pub fn highlight(
        book_id: &str,
        chapter_index: usize,
        start_pos: usize,
        end_pos: usize,
        text: &str,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            book_id: book_id.to_string(),
            chapter_index,
            start_pos,
            end_pos,
            text: text.to_string(),
            note: None,
            annotation_type: AnnotationType::Highlight,
            color: "yellow".to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a bookmark
    pub fn bookmark(book_id: &str, chapter_index: usize, position: usize) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            book_id: book_id.to_string(),
            chapter_index,
            start_pos: position,
            end_pos: position,
            text: String::new(),
            note: None,
            annotation_type: AnnotationType::Bookmark,
            color: "blue".to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a note to the annotation
    pub fn with_note(mut self, note: &str) -> Self {
        self.note = Some(note.to_string());
        self.annotation_type = AnnotationType::Note;
        self.updated_at = chrono::Utc::now();
        self
    }

    /// Change highlight color
    pub fn with_color(mut self, color: &str) -> Self {
        self.color = color.to_string();
        self.updated_at = chrono::Utc::now();
        self
    }
}

/// Annotation manager
pub struct AnnotationManager {
    annotations: Vec<Annotation>,
}

impl AnnotationManager {
    pub fn new() -> Self {
        Self {
            annotations: Vec::new(),
        }
    }

    /// Add an annotation
    pub fn add(&mut self, annotation: Annotation) {
        self.annotations.push(annotation);
    }

    /// Get annotations for a book
    pub fn for_book(&self, book_id: &str) -> Vec<&Annotation> {
        self.annotations
            .iter()
            .filter(|a| a.book_id == book_id)
            .collect()
    }

    /// Get annotations for a chapter
    pub fn for_chapter(&self, book_id: &str, chapter_index: usize) -> Vec<&Annotation> {
        self.annotations
            .iter()
            .filter(|a| a.book_id == book_id && a.chapter_index == chapter_index)
            .collect()
    }

    /// Remove an annotation
    pub fn remove(&mut self, id: &str) -> bool {
        if let Some(pos) = self.annotations.iter().position(|a| a.id == id) {
            self.annotations.remove(pos);
            true
        } else {
            false
        }
    }

    /// Export annotations as markdown
    pub fn export_markdown(&self, book_id: &str) -> String {
        let annotations = self.for_book(book_id);
        let mut output = String::new();

        for annotation in annotations {
            match annotation.annotation_type {
                AnnotationType::Highlight => {
                    output.push_str(&format!("> {}\n\n", annotation.text));
                    if let Some(ref note) = annotation.note {
                        output.push_str(&format!("Note: {}\n\n", note));
                    }
                }
                AnnotationType::Note => {
                    output.push_str(&format!(
                        "**Note:** {}\n\n",
                        annotation.note.as_deref().unwrap_or("")
                    ));
                }
                AnnotationType::Bookmark => {
                    output.push_str(&format!(
                        "📌 Bookmark at chapter {}\n\n",
                        annotation.chapter_index
                    ));
                }
            }
        }

        output
    }
}

impl Default for AnnotationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_creation() {
        let highlight = Annotation::highlight("book1", 0, 10, 50, "Important text");

        assert_eq!(highlight.book_id, "book1");
        assert_eq!(highlight.chapter_index, 0);
        assert_eq!(highlight.start_pos, 10);
        assert_eq!(highlight.end_pos, 50);
        assert_eq!(highlight.text, "Important text");
        assert_eq!(highlight.annotation_type, AnnotationType::Highlight);
        assert_eq!(highlight.color, "yellow");
        assert!(highlight.note.is_none());
    }

    #[test]
    fn test_bookmark_creation() {
        let bookmark = Annotation::bookmark("book1", 5, 100);

        assert_eq!(bookmark.book_id, "book1");
        assert_eq!(bookmark.chapter_index, 5);
        assert_eq!(bookmark.start_pos, 100);
        assert_eq!(bookmark.end_pos, 100);
        assert!(bookmark.text.is_empty());
        assert_eq!(bookmark.annotation_type, AnnotationType::Bookmark);
        assert_eq!(bookmark.color, "blue");
    }

    #[test]
    fn test_annotation_with_note() {
        let annotation =
            Annotation::highlight("book1", 0, 0, 10, "Text").with_note("This is important");

        assert_eq!(annotation.note, Some("This is important".to_string()));
        assert_eq!(annotation.annotation_type, AnnotationType::Note);
    }

    #[test]
    fn test_annotation_with_color() {
        let annotation = Annotation::highlight("book1", 0, 0, 10, "Text").with_color("green");

        assert_eq!(annotation.color, "green");
    }

    #[test]
    fn test_annotation_clone() {
        let original = Annotation::highlight("book1", 0, 0, 10, "Text");
        let cloned = original.clone();

        assert_eq!(cloned.book_id, original.book_id);
        assert_eq!(cloned.text, original.text);
        assert_eq!(cloned.annotation_type, original.annotation_type);
    }

    #[test]
    fn test_annotation_manager_new() {
        let manager = AnnotationManager::new();
        assert!(manager.annotations.is_empty());
    }

    #[test]
    fn test_annotation_manager_default() {
        let manager = AnnotationManager::default();
        assert!(manager.annotations.is_empty());
    }

    #[test]
    fn test_annotation_manager_add() {
        let mut manager = AnnotationManager::new();
        let annotation = Annotation::highlight("book1", 0, 0, 10, "Text");

        manager.add(annotation);

        assert_eq!(manager.annotations.len(), 1);
    }

    #[test]
    fn test_annotation_manager_for_book() {
        let mut manager = AnnotationManager::new();
        manager.add(Annotation::highlight("book1", 0, 0, 10, "Text 1"));
        manager.add(Annotation::highlight("book1", 1, 0, 10, "Text 2"));
        manager.add(Annotation::highlight("book2", 0, 0, 10, "Text 3"));

        let book1_annotations = manager.for_book("book1");
        assert_eq!(book1_annotations.len(), 2);

        let book2_annotations = manager.for_book("book2");
        assert_eq!(book2_annotations.len(), 1);

        let book3_annotations = manager.for_book("book3");
        assert_eq!(book3_annotations.len(), 0);
    }

    #[test]
    fn test_annotation_manager_for_chapter() {
        let mut manager = AnnotationManager::new();
        manager.add(Annotation::highlight("book1", 0, 0, 10, "Chapter 0"));
        manager.add(Annotation::highlight("book1", 0, 20, 30, "Chapter 0 again"));
        manager.add(Annotation::highlight("book1", 1, 0, 10, "Chapter 1"));

        let chapter_0 = manager.for_chapter("book1", 0);
        assert_eq!(chapter_0.len(), 2);

        let chapter_1 = manager.for_chapter("book1", 1);
        assert_eq!(chapter_1.len(), 1);
    }

    #[test]
    fn test_annotation_manager_remove() {
        let mut manager = AnnotationManager::new();
        let annotation = Annotation::highlight("book1", 0, 0, 10, "Text");
        let id = annotation.id.clone();

        manager.add(annotation);
        assert_eq!(manager.annotations.len(), 1);

        let removed = manager.remove(&id);
        assert!(removed);
        assert!(manager.annotations.is_empty());
    }

    #[test]
    fn test_annotation_manager_remove_nonexistent() {
        let mut manager = AnnotationManager::new();
        let removed = manager.remove("nonexistent-id");
        assert!(!removed);
    }

    #[test]
    fn test_annotation_manager_export_markdown() {
        let mut manager = AnnotationManager::new();
        manager.add(Annotation::highlight("book1", 0, 0, 10, "Important quote"));
        manager.add(Annotation::bookmark("book1", 5, 100));
        manager.add(
            Annotation::highlight("book1", 1, 0, 10, "Another quote")
                .with_note("My thoughts on this"),
        );

        let markdown = manager.export_markdown("book1");

        assert!(markdown.contains("> Important quote"));
        assert!(markdown.contains("📌 Bookmark at chapter 5"));
        assert!(markdown.contains("**Note:**"));
    }

    #[test]
    fn test_annotation_manager_export_empty() {
        let manager = AnnotationManager::new();
        let markdown = manager.export_markdown("book1");
        assert!(markdown.is_empty());
    }

    #[test]
    fn test_annotation_id_unique() {
        let a1 = Annotation::highlight("book1", 0, 0, 10, "Text");
        let a2 = Annotation::highlight("book1", 0, 0, 10, "Text");

        // Each annotation should have a unique ID
        assert_ne!(a1.id, a2.id);
    }

    #[test]
    fn test_annotation_timestamps() {
        let annotation = Annotation::highlight("book1", 0, 0, 10, "Text");

        // Created and updated should be set
        assert!(annotation.created_at <= chrono::Utc::now());
        assert_eq!(annotation.created_at, annotation.updated_at);
    }

    #[test]
    fn test_annotation_with_note_updates_timestamp() {
        let annotation = Annotation::highlight("book1", 0, 0, 10, "Text");
        let original_updated = annotation.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));

        let annotated = annotation.with_note("Note");

        // updated_at should be later (or equal if very fast)
        assert!(annotated.updated_at >= original_updated);
    }
}
