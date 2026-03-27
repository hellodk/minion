//! Media metadata extraction

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Metadata extracted from a media file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaMetadata {
    /// Full path to the media file
    pub file_path: String,

    /// File name (basename)
    pub file_name: String,

    /// File size in bytes
    pub file_size: u64,

    /// Detected media type
    pub media_type: MediaType,

    /// Duration in seconds (for audio/video)
    pub duration_seconds: Option<f64>,

    /// Width in pixels (for video/image)
    pub width: Option<u32>,

    /// Height in pixels (for video/image)
    pub height: Option<u32>,

    /// Codec name (e.g. "h264", "aac")
    pub codec: Option<String>,

    /// Bitrate in bits per second
    pub bitrate: Option<u64>,

    /// File creation timestamp
    pub created_at: Option<DateTime<Utc>>,
}

/// Type of media content
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType {
    Video,
    Audio,
    Image,
}

impl MediaMetadata {
    /// Create metadata from a file path, inferring media type from the extension.
    ///
    /// Populates `file_path`, `file_name`, `file_size`, and `media_type`.
    /// Other fields are left as `None` because full extraction requires
    /// external tools (ffprobe, etc.).
    pub fn from_path(path: &str) -> crate::Result<Self> {
        let p = Path::new(path);

        let file_name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let extension = p
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        let media_type = detect_media_type(&extension).ok_or_else(|| {
            crate::Error::Video(format!("Unsupported media extension: {extension}"))
        })?;

        // Try to read file size from disk; fall back to 0 if the file doesn't
        // exist yet (e.g. planning a future render).
        let file_size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);

        Ok(Self {
            file_path: path.to_string(),
            file_name,
            file_size,
            media_type,
            duration_seconds: None,
            width: None,
            height: None,
            codec: None,
            bitrate: None,
            created_at: None,
        })
    }

    /// Return the resolution as a `"WIDTHxHEIGHT"` string if both dimensions
    /// are known.
    pub fn resolution(&self) -> Option<String> {
        match (self.width, self.height) {
            (Some(w), Some(h)) => Some(format!("{w}x{h}")),
            _ => None,
        }
    }

    /// Whether the media is at least HD (width >= 1280).
    pub fn is_hd(&self) -> bool {
        self.width.is_some_and(|w| w >= 1280)
    }

    /// Whether the media is at least 4K (width >= 3840).
    pub fn is_4k(&self) -> bool {
        self.width.is_some_and(|w| w >= 3840)
    }

    /// Compute the aspect ratio in simplified form (e.g. `"16:9"`).
    pub fn aspect_ratio(&self) -> Option<String> {
        match (self.width, self.height) {
            (Some(w), Some(h)) if w > 0 && h > 0 => {
                let divisor = gcd(w, h);
                Some(format!("{}:{}", w / divisor, h / divisor))
            }
            _ => None,
        }
    }
}

