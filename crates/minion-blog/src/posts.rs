//! Blog post management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result};

/// Status of a blog post in the editorial workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostStatus {
    Draft,
    Review,
    Published,
    Archived,
}

/// A single blog post with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlogPost {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub excerpt: Option<String>,
    pub tags: Vec<String>,
    pub categories: Vec<String>,
    pub status: PostStatus,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub word_count: usize,
    pub reading_time_minutes: u32,
}

/// Manages an in-memory collection of blog posts.
pub struct PostManager {
    posts: Vec<BlogPost>,
}

impl PostManager {
    /// Create a new empty `PostManager`.
    pub fn new() -> Self {
        Self { posts: Vec::new() }
    }

    /// Create a new blog post from a title, markdown content, and author name.
    pub fn create(&mut self, title: &str, content: &str, author: &str) -> Result<BlogPost> {
        if title.is_empty() {
            return Err(Error::Post("title cannot be empty".to_string()));
        }
        if content.is_empty() {
            return Err(Error::Post("content cannot be empty".to_string()));
        }

        let now = Utc::now();
        let wc = word_count(content);

        let post = BlogPost {
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            slug: slugify(title),
            content: content.to_string(),
            excerpt: None,
            tags: Vec::new(),
            categories: Vec::new(),
            status: PostStatus::Draft,
            author: author.to_string(),
            created_at: now,
            updated_at: now,
            published_at: None,
            word_count: wc,
            reading_time_minutes: calculate_reading_time(content),
        };

        self.posts.push(post.clone());
        Ok(post)
    }

    /// Look up a post by its unique id.
    pub fn get(&self, id: &str) -> Option<&BlogPost> {
        self.posts.iter().find(|p| p.id == id)
    }

    /// Look up a post by its URL slug.
    pub fn get_by_slug(&self, slug: &str) -> Option<&BlogPost> {
        self.posts.iter().find(|p| p.slug == slug)
    }

    /// Return a slice of all posts.
    pub fn list(&self) -> &[BlogPost] {
        &self.posts
    }

    /// Return posts that match a given status.
    pub fn list_by_status(&self, status: PostStatus) -> Vec<&BlogPost> {
        self.posts.iter().filter(|p| p.status == status).collect()
    }

    /// Replace the content of an existing post, updating derived fields.
    pub fn update_content(&mut self, id: &str, content: &str) -> Result<()> {
        let post = self
            .posts
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| Error::Post(format!("post not found: {id}")))?;

        post.content = content.to_string();
        post.word_count = word_count(content);
        post.reading_time_minutes = calculate_reading_time(content);
        post.updated_at = Utc::now();
        Ok(())
    }

    /// Transition a post to a new status.
    pub fn update_status(&mut self, id: &str, status: PostStatus) -> Result<()> {
        let post = self
            .posts
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| Error::Post(format!("post not found: {id}")))?;

        post.status = status;
        post.updated_at = Utc::now();

        if status == PostStatus::Published && post.published_at.is_none() {
            post.published_at = Some(Utc::now());
        }
        Ok(())
    }

    /// Remove a post by id.
    pub fn delete(&mut self, id: &str) -> Result<()> {
        let idx = self
            .posts
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| Error::Post(format!("post not found: {id}")))?;

        self.posts.remove(idx);
        Ok(())
    }

    /// Simple case-insensitive text search across title and content.
    pub fn search(&self, query: &str) -> Vec<&BlogPost> {
        let q = query.to_lowercase();
        self.posts
            .iter()
            .filter(|p| {
                p.title.to_lowercase().contains(&q) || p.content.to_lowercase().contains(&q)
            })
            .collect()
    }
}

impl Default for PostManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a title into a URL-friendly slug.
///
/// Lowercases the input, replaces non-alphanumeric characters with hyphens,
/// collapses consecutive hyphens, and trims leading/trailing hyphens.
pub fn slugify(title: &str) -> String {
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();

    // Collapse consecutive hyphens.
    let mut result = String::with_capacity(slug.len());
    let mut prev_hyphen = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }

    result.trim_matches('-').to_string()
}

/// Estimate reading time at approximately 200 words per minute, minimum 1.
pub fn calculate_reading_time(content: &str) -> u32 {
    let wc = word_count(content);
    if wc == 0 {
        return 0;
    }
    let minutes = (wc as f64 / 200.0).ceil() as u32;
    minutes.max(1)
}

