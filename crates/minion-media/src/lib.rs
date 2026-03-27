//! MINION Media Intelligence Module
//!
//! Video processing, YouTube automation, and AI content generation.

pub mod metadata;
pub mod thumbnails;
pub mod video;
pub mod youtube;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Video error: {0}")]
    Video(String),

    #[error("YouTube error: {0}")]
    YouTube(String),

    #[error("Auth error: {0}")]
    Auth(String),

    #[error("Upload error: {0}")]
    Upload(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_video() {
        let err = Error::Video("test video error".to_string());
        assert!(err.to_string().contains("Video error"));
        assert!(err.to_string().contains("test video error"));
    }

    #[test]
    fn test_error_youtube() {
        let err = Error::YouTube("test youtube error".to_string());
        assert!(err.to_string().contains("YouTube error"));
    }

    #[test]
    fn test_error_auth() {
        let err = Error::Auth("test auth error".to_string());
        assert!(err.to_string().contains("Auth error"));
    }

    #[test]
    fn test_error_upload() {
        let err = Error::Upload("test upload error".to_string());
        assert!(err.to_string().contains("Upload error"));
    }

    #[test]
    fn test_result_type() {
        let ok_result: Result<i32> = Ok(42);
        assert_eq!(ok_result.unwrap(), 42);

        let err_result: Result<i32> = Err(Error::Video("test".to_string()));
        assert!(err_result.is_err());
    }

    #[test]
    fn test_error_debug() {
        let err = Error::Video("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Video"));
    }
}
