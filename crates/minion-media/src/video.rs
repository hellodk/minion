//! Video processing

use serde::{Deserialize, Serialize};

/// Video quality preset
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoQuality {
    Low,
    Medium,
    High,
    Ultra,
}

/// Container format for video output
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoFormat {
    Mp4,
    Mkv,
    Webm,
    Avi,
    Mov,
}

/// Configuration for video encoding / rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoConfig {
    /// Quality preset
    pub quality: VideoQuality,

    /// Output container format
    pub format: VideoFormat,

    /// Output width in pixels
    pub width: Option<u32>,

    /// Output height in pixels
    pub height: Option<u32>,

    /// Frames per second
    pub fps: Option<u32>,

    /// Target bitrate in kilobits per second
    pub bitrate_kbps: Option<u32>,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            quality: VideoQuality::High,
            format: VideoFormat::Mp4,
            width: None,
            height: None,
            fps: Some(30),
            bitrate_kbps: None,
        }
    }
}

impl VideoConfig {
    /// Preset for 1080p output (1920x1080, 30 fps, 8 Mbps).
    pub fn preset_1080p() -> Self {
        Self {
            quality: VideoQuality::High,
            format: VideoFormat::Mp4,
            width: Some(1920),
            height: Some(1080),
            fps: Some(30),
            bitrate_kbps: Some(8_000),
        }
    }

    /// Preset for 720p output (1280x720, 30 fps, 5 Mbps).
    pub fn preset_720p() -> Self {
        Self {
            quality: VideoQuality::Medium,
            format: VideoFormat::Mp4,
            width: Some(1280),
            height: Some(720),
            fps: Some(30),
            bitrate_kbps: Some(5_000),
        }
    }

    /// Preset for 4K output (3840x2160, 30 fps, 35 Mbps).
    pub fn preset_4k() -> Self {
        Self {
            quality: VideoQuality::Ultra,
            format: VideoFormat::Mp4,
            width: Some(3840),
            height: Some(2160),
            fps: Some(30),
            bitrate_kbps: Some(35_000),
        }
    }

    /// Estimate the output file size in megabytes for the given duration.
    ///
    /// Uses the configured bitrate when available; otherwise falls back to a
    /// rough default based on the quality preset.
    pub fn estimated_file_size_mb(&self, duration_seconds: f64) -> f64 {
        let kbps = self.bitrate_kbps.unwrap_or(match self.quality {
            VideoQuality::Low => 1_000,
            VideoQuality::Medium => 5_000,
            VideoQuality::High => 8_000,
            VideoQuality::Ultra => 35_000,
        });

        // kbps * seconds / 8 = kilobytes, / 1024 = megabytes
        (kbps as f64) * duration_seconds / 8.0 / 1024.0
    }
}

impl VideoFormat {
    /// File extension for this format (without leading dot).
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mkv => "mkv",
            Self::Webm => "webm",
            Self::Avi => "avi",
            Self::Mov => "mov",
        }
    }
}

/// Status of a video project
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectStatus {
    Created,
    Processing,
    Completed,
    Failed,
}

/// A video processing project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProject {
    /// Unique identifier
    pub id: String,

    /// Human-readable project name
    pub name: String,

    /// Path to the source media file
    pub source_path: String,

    /// Encoding / output configuration
    pub output_config: VideoConfig,

    /// Current processing status
    pub status: ProjectStatus,
}

impl VideoProject {
    /// Create a new project in `Created` status with a random UUID.
    pub fn new(name: String, source_path: String, config: VideoConfig) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            source_path,
            output_config: config,
            status: ProjectStatus::Created,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_config_default() {
        let config = VideoConfig::default();
        assert_eq!(config.quality, VideoQuality::High);
        assert_eq!(config.format, VideoFormat::Mp4);
        assert!(config.width.is_none());
        assert!(config.height.is_none());
        assert_eq!(config.fps, Some(30));
        assert!(config.bitrate_kbps.is_none());
    }

    #[test]
    fn test_preset_1080p() {
        let config = VideoConfig::preset_1080p();
        assert_eq!(config.width, Some(1920));
        assert_eq!(config.height, Some(1080));
        assert_eq!(config.fps, Some(30));
        assert_eq!(config.bitrate_kbps, Some(8_000));
        assert_eq!(config.quality, VideoQuality::High);
    }

