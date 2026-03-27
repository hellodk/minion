//! Multi-platform publishing

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::platforms::PlatformType;

/// Outcome of a publish attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PublishStatus {
    Pending,
    Published,
    Failed,
}

/// Record of a single publish attempt for a post on a platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishRecord {
    pub id: String,
    pub post_id: String,
    pub platform: PlatformType,
    pub status: PublishStatus,
    pub url: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

/// Manages in-memory publish records.
pub struct PublishManager {
    records: Vec<PublishRecord>,
}

impl PublishManager {
    /// Create a new empty `PublishManager`.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Record a successful publish.
    pub fn record_publish(
        &mut self,
        post_id: &str,
        platform: PlatformType,
        url: &str,
    ) -> PublishRecord {
        let record = PublishRecord {
            id: Uuid::new_v4().to_string(),
            post_id: post_id.to_string(),
            platform,
            status: PublishStatus::Published,
            url: Some(url.to_string()),
            published_at: Some(Utc::now()),
            error: None,
        };
        self.records.push(record.clone());
        record
    }

    /// Record a failed publish attempt.
    pub fn record_failure(
        &mut self,
        post_id: &str,
        platform: PlatformType,
        error: &str,
    ) -> PublishRecord {
        let record = PublishRecord {
            id: Uuid::new_v4().to_string(),
            post_id: post_id.to_string(),
            platform,
            status: PublishStatus::Failed,
            url: None,
            published_at: None,
            error: Some(error.to_string()),
        };
        self.records.push(record.clone());
        record
    }

    /// Retrieve all publish records for a given post id.
    pub fn get_records_for_post(&self, post_id: &str) -> Vec<&PublishRecord> {
        self.records
            .iter()
            .filter(|r| r.post_id == post_id)
            .collect()
    }

    /// Check whether a post has been successfully published on a given platform.
    pub fn is_published(&self, post_id: &str, platform: PlatformType) -> bool {
        self.records.iter().any(|r| {
            r.post_id == post_id && r.platform == platform && r.status == PublishStatus::Published
        })
    }

    /// Return a slice of all publish records.
    pub fn list(&self) -> &[PublishRecord] {
        &self.records
    }
}

impl Default for PublishManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let mgr = PublishManager::new();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_record_publish() {
        let mut mgr = PublishManager::new();
        let record = mgr.record_publish(
            "post-1",
            PlatformType::Medium,
            "https://medium.com/@user/my-post",
        );

        assert_eq!(record.post_id, "post-1");
        assert_eq!(record.platform, PlatformType::Medium);
        assert_eq!(record.status, PublishStatus::Published);
        assert_eq!(
            record.url.as_deref(),
            Some("https://medium.com/@user/my-post")
        );
        assert!(record.published_at.is_some());
        assert!(record.error.is_none());
        assert!(!record.id.is_empty());
    }

    #[test]
    fn test_record_failure() {
        let mut mgr = PublishManager::new();
        let record = mgr.record_failure("post-2", PlatformType::WordPress, "401 Unauthorized");

        assert_eq!(record.post_id, "post-2");
        assert_eq!(record.platform, PlatformType::WordPress);
        assert_eq!(record.status, PublishStatus::Failed);
        assert!(record.url.is_none());
        assert!(record.published_at.is_none());
        assert_eq!(record.error.as_deref(), Some("401 Unauthorized"));
    }

    #[test]
    fn test_get_records_for_post() {
        let mut mgr = PublishManager::new();
        mgr.record_publish("post-1", PlatformType::Medium, "https://medium.com/p1");
        mgr.record_failure("post-1", PlatformType::WordPress, "timeout");
        mgr.record_publish("post-2", PlatformType::DevTo, "https://dev.to/p2");

        let records = mgr.get_records_for_post("post-1");
        assert_eq!(records.len(), 2);

        let records2 = mgr.get_records_for_post("post-2");
        assert_eq!(records2.len(), 1);
    }

    #[test]
    fn test_get_records_for_post_none() {
        let mgr = PublishManager::new();
        assert!(mgr.get_records_for_post("nonexistent").is_empty());
    }

    #[test]
    fn test_is_published_true() {
        let mut mgr = PublishManager::new();
        mgr.record_publish("post-1", PlatformType::Medium, "https://medium.com/p1");
        assert!(mgr.is_published("post-1", PlatformType::Medium));
    }

    #[test]
    fn test_is_published_false_different_platform() {
        let mut mgr = PublishManager::new();
        mgr.record_publish("post-1", PlatformType::Medium, "https://medium.com/p1");
        assert!(!mgr.is_published("post-1", PlatformType::WordPress));
    }

    #[test]
    fn test_is_published_false_failed() {
        let mut mgr = PublishManager::new();
        mgr.record_failure("post-1", PlatformType::Medium, "error");
        assert!(!mgr.is_published("post-1", PlatformType::Medium));
    }

    #[test]
    fn test_is_published_false_no_records() {
        let mgr = PublishManager::new();
        assert!(!mgr.is_published("post-1", PlatformType::Medium));
    }

    #[test]
    fn test_list() {
        let mut mgr = PublishManager::new();
        mgr.record_publish("p1", PlatformType::Medium, "url1");
        mgr.record_failure("p2", PlatformType::DevTo, "err");
        mgr.record_publish("p3", PlatformType::Hashnode, "url3");

        assert_eq!(mgr.list().len(), 3);
    }

    #[test]
    fn test_default_impl() {
        let mgr = PublishManager::default();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_publish_status_serde() {
        let json = serde_json::to_string(&PublishStatus::Published).unwrap();
        let deserialized: PublishStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, PublishStatus::Published);
    }

    #[test]
    fn test_publish_record_serde() {
        let mut mgr = PublishManager::new();
        let record = mgr.record_publish("post-1", PlatformType::Medium, "https://example.com");
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: PublishRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, record.id);
        assert_eq!(deserialized.post_id, record.post_id);
        assert_eq!(deserialized.platform, record.platform);
        assert_eq!(deserialized.status, record.status);
        assert_eq!(deserialized.url, record.url);
    }

    #[test]
    fn test_multiple_publish_same_post_platform() {
        let mut mgr = PublishManager::new();
        mgr.record_failure("post-1", PlatformType::Medium, "first attempt failed");
        mgr.record_publish("post-1", PlatformType::Medium, "https://medium.com/post");

        assert!(mgr.is_published("post-1", PlatformType::Medium));
        let records = mgr.get_records_for_post("post-1");
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn test_unique_ids() {
        let mut mgr = PublishManager::new();
        let r1 = mgr.record_publish("p", PlatformType::Medium, "u1");
        let r2 = mgr.record_publish("p", PlatformType::DevTo, "u2");
        assert_ne!(r1.id, r2.id);
    }
}
