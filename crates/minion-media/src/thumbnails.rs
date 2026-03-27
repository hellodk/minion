//! Thumbnail generation

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Predefined or custom thumbnail size
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThumbnailSize {
    /// 160x90
    Small,
    /// 320x180
    Medium,
    /// 640x360
    Large,
    /// Arbitrary width and height
    Custom(u32, u32),
}

impl ThumbnailSize {
    /// Return the (width, height) for this size variant.
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::Small => (160, 90),
            Self::Medium => (320, 180),
            Self::Large => (640, 360),
            Self::Custom(w, h) => (*w, *h),
        }
    }
}

/// Image format for the generated thumbnail
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThumbnailFormat {
    Jpeg,
    Png,
    Webp,
}

impl ThumbnailFormat {
    /// File extension for this format (without leading dot).
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
        }
    }
}

/// Configuration for thumbnail generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailConfig {
    /// Target thumbnail dimensions
    pub size: ThumbnailSize,

    /// Output image format
    pub format: ThumbnailFormat,

    /// Image quality (1-100, applicable to JPEG/WebP)
    pub quality: u8,
}

impl Default for ThumbnailConfig {
    fn default() -> Self {
        Self {
            size: ThumbnailSize::Medium,
            format: ThumbnailFormat::Jpeg,
            quality: 85,
        }
    }
}

/// Generates thumbnail file paths from source media.
///
/// Actual pixel-level image processing requires external tools (ffmpeg,
/// image crate, etc.).  This struct provides the naming / configuration
/// helpers used by such pipelines.
pub struct ThumbnailGenerator;

impl ThumbnailGenerator {
    /// Derive an output filename for a thumbnail.
    ///
    /// Given a source path like `"/videos/clip.mp4"` and a JPEG config at
    /// Medium size, this returns `"clip_thumb_320x180.jpg"`.
    pub fn output_filename(source: &str, config: &ThumbnailConfig) -> String {
        let stem = Path::new(source)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "thumb".to_string());

        let (w, h) = config.size.dimensions();
        let ext = config.format.extension();

        format!("{stem}_thumb_{w}x{h}.{ext}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- ThumbnailSize --

    #[test]
    fn test_small_dimensions() {
        assert_eq!(ThumbnailSize::Small.dimensions(), (160, 90));
    }

    #[test]
    fn test_medium_dimensions() {
        assert_eq!(ThumbnailSize::Medium.dimensions(), (320, 180));
    }

    #[test]
    fn test_large_dimensions() {
        assert_eq!(ThumbnailSize::Large.dimensions(), (640, 360));
    }

    #[test]
    fn test_custom_dimensions() {
        assert_eq!(ThumbnailSize::Custom(800, 600).dimensions(), (800, 600));
    }

    #[test]
    fn test_thumbnail_size_copy() {
        let size = ThumbnailSize::Custom(100, 200);
        let copy = size; // Copy
        assert_eq!(size, copy);
    }

    // -- ThumbnailFormat --

    #[test]
    fn test_format_extension_jpeg() {
        assert_eq!(ThumbnailFormat::Jpeg.extension(), "jpg");
    }

    #[test]
    fn test_format_extension_png() {
        assert_eq!(ThumbnailFormat::Png.extension(), "png");
    }

    #[test]
    fn test_format_extension_webp() {
        assert_eq!(ThumbnailFormat::Webp.extension(), "webp");
    }

    // -- ThumbnailConfig --

    #[test]
    fn test_config_default() {
        let config = ThumbnailConfig::default();
        assert_eq!(config.size, ThumbnailSize::Medium);
        assert_eq!(config.format, ThumbnailFormat::Jpeg);
        assert_eq!(config.quality, 85);
    }

    #[test]
    fn test_config_serialization() {
        let config = ThumbnailConfig {
            size: ThumbnailSize::Large,
            format: ThumbnailFormat::Png,
            quality: 90,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ThumbnailConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.size, ThumbnailSize::Large);
        assert_eq!(deserialized.format, ThumbnailFormat::Png);
        assert_eq!(deserialized.quality, 90);
    }

    #[test]
    fn test_config_custom_size_serialization() {
        let config = ThumbnailConfig {
            size: ThumbnailSize::Custom(1280, 720),
            format: ThumbnailFormat::Webp,
            quality: 75,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ThumbnailConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.size, ThumbnailSize::Custom(1280, 720));
    }

    // -- ThumbnailGenerator --

    #[test]
    fn test_output_filename_default_config() {
        let config = ThumbnailConfig::default(); // Medium, JPEG
        let name = ThumbnailGenerator::output_filename("/videos/clip.mp4", &config);
        assert_eq!(name, "clip_thumb_320x180.jpg");
    }

    #[test]
    fn test_output_filename_large_png() {
        let config = ThumbnailConfig {
            size: ThumbnailSize::Large,
            format: ThumbnailFormat::Png,
            quality: 100,
        };
        let name = ThumbnailGenerator::output_filename("/path/to/movie.mkv", &config);
        assert_eq!(name, "movie_thumb_640x360.png");
    }

    #[test]
    fn test_output_filename_small_webp() {
        let config = ThumbnailConfig {
            size: ThumbnailSize::Small,
            format: ThumbnailFormat::Webp,
            quality: 50,
        };
        let name = ThumbnailGenerator::output_filename("photo.jpg", &config);
        assert_eq!(name, "photo_thumb_160x90.webp");
    }

    #[test]
    fn test_output_filename_custom_size() {
        let config = ThumbnailConfig {
            size: ThumbnailSize::Custom(1920, 1080),
            format: ThumbnailFormat::Jpeg,
            quality: 85,
        };
        let name = ThumbnailGenerator::output_filename("input.avi", &config);
        assert_eq!(name, "input_thumb_1920x1080.jpg");
    }

    #[test]
    fn test_output_filename_no_extension_source() {
        let config = ThumbnailConfig::default();
        let name = ThumbnailGenerator::output_filename("/tmp/noext", &config);
        assert_eq!(name, "noext_thumb_320x180.jpg");
    }

    #[test]
    fn test_output_filename_nested_path() {
        let config = ThumbnailConfig::default();
        let name = ThumbnailGenerator::output_filename("/a/b/c/d/deep_video.webm", &config);
        assert_eq!(name, "deep_video_thumb_320x180.jpg");
    }

    #[test]
    fn test_thumbnail_size_equality() {
        assert_eq!(ThumbnailSize::Small, ThumbnailSize::Small);
        assert_ne!(ThumbnailSize::Small, ThumbnailSize::Medium);
        assert_ne!(
            ThumbnailSize::Custom(100, 100),
            ThumbnailSize::Custom(200, 200)
        );
    }

    #[test]
    fn test_thumbnail_format_equality() {
        assert_eq!(ThumbnailFormat::Jpeg, ThumbnailFormat::Jpeg);
        assert_ne!(ThumbnailFormat::Jpeg, ThumbnailFormat::Png);
    }
}