    #[test]
    fn test_preset_720p() {
        let config = VideoConfig::preset_720p();
        assert_eq!(config.width, Some(1280));
        assert_eq!(config.height, Some(720));
        assert_eq!(config.fps, Some(30));
        assert_eq!(config.bitrate_kbps, Some(5_000));
        assert_eq!(config.quality, VideoQuality::Medium);
    }

    #[test]
    fn test_preset_4k() {
        let config = VideoConfig::preset_4k();
        assert_eq!(config.width, Some(3840));
        assert_eq!(config.height, Some(2160));
        assert_eq!(config.fps, Some(30));
        assert_eq!(config.bitrate_kbps, Some(35_000));
        assert_eq!(config.quality, VideoQuality::Ultra);
    }

    #[test]
    fn test_estimated_file_size_with_bitrate() {
        // 8000 kbps, 60 seconds: 8000 * 60 / 8 / 1024 = 58.59375 MB
        let config = VideoConfig::preset_1080p();
        let size = config.estimated_file_size_mb(60.0);
        assert!((size - 58.59375).abs() < 0.001);
    }

    #[test]
    fn test_estimated_file_size_no_bitrate_uses_quality_default() {
        // High quality, no bitrate => defaults to 8000 kbps => 60s => 58.59375 MB
        let config = VideoConfig::default();
        let size = config.estimated_file_size_mb(60.0);
        assert!((size - 58.59375).abs() < 0.001);
    }

    #[test]
    fn test_estimated_file_size_low_quality() {
        let config = VideoConfig {
            quality: VideoQuality::Low,
            bitrate_kbps: None,
            ..Default::default()
        };
        // Low defaults to 1000 kbps => 60s => 1000*60/8/1024 ≈ 7.32
        let size = config.estimated_file_size_mb(60.0);
        assert!((size - 7.324).abs() < 0.1);
    }

    #[test]
    fn test_estimated_file_size_zero_duration() {
        let config = VideoConfig::preset_1080p();
        assert_eq!(config.estimated_file_size_mb(0.0), 0.0);
    }

    #[test]
    fn test_video_format_extension() {
        assert_eq!(VideoFormat::Mp4.extension(), "mp4");
        assert_eq!(VideoFormat::Mkv.extension(), "mkv");
        assert_eq!(VideoFormat::Webm.extension(), "webm");
        assert_eq!(VideoFormat::Avi.extension(), "avi");
        assert_eq!(VideoFormat::Mov.extension(), "mov");
    }

    #[test]
    fn test_video_project_new() {
        let project = VideoProject::new(
            "My Project".to_string(),
            "/videos/input.mp4".to_string(),
            VideoConfig::preset_1080p(),
        );

        assert!(!project.id.is_empty());
        assert_eq!(project.name, "My Project");
        assert_eq!(project.source_path, "/videos/input.mp4");
        assert_eq!(project.status, ProjectStatus::Created);
        assert_eq!(project.output_config.width, Some(1920));
    }

    #[test]
    fn test_video_project_unique_ids() {
        let p1 = VideoProject::new("A".to_string(), "a.mp4".to_string(), VideoConfig::default());
        let p2 = VideoProject::new("B".to_string(), "b.mp4".to_string(), VideoConfig::default());
        assert_ne!(p1.id, p2.id);
    }

    #[test]
    fn test_project_status_equality() {
        assert_eq!(ProjectStatus::Created, ProjectStatus::Created);
        assert_ne!(ProjectStatus::Created, ProjectStatus::Processing);
        assert_ne!(ProjectStatus::Completed, ProjectStatus::Failed);
    }

    #[test]
    fn test_video_quality_serialization() {
        let json = serde_json::to_string(&VideoQuality::Ultra).unwrap();
        let deserialized: VideoQuality = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, VideoQuality::Ultra);
    }

    #[test]
    fn test_video_format_serialization() {
        let json = serde_json::to_string(&VideoFormat::Webm).unwrap();
        let deserialized: VideoFormat = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, VideoFormat::Webm);
    }

    #[test]
    fn test_video_config_serialization() {
        let config = VideoConfig::preset_4k();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: VideoConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.width, Some(3840));
        assert_eq!(deserialized.height, Some(2160));
        assert_eq!(deserialized.quality, VideoQuality::Ultra);
    }

    #[test]
    fn test_video_project_serialization() {
        let project = VideoProject::new(
            "Test".to_string(),
            "/path/to/video.mp4".to_string(),
            VideoConfig::default(),
        );
        let json = serde_json::to_string(&project).unwrap();
        let deserialized: VideoProject = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "Test");
        assert_eq!(deserialized.status, ProjectStatus::Created);
    }
}
