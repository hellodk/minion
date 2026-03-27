//! MINION File Intelligence Module
//!
//! Provides duplicate detection, storage analytics, and file management.

pub mod analytics;
pub mod duplicates;
pub mod hash;
pub mod scanner;

pub use analytics::{AnalyticsCalculator, StorageAnalytics};
pub use duplicates::DuplicateFinder;
pub use scanner::{ScanConfig, ScanResult, Scanner};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Scan error: {0}")]
    Scan(String),

    #[error("Hash error: {0}")]
    Hash(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] minion_db::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// File metadata
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: std::path::PathBuf,
    pub name: String,
    pub extension: Option<String>,
    pub size: u64,
    pub modified: chrono::DateTime<chrono::Utc>,
    pub sha256: Option<String>,
    pub perceptual_hash: Option<u64>,
}

/// Duplicate match type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateType {
    /// Exact byte-for-byte match
    Exact,
    /// Perceptually similar (images/videos)
    Perceptual,
    /// Audio fingerprint match
    Audio,
    /// Near-duplicate (fuzzy)
    Near,
}

/// Duplicate group
#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub id: String,
    pub match_type: DuplicateType,
    pub files: Vec<FileInfo>,
    pub similarity: f32,
    pub wasted_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::path::PathBuf;

    #[test]
    fn test_file_info_creation() {
        let file_info = FileInfo {
            path: PathBuf::from("/test/file.txt"),
            name: "file.txt".to_string(),
            extension: Some("txt".to_string()),
            size: 1024,
            modified: Utc::now(),
            sha256: Some("abc123".to_string()),
            perceptual_hash: None,
        };

        assert_eq!(file_info.name, "file.txt");
        assert_eq!(file_info.extension, Some("txt".to_string()));
        assert_eq!(file_info.size, 1024);
    }

    #[test]
    fn test_file_info_no_extension() {
        let file_info = FileInfo {
            path: PathBuf::from("/test/Makefile"),
            name: "Makefile".to_string(),
            extension: None,
            size: 512,
            modified: Utc::now(),
            sha256: None,
            perceptual_hash: None,
        };

        assert!(file_info.extension.is_none());
    }

    #[test]
    fn test_duplicate_type_variants() {
        let types = [
            DuplicateType::Exact,
            DuplicateType::Perceptual,
            DuplicateType::Audio,
            DuplicateType::Near,
        ];

        for (i, t1) in types.iter().enumerate() {
            for (j, t2) in types.iter().enumerate() {
                if i == j {
                    assert_eq!(t1, t2);
                } else {
                    assert_ne!(t1, t2);
                }
            }
        }
    }

    #[test]
    fn test_duplicate_group_creation() {
        let file1 = FileInfo {
            path: PathBuf::from("/a.txt"),
            name: "a.txt".to_string(),
            extension: Some("txt".to_string()),
            size: 100,
            modified: Utc::now(),
            sha256: Some("hash1".to_string()),
            perceptual_hash: None,
        };

        let file2 = FileInfo {
            path: PathBuf::from("/b.txt"),
            name: "b.txt".to_string(),
            extension: Some("txt".to_string()),
            size: 100,
            modified: Utc::now(),
            sha256: Some("hash1".to_string()),
            perceptual_hash: None,
        };

        let group = DuplicateGroup {
            id: "group1".to_string(),
            match_type: DuplicateType::Exact,
            files: vec![file1, file2],
            similarity: 1.0,
            wasted_bytes: 100,
        };

        assert_eq!(group.files.len(), 2);
        assert_eq!(group.match_type, DuplicateType::Exact);
        assert!((group.similarity - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_error_variants() {
        let scan_err = Error::Scan("test error".to_string());
        assert!(scan_err.to_string().contains("Scan error"));

        let hash_err = Error::Hash("test error".to_string());
        assert!(hash_err.to_string().contains("Hash error"));
    }

    #[test]
    fn test_file_info_clone() {
        let original = FileInfo {
            path: PathBuf::from("/test.txt"),
            name: "test.txt".to_string(),
            extension: Some("txt".to_string()),
            size: 256,
            modified: Utc::now(),
            sha256: Some("hash".to_string()),
            perceptual_hash: Some(12345),
        };

        let cloned = original.clone();

        assert_eq!(cloned.path, original.path);
        assert_eq!(cloned.name, original.name);
        assert_eq!(cloned.size, original.size);
        assert_eq!(cloned.sha256, original.sha256);
        assert_eq!(cloned.perceptual_hash, original.perceptual_hash);
    }

    #[test]
    fn test_duplicate_group_clone() {
        let group = DuplicateGroup {
            id: "group1".to_string(),
            match_type: DuplicateType::Perceptual,
            files: vec![],
            similarity: 0.95,
            wasted_bytes: 1000,
        };

        let cloned = group.clone();

        assert_eq!(cloned.id, group.id);
        assert_eq!(cloned.match_type, group.match_type);
        assert_eq!(cloned.similarity, group.similarity);
        assert_eq!(cloned.wasted_bytes, group.wasted_bytes);
    }

    #[test]
    fn test_result_type() {
        let ok_result: Result<i32> = Ok(42);
        assert_eq!(ok_result.unwrap(), 42);

        let err_result: Result<i32> = Err(Error::Scan("test".to_string()));
        assert!(err_result.is_err());
    }
}
