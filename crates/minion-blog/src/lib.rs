//! MINION Blog AI Engine
//!
//! Multi-platform blog publishing with AI assistance.

pub mod platforms;
pub mod posts;
pub mod publishing;
pub mod seo;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Post error: {0}")]
    Post(String),

    #[error("Platform error: {0}")]
    Platform(String),

    #[error("Publishing error: {0}")]
    Publishing(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Supported blog platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    WordPress,
    Medium,
    Hashnode,
    DevTo,
    Custom,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_variants() {
        let platforms = [
            Platform::WordPress,
            Platform::Medium,
            Platform::Hashnode,
            Platform::DevTo,
            Platform::Custom,
        ];

        for (i, p1) in platforms.iter().enumerate() {
            for (j, p2) in platforms.iter().enumerate() {
                if i == j {
                    assert_eq!(p1, p2);
                } else {
                    assert_ne!(p1, p2);
                }
            }
        }
    }

    #[test]
    fn test_platform_clone() {
        let platform = Platform::WordPress;
        let cloned = platform.clone();
        assert_eq!(platform, cloned);
    }

    #[test]
    fn test_platform_copy() {
        let platform = Platform::Medium;
        let copied = platform;
        assert_eq!(platform, copied);
    }

    #[test]
    fn test_error_post() {
        let err = Error::Post("test post error".to_string());
        assert!(err.to_string().contains("Post error"));
    }

    #[test]
    fn test_error_platform() {
        let err = Error::Platform("test platform error".to_string());
        assert!(err.to_string().contains("Platform error"));
    }

    #[test]
    fn test_error_publishing() {
        let err = Error::Publishing("test publishing error".to_string());
        assert!(err.to_string().contains("Publishing error"));
    }

    #[test]
    fn test_result_type() {
        let ok_result: Result<i32> = Ok(42);
        assert_eq!(ok_result.unwrap(), 42);

        let err_result: Result<i32> = Err(Error::Post("test".to_string()));
        assert!(err_result.is_err());
    }

    #[test]
    fn test_error_debug() {
        let err = Error::Post("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Post"));
    }
}