/// Count the number of whitespace-delimited words in the content.
pub fn word_count(content: &str) -> usize {
    content.split_whitespace().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- helper function tests ----

    #[test]
    fn test_slugify_basic() {
        assert_eq!(slugify("Hello World"), "hello-world");
    }

    #[test]
    fn test_slugify_special_chars() {
        assert_eq!(slugify("My Post! @#$ Title"), "my-post-title");
    }

    #[test]
    fn test_slugify_leading_trailing() {
        assert_eq!(slugify("  Hello  "), "hello");
    }

    #[test]
    fn test_slugify_consecutive_special() {
        assert_eq!(slugify("a---b___c"), "a-b-c");
    }

    #[test]
    fn test_slugify_empty() {
        assert_eq!(slugify(""), "");
    }

    #[test]
    fn test_slugify_numbers() {
        assert_eq!(slugify("Top 10 Tips for 2024"), "top-10-tips-for-2024");
    }

    #[test]
    fn test_word_count_basic() {
        assert_eq!(word_count("hello world foo"), 3);
    }

    #[test]
    fn test_word_count_empty() {
        assert_eq!(word_count(""), 0);
    }

    #[test]
    fn test_word_count_whitespace_only() {
        assert_eq!(word_count("   \n\t  "), 0);
    }

    #[test]
    fn test_word_count_multiline() {
        assert_eq!(word_count("one\ntwo\nthree"), 3);
    }

    #[test]
    fn test_calculate_reading_time_empty() {
        assert_eq!(calculate_reading_time(""), 0);
    }

    #[test]
    fn test_calculate_reading_time_short() {
        // 50 words -> ceil(50/200) = 1
        let content = (0..50).map(|_| "word").collect::<Vec<_>>().join(" ");
        assert_eq!(calculate_reading_time(&content), 1);
    }

    #[test]
    fn test_calculate_reading_time_medium() {
        // 450 words -> ceil(450/200) = 3
        let content = (0..450).map(|_| "word").collect::<Vec<_>>().join(" ");
        assert_eq!(calculate_reading_time(&content), 3);
    }

    #[test]
    fn test_calculate_reading_time_exact() {
        // 200 words -> ceil(200/200) = 1
        let content = (0..200).map(|_| "word").collect::<Vec<_>>().join(" ");
        assert_eq!(calculate_reading_time(&content), 1);
    }

    // ---- PostManager tests ----

    #[test]
    fn test_create_post() {
        let mut mgr = PostManager::new();
        let post = mgr
            .create("Test Title", "Some content here", "alice")
            .unwrap();
        assert_eq!(post.title, "Test Title");
        assert_eq!(post.slug, "test-title");
        assert_eq!(post.author, "alice");
        assert_eq!(post.status, PostStatus::Draft);
        assert_eq!(post.word_count, 3);
        assert!(post.published_at.is_none());
    }

    #[test]
    fn test_create_empty_title_error() {
        let mut mgr = PostManager::new();
        let err = mgr.create("", "content", "alice").unwrap_err();
        assert!(err.to_string().contains("title cannot be empty"));
    }

    #[test]
    fn test_create_empty_content_error() {
        let mut mgr = PostManager::new();
        let err = mgr.create("Title", "", "alice").unwrap_err();
        assert!(err.to_string().contains("content cannot be empty"));
    }

    #[test]
    fn test_get_post() {
        let mut mgr = PostManager::new();
        let post = mgr.create("Title", "Body text", "bob").unwrap();
        let found = mgr.get(&post.id).unwrap();
        assert_eq!(found.title, "Title");
    }

    #[test]
    fn test_get_missing_post() {
        let mgr = PostManager::new();
        assert!(mgr.get("nonexistent").is_none());
    }

    #[test]
    fn test_get_by_slug() {
        let mut mgr = PostManager::new();
        mgr.create("My Great Post", "body", "carol").unwrap();
        let found = mgr.get_by_slug("my-great-post").unwrap();
        assert_eq!(found.title, "My Great Post");
    }

    #[test]
    fn test_get_by_slug_missing() {
        let mgr = PostManager::new();
        assert!(mgr.get_by_slug("nope").is_none());
    }

    #[test]
    fn test_list() {
        let mut mgr = PostManager::new();
        assert!(mgr.list().is_empty());
        mgr.create("A", "content a", "x").unwrap();
        mgr.create("B", "content b", "x").unwrap();
        assert_eq!(mgr.list().len(), 2);
    }

    #[test]
    fn test_list_by_status() {
        let mut mgr = PostManager::new();
        let p1 = mgr.create("A", "body", "x").unwrap();
        mgr.create("B", "body", "x").unwrap();
        mgr.update_status(&p1.id, PostStatus::Published).unwrap();

        let drafts = mgr.list_by_status(PostStatus::Draft);
        assert_eq!(drafts.len(), 1);
        let published = mgr.list_by_status(PostStatus::Published);
        assert_eq!(published.len(), 1);
    }

    #[test]
    fn test_update_content() {
        let mut mgr = PostManager::new();
        let post = mgr.create("Title", "old body", "x").unwrap();
        mgr.update_content(&post.id, "new body with more words here")
            .unwrap();

        let updated = mgr.get(&post.id).unwrap();
        assert_eq!(updated.content, "new body with more words here");
        assert_eq!(updated.word_count, 6);
        assert!(updated.updated_at > post.updated_at);
    }

    #[test]
    fn test_update_content_missing() {
        let mut mgr = PostManager::new();
        let err = mgr.update_content("nope", "text").unwrap_err();
        assert!(err.to_string().contains("post not found"));
    }

    #[test]
    fn test_update_status() {
        let mut mgr = PostManager::new();
        let post = mgr.create("Title", "body", "x").unwrap();
        mgr.update_status(&post.id, PostStatus::Review).unwrap();

        let updated = mgr.get(&post.id).unwrap();
        assert_eq!(updated.status, PostStatus::Review);
        assert!(updated.published_at.is_none());
    }

    #[test]
    fn test_update_status_published_sets_timestamp() {
        let mut mgr = PostManager::new();
        let post = mgr.create("Title", "body", "x").unwrap();
        mgr.update_status(&post.id, PostStatus::Published).unwrap();

        let updated = mgr.get(&post.id).unwrap();
        assert_eq!(updated.status, PostStatus::Published);
        assert!(updated.published_at.is_some());
    }

    #[test]
    fn test_update_status_published_only_sets_once() {
        let mut mgr = PostManager::new();
        let post = mgr.create("Title", "body", "x").unwrap();
        mgr.update_status(&post.id, PostStatus::Published).unwrap();
        let first_ts = mgr.get(&post.id).unwrap().published_at;

        // Transition away and back — should keep the original published_at.
        mgr.update_status(&post.id, PostStatus::Archived).unwrap();
        mgr.update_status(&post.id, PostStatus::Published).unwrap();
        let second_ts = mgr.get(&post.id).unwrap().published_at;
        assert_eq!(first_ts, second_ts);
    }

    #[test]
    fn test_update_status_missing() {
        let mut mgr = PostManager::new();
        let err = mgr.update_status("nope", PostStatus::Draft).unwrap_err();
        assert!(err.to_string().contains("post not found"));
    }

    #[test]
    fn test_delete() {
        let mut mgr = PostManager::new();
        let post = mgr.create("Title", "body", "x").unwrap();
        mgr.delete(&post.id).unwrap();
        assert!(mgr.get(&post.id).is_none());
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_delete_missing() {
        let mut mgr = PostManager::new();
        let err = mgr.delete("nope").unwrap_err();
        assert!(err.to_string().contains("post not found"));
    }

    #[test]
    fn test_search_by_title() {
        let mut mgr = PostManager::new();
        mgr.create("Rust Programming", "some body", "x").unwrap();
        mgr.create("Python Tips", "other body", "x").unwrap();

        let results = mgr.search("rust");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming");
    }

    #[test]
    fn test_search_by_content() {
        let mut mgr = PostManager::new();
        mgr.create("Title A", "learn about rust today", "x")
            .unwrap();
        mgr.create("Title B", "learn about python today", "x")
            .unwrap();

        let results = mgr.search("python");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Title B");
    }

    #[test]
    fn test_search_case_insensitive() {
        let mut mgr = PostManager::new();
        mgr.create("HELLO World", "body", "x").unwrap();

        let results = mgr.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_no_results() {
        let mut mgr = PostManager::new();
        mgr.create("Title", "body", "x").unwrap();
        let results = mgr.search("zzzzz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_default_impl() {
        let mgr = PostManager::default();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_post_status_serde() {
        let json = serde_json::to_string(&PostStatus::Published).unwrap();
        let deserialized: PostStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, PostStatus::Published);
    }

    #[test]
    fn test_blog_post_serde() {
        let mut mgr = PostManager::new();
        let post = mgr.create("Serde Test", "content for serde", "x").unwrap();
        let json = serde_json::to_string(&post).unwrap();
        let deserialized: BlogPost = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.title, post.title);
        assert_eq!(deserialized.slug, post.slug);
        assert_eq!(deserialized.id, post.id);
    }
}
