//! YouTube API integration

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Visibility setting for a YouTube video
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoVisibility {
    Public,
    Unlisted,
    Private,
}

/// Represents a YouTube video (planned, uploaded, or published)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeVideo {
    /// YouTube video ID (populated after a successful upload)
    pub id: Option<String>,

    /// Video title
    pub title: String,

    /// Video description
    pub description: String,

    /// Tags / keywords
    pub tags: Vec<String>,

    /// Visibility setting
    pub visibility: VideoVisibility,

    /// YouTube category ID (e.g. 22 = People & Blogs)
    pub category_id: u32,

    /// Path to a custom thumbnail image
    pub thumbnail_path: Option<String>,

    /// Scheduled publish time (for premieres / scheduled uploads)
    pub scheduled_at: Option<DateTime<Utc>>,

    /// Actual publication timestamp
    pub published_at: Option<DateTime<Utc>>,
}

/// Basic information about a YouTube channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    /// YouTube channel ID
    pub id: String,

    /// Channel display name
    pub name: String,

    /// Subscriber count
    pub subscriber_count: u64,

    /// Total number of uploaded videos
    pub video_count: u64,
}

/// Manages a collection of YouTube videos.
///
/// This is an in-memory manager for planning uploads, tracking publication
/// state, etc.  Actual YouTube API calls would be performed by a separate
/// async client using the OAuth tokens from `minion-crypto`.
pub struct YouTubeManager {
    videos: Vec<YouTubeVideo>,
}

impl YouTubeManager {
    /// Create an empty manager.
    pub fn new() -> Self {
        Self { videos: vec![] }
    }

    /// Add a new video entry and return a reference to it.
    ///
    /// The video starts with `id: None` (not yet uploaded), default
    /// category 22 (People & Blogs), and no scheduled/published
    /// timestamps.
    pub fn add_video(
        &mut self,
        title: String,
        description: String,
        tags: Vec<String>,
        visibility: VideoVisibility,
    ) -> &YouTubeVideo {
        let video = YouTubeVideo {
            id: None,
            title,
            description,
            tags,
            visibility,
            category_id: 22,
            thumbnail_path: None,
            scheduled_at: None,
            published_at: None,
        };
        self.videos.push(video);
        self.videos.last().expect("just pushed")
    }

    /// Return a slice of all managed videos.
    pub fn list(&self) -> &[YouTubeVideo] {
        &self.videos
    }

    /// Find the first video whose title matches exactly.
    pub fn get(&self, title: &str) -> Option<&YouTubeVideo> {
        self.videos.iter().find(|v| v.title == title)
    }

    /// Mark the video at `index` as published with the given YouTube
    /// video ID and set `published_at` to now.
    pub fn set_published(&mut self, index: usize, video_id: String) -> crate::Result<()> {
        let len = self.videos.len();
        let video = self.videos.get_mut(index).ok_or_else(|| {
            crate::Error::YouTube(format!("Video index {index} out of range (have {len})"))
        })?;

        video.id = Some(video_id);
        video.published_at = Some(Utc::now());
        Ok(())
    }
}