/// Compute the greatest common divisor of two positive integers.
fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Detect the media type from a file extension (lower-case, no dot).
fn detect_media_type(extension: &str) -> Option<MediaType> {
    match extension {
        // Video
        "mp4" | "mkv" | "webm" | "avi" | "mov" | "wmv" | "flv" | "m4v" | "ts" | "mpg" | "mpeg" => {
            Some(MediaType::Video)
        }
        // Audio
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "opus" | "wma" | "m4a" => Some(MediaType::Audio),
        // Image
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg" | "tiff" | "tif" | "ico" => {
            Some(MediaType::Image)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_media_type_video() {
        assert_eq!(detect_media_type("mp4"), Some(MediaType::Video));
        assert_eq!(detect_media_type("mkv"), Some(MediaType::Video));
        assert_eq!(detect_media_type("webm"), Some(MediaType::Video));
        assert_eq!(detect_media_type("avi"), Some(MediaType::Video));
        assert_eq!(detect_media_type("mov"), Some(MediaType::Video));
    }

    #[test]
    fn test_detect_media_type_audio() {
        assert_eq!(detect_media_type("mp3"), Some(MediaType::Audio));
        assert_eq!(detect_media_type("wav"), Some(MediaType::Audio));
        assert_eq!(detect_media_type("flac"), Some(MediaType::Audio));
        assert_eq!(detect_media_type("ogg"), Some(MediaType::Audio));
    }

    #[test]
    fn test_detect_media_type_image() {
        assert_eq!(detect_media_type("jpg"), Some(MediaType::Image));
        assert_eq!(detect_media_type("jpeg"), Some(MediaType::Image));
        assert_eq!(detect_media_type("png"), Some(MediaType::Image));
        assert_eq!(detect_media_type("gif"), Some(MediaType::Image));
        assert_eq!(detect_media_type("webp"), Some(MediaType::Image));
    }

    #[test]
    fn test_detect_media_type_unknown() {
        assert_eq!(detect_media_type("xyz"), None);
        assert_eq!(detect_media_type("rs"), None);
        assert_eq!(detect_media_type(""), None);
    }

    #[test]
    fn test_from_path_video() {
        let meta = MediaMetadata::from_path("/tmp/nonexistent/video.mp4").unwrap();
        assert_eq!(meta.media_type, MediaType::Video);
        assert_eq!(meta.file_name, "video.mp4");
        assert_eq!(meta.file_path, "/tmp/nonexistent/video.mp4");
        assert_eq!(meta.file_size, 0); // file doesn't exist
    }

    #[test]
    fn test_from_path_audio() {
        let meta = MediaMetadata::from_path("/tmp/nonexistent/song.mp3").unwrap();
        assert_eq!(meta.media_type, MediaType::Audio);
        assert_eq!(meta.file_name, "song.mp3");
    }

    #[test]
    fn test_from_path_image() {
        let meta = MediaMetadata::from_path("/tmp/nonexistent/photo.jpg").unwrap();
        assert_eq!(meta.media_type, MediaType::Image);
    }

    #[test]
    fn test_from_path_unsupported() {
        let result = MediaMetadata::from_path("/tmp/file.xyz");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolution() {
        let meta = MediaMetadata {
            file_path: "test.mp4".to_string(),
            file_name: "test.mp4".to_string(),
            file_size: 0,
            media_type: MediaType::Video,
            duration_seconds: None,
            width: Some(1920),
            height: Some(1080),
            codec: None,
            bitrate: None,
            created_at: None,
        };
        assert_eq!(meta.resolution(), Some("1920x1080".to_string()));
    }

    #[test]
    fn test_resolution_none() {
        let meta = MediaMetadata {
            file_path: "test.mp4".to_string(),
            file_name: "test.mp4".to_string(),
            file_size: 0,
            media_type: MediaType::Video,
            duration_seconds: None,
            width: None,
            height: None,
            codec: None,
            bitrate: None,
            created_at: None,
        };
        assert_eq!(meta.resolution(), None);
    }

    #[test]
    fn test_is_hd() {
        let make = |w| MediaMetadata {
            file_path: String::new(),
            file_name: String::new(),
            file_size: 0,
            media_type: MediaType::Video,
            duration_seconds: None,
            width: Some(w),
            height: Some(720),
            codec: None,
            bitrate: None,
            created_at: None,
        };

        assert!(!make(1279).is_hd());
        assert!(make(1280).is_hd());
        assert!(make(1920).is_hd());
        assert!(make(3840).is_hd());
    }

    #[test]
    fn test_is_hd_none() {
        let meta = MediaMetadata {
            file_path: String::new(),
            file_name: String::new(),
            file_size: 0,
            media_type: MediaType::Video,
            duration_seconds: None,
            width: None,
            height: None,
            codec: None,
            bitrate: None,
            created_at: None,
        };
        assert!(!meta.is_hd());
    }

    #[test]
    fn test_is_4k() {
        let make = |w| MediaMetadata {
            file_path: String::new(),
            file_name: String::new(),
            file_size: 0,
            media_type: MediaType::Video,
            duration_seconds: None,
            width: Some(w),
            height: Some(2160),
            codec: None,
            bitrate: None,
            created_at: None,
        };

        assert!(!make(3839).is_4k());
        assert!(make(3840).is_4k());
        assert!(make(7680).is_4k());
    }

    #[test]
    fn test_aspect_ratio_16_9() {
        let meta = MediaMetadata {
            file_path: String::new(),
            file_name: String::new(),
            file_size: 0,
            media_type: MediaType::Video,
            duration_seconds: None,
            width: Some(1920),
            height: Some(1080),
            codec: None,
            bitrate: None,
            created_at: None,
        };
        assert_eq!(meta.aspect_ratio(), Some("16:9".to_string()));
    }

    #[test]
    fn test_aspect_ratio_4_3() {
        let meta = MediaMetadata {
            file_path: String::new(),
            file_name: String::new(),
            file_size: 0,
            media_type: MediaType::Video,
            duration_seconds: None,
            width: Some(1024),
            height: Some(768),
            codec: None,
            bitrate: None,
            created_at: None,
        };
        assert_eq!(meta.aspect_ratio(), Some("4:3".to_string()));
    }

    #[test]
    fn test_aspect_ratio_1_1() {
        let meta = MediaMetadata {
            file_path: String::new(),
            file_name: String::new(),
            file_size: 0,
            media_type: MediaType::Video,
            duration_seconds: None,
            width: Some(500),
            height: Some(500),
            codec: None,
            bitrate: None,
            created_at: None,
        };
        assert_eq!(meta.aspect_ratio(), Some("1:1".to_string()));
    }

    #[test]
    fn test_aspect_ratio_none() {
        let meta = MediaMetadata {
            file_path: String::new(),
            file_name: String::new(),
            file_size: 0,
            media_type: MediaType::Video,
            duration_seconds: None,
            width: None,
            height: Some(1080),
            codec: None,
            bitrate: None,
            created_at: None,
        };
        assert_eq!(meta.aspect_ratio(), None);
    }

    #[test]
    fn test_gcd() {
        assert_eq!(gcd(1920, 1080), 120);
        assert_eq!(gcd(1024, 768), 256);
        assert_eq!(gcd(500, 500), 500);
        assert_eq!(gcd(7, 3), 1);
    }

    #[test]
    fn test_media_metadata_serialization() {
        let meta = MediaMetadata {
            file_path: "video.mp4".to_string(),
            file_name: "video.mp4".to_string(),
            file_size: 1024,
            media_type: MediaType::Video,
            duration_seconds: Some(60.0),
            width: Some(1920),
            height: Some(1080),
            codec: Some("h264".to_string()),
            bitrate: Some(5_000_000),
            created_at: None,
        };

        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: MediaMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.file_name, "video.mp4");
        assert_eq!(deserialized.media_type, MediaType::Video);
        assert_eq!(deserialized.width, Some(1920));
    }

    #[test]
    fn test_media_type_serialization() {
        let json = serde_json::to_string(&MediaType::Audio).unwrap();
        let deserialized: MediaType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, MediaType::Audio);
    }

    #[test]
    fn test_from_path_with_real_file() {
        let dir = std::env::temp_dir().join("minion_media_test");
        std::fs::create_dir_all(&dir).ok();
        let file_path = dir.join("test_clip.mp4");
        std::fs::write(&file_path, b"fake video content").unwrap();

        let meta = MediaMetadata::from_path(file_path.to_str().unwrap()).unwrap();
        assert_eq!(meta.media_type, MediaType::Video);
        assert_eq!(meta.file_size, 18); // "fake video content" = 18 bytes
        assert_eq!(meta.file_name, "test_clip.mp4");

        std::fs::remove_file(&file_path).ok();
        std::fs::remove_dir(&dir).ok();
    }
}