impl Default for YouTubeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- VideoVisibility --

    #[test]
    fn test_visibility_equality() {
        assert_eq!(VideoVisibility::Public, VideoVisibility::Public);
        assert_ne!(VideoVisibility::Public, VideoVisibility::Private);
        assert_ne!(VideoVisibility::Unlisted, VideoVisibility::Private);
    }

    #[test]
    fn test_visibility_serialization() {
        let json = serde_json::to_string(&VideoVisibility::Unlisted).unwrap();
        let deserialized: VideoVisibility = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, VideoVisibility::Unlisted);
    }

    // -- YouTubeVideo --

    #[test]
    fn test_youtube_video_serialization() {
        let video = YouTubeVideo {
            id: Some("abc123".to_string()),
            title: "My Video".to_string(),
            description: "A great video".to_string(),
            tags: vec!["rust".to_string(), "coding".to_string()],
            visibility: VideoVisibility::Public,
            category_id: 22,
            thumbnail_path: Some("/thumbs/my_video.jpg".to_string()),
            scheduled_at: None,
            published_at: None,
        };

        let json = serde_json::to_string(&video).unwrap();
        let deserialized: YouTubeVideo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.title, "My Video");
        assert_eq!(deserialized.id, Some("abc123".to_string()));
        assert_eq!(deserialized.tags.len(), 2);
    }

    // -- ChannelInfo --

    #[test]
    fn test_channel_info_serialization() {
        let channel = ChannelInfo {
            id: "UC123".to_string(),
            name: "My Channel".to_string(),
            subscriber_count: 50_000,
            video_count: 120,
        };

        let json = serde_json::to_string(&channel).unwrap();
        let deserialized: ChannelInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "My Channel");
        assert_eq!(deserialized.subscriber_count, 50_000);
    }

    // -- YouTubeManager --

    #[test]
    fn test_manager_new_is_empty() {
        let mgr = YouTubeManager::new();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_manager_default() {
        let mgr = YouTubeManager::default();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_add_video() {
        let mut mgr = YouTubeManager::new();
        let video = mgr.add_video(
            "First Video".to_string(),
            "Description".to_string(),
            vec!["tag1".to_string()],
            VideoVisibility::Public,
        );

        assert_eq!(video.title, "First Video");
        assert_eq!(video.description, "Description");
        assert_eq!(video.visibility, VideoVisibility::Public);
        assert!(video.id.is_none());
        assert_eq!(video.category_id, 22);
        assert!(video.thumbnail_path.is_none());
        assert!(video.scheduled_at.is_none());
        assert!(video.published_at.is_none());
    }

    #[test]
    fn test_list_videos() {
        let mut mgr = YouTubeManager::new();
        mgr.add_video(
            "V1".to_string(),
            "D1".to_string(),
            vec![],
            VideoVisibility::Public,
        );
        mgr.add_video(
            "V2".to_string(),
            "D2".to_string(),
            vec![],
            VideoVisibility::Private,
        );

        let list = mgr.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].title, "V1");
        assert_eq!(list[1].title, "V2");
    }

    #[test]
    fn test_get_video_found() {
        let mut mgr = YouTubeManager::new();
        mgr.add_video(
            "Target".to_string(),
            "desc".to_string(),
            vec![],
            VideoVisibility::Unlisted,
        );

        let found = mgr.get("Target");
        assert!(found.is_some());
        assert_eq!(found.unwrap().visibility, VideoVisibility::Unlisted);
    }

    #[test]
    fn test_get_video_not_found() {
        let mgr = YouTubeManager::new();
        assert!(mgr.get("Nonexistent").is_none());
    }

    #[test]
    fn test_set_published_success() {
        let mut mgr = YouTubeManager::new();
        mgr.add_video(
            "V1".to_string(),
            "D1".to_string(),
            vec![],
            VideoVisibility::Public,
        );

        mgr.set_published(0, "yt_abc123".to_string()).unwrap();

        let video = &mgr.list()[0];
        assert_eq!(video.id, Some("yt_abc123".to_string()));
        assert!(video.published_at.is_some());
    }

    #[test]
    fn test_set_published_out_of_range() {
        let mut mgr = YouTubeManager::new();
        let result = mgr.set_published(0, "id".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_set_published_error_message() {
        let mut mgr = YouTubeManager::new();
        let err = mgr.set_published(5, "id".to_string()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("5"));
        assert!(msg.contains("out of range"));
    }

    #[test]
    fn test_add_multiple_and_get() {
        let mut mgr = YouTubeManager::new();
        mgr.add_video(
            "Alpha".to_string(),
            "A".to_string(),
            vec!["a".to_string()],
            VideoVisibility::Public,
        );
        mgr.add_video(
            "Beta".to_string(),
            "B".to_string(),
            vec!["b".to_string()],
            VideoVisibility::Private,
        );
        mgr.add_video(
            "Gamma".to_string(),
            "G".to_string(),
            vec![],
            VideoVisibility::Unlisted,
        );

        assert_eq!(mgr.list().len(), 3);
        assert_eq!(
            mgr.get("Beta").unwrap().visibility,
            VideoVisibility::Private
        );
        assert!(mgr.get("Delta").is_none());
    }

    #[test]
    fn test_set_published_then_verify_list() {
        let mut mgr = YouTubeManager::new();
        mgr.add_video(
            "V".to_string(),
            "D".to_string(),
            vec![],
            VideoVisibility::Public,
        );
        mgr.add_video(
            "W".to_string(),
            "E".to_string(),
            vec![],
            VideoVisibility::Public,
        );

        mgr.set_published(1, "yt_xyz".to_string()).unwrap();

        // First video unchanged
        assert!(mgr.list()[0].id.is_none());
        assert!(mgr.list()[0].published_at.is_none());

        // Second video updated
        assert_eq!(mgr.list()[1].id, Some("yt_xyz".to_string()));
        assert!(mgr.list()[1].published_at.is_some());
    }

    #[test]
    fn test_video_with_tags() {
        let mut mgr = YouTubeManager::new();
        mgr.add_video(
            "Tagged".to_string(),
            "D".to_string(),
            vec![
                "rust".to_string(),
                "programming".to_string(),
                "tutorial".to_string(),
            ],
            VideoVisibility::Public,
        );

        let video = mgr.get("Tagged").unwrap();
        assert_eq!(video.tags.len(), 3);
        assert_eq!(video.tags[0], "rust");
        assert_eq!(video.tags[2], "tutorial");
    }
}
