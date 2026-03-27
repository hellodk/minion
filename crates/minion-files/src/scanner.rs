//! High-performance parallel file scanner

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use jwalk::WalkDir;
use rayon::prelude::*;

use crate::{Error, FileInfo, Result};

/// Scan configuration
#[derive(Debug, Clone)]
pub struct ScanConfig {
    /// Root directory to scan
    pub root: PathBuf,

    /// Scan subdirectories recursively
    pub recursive: bool,

    /// Include patterns (glob)
    pub include_patterns: Vec<String>,

    /// Exclude patterns (glob)
    pub exclude_patterns: Vec<String>,

    /// Compute hashes
    pub compute_hashes: bool,

    /// Number of parallel workers
    pub parallelism: usize,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::new(),
            recursive: true,
            include_patterns: vec![],
            exclude_patterns: vec![
                "**/node_modules/**".to_string(),
                "**/.git/**".to_string(),
                "**/target/**".to_string(),
            ],
            compute_hashes: true,
            parallelism: num_cpus::get(),
        }
    }
}

/// Scan progress information
#[derive(Debug, Clone)]
pub struct ScanProgress {
    pub files_found: usize,
    pub files_processed: usize,
    pub bytes_processed: u64,
    pub errors: usize,
}

/// Scanner result
pub struct ScanResult {
    pub files: Vec<FileInfo>,
    pub total_size: u64,
    pub error_count: usize,
}

/// High-performance parallel file scanner
pub struct Scanner {
    config: ScanConfig,
    files_found: Arc<AtomicUsize>,
    files_processed: Arc<AtomicUsize>,
    bytes_processed: Arc<AtomicUsize>,
    errors: Arc<AtomicUsize>,
}

impl Scanner {
    /// Create a new scanner with the given configuration
    pub fn new(config: ScanConfig) -> Self {
        Self {
            config,
            files_found: Arc::new(AtomicUsize::new(0)),
            files_processed: Arc::new(AtomicUsize::new(0)),
            bytes_processed: Arc::new(AtomicUsize::new(0)),
            errors: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get the files_found atomic counter (for external progress monitoring)
    pub fn files_found(&self) -> Arc<AtomicUsize> {
        self.files_found.clone()
    }

    /// Get the files_processed atomic counter (for external progress monitoring)
    pub fn files_processed(&self) -> Arc<AtomicUsize> {
        self.files_processed.clone()
    }

    /// Get the bytes_processed atomic counter (for external progress monitoring)
    pub fn bytes_processed(&self) -> Arc<AtomicUsize> {
        self.bytes_processed.clone()
    }

    /// Get current progress
    pub fn progress(&self) -> ScanProgress {
        ScanProgress {
            files_found: self.files_found.load(Ordering::Relaxed),
            files_processed: self.files_processed.load(Ordering::Relaxed),
            bytes_processed: self.bytes_processed.load(Ordering::Relaxed) as u64,
            errors: self.errors.load(Ordering::Relaxed),
        }
    }

    /// Run the scan
    pub fn scan(&self) -> Result<ScanResult> {
        let root = &self.config.root;

        if !root.exists() {
            return Err(Error::Scan(format!(
                "Path does not exist: {}",
                root.display()
            )));
        }

        if !root.is_dir() {
            return Err(Error::Scan(format!(
                "Path is not a directory: {}",
                root.display()
            )));
        }

        tracing::info!("Starting scan of {}", root.display());

        // Collect file paths first
        let entries: Vec<PathBuf> = WalkDir::new(root)
            .skip_hidden(false)
            .follow_links(false)
            .parallelism(jwalk::Parallelism::RayonNewPool(self.config.parallelism))
            .into_iter()
            .filter_map(|entry| match entry {
                Ok(e) => {
                    if e.file_type().is_file() {
                        self.files_found.fetch_add(1, Ordering::Relaxed);
                        Some(e.path())
                    } else {
                        None
                    }
                }
                Err(e) => {
                    tracing::warn!("Error reading entry: {}", e);
                    self.errors.fetch_add(1, Ordering::Relaxed);
                    None
                }
            })
            .collect();

        tracing::info!("Found {} files", entries.len());

        // Process files in parallel
        let _files_found = self.files_found.clone();
        let files_processed = self.files_processed.clone();
        let bytes_processed = self.bytes_processed.clone();
        let errors = self.errors.clone();
        let compute_hashes = self.config.compute_hashes;

        let files: Vec<FileInfo> = entries
            .par_iter()
            .filter_map(|path| match Self::process_file(path, compute_hashes) {
                Ok(info) => {
                    files_processed.fetch_add(1, Ordering::Relaxed);
                    bytes_processed.fetch_add(info.size as usize, Ordering::Relaxed);
                    Some(info)
                }
                Err(e) => {
                    tracing::warn!("Error processing {}: {}", path.display(), e);
                    errors.fetch_add(1, Ordering::Relaxed);
                    None
                }
            })
            .collect();

        let total_size = files.iter().map(|f| f.size).sum();

        tracing::info!("Scan complete: {} files, {} bytes", files.len(), total_size);

        Ok(ScanResult {
            files,
            total_size,
            error_count: self.errors.load(Ordering::Relaxed),
        })
    }

    /// Process a single file
    fn process_file(path: &Path, compute_hash: bool) -> Result<FileInfo> {
        let metadata = std::fs::metadata(path)?;

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let extension = path.extension().map(|e| e.to_string_lossy().to_string());

        let modified = metadata
            .modified()
            .map(chrono::DateTime::<chrono::Utc>::from)
            .unwrap_or_else(|_| chrono::Utc::now());

        let sha256 = if compute_hash {
            Some(crate::hash::compute_sha256(path)?)
        } else {
            None
        };

        Ok(FileInfo {
            path: path.to_path_buf(),
            name,
            extension,
            size: metadata.len(),
            modified,
            sha256,
            perceptual_hash: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_scan_config_default() {
        let config = ScanConfig::default();

        assert!(config.recursive);
        assert!(config.compute_hashes);
        assert!(config.parallelism > 0);
        assert!(config
            .exclude_patterns
            .contains(&"**/node_modules/**".to_string()));
        assert!(config.exclude_patterns.contains(&"**/.git/**".to_string()));
    }

    #[test]
    fn test_scanner() {
        let dir = tempdir().unwrap();

        // Create some test files
        for i in 0..5 {
            let path = dir.path().join(format!("file{}.txt", i));
            let mut file = File::create(&path).unwrap();
            writeln!(file, "Content {}", i).unwrap();
        }

        let config = ScanConfig {
            root: dir.path().to_path_buf(),
            compute_hashes: true,
            ..Default::default()
        };

        let scanner = Scanner::new(config);
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 5);
        assert!(result.files[0].sha256.is_some());
    }

    #[test]
    fn test_scanner_without_hashes() {
        let dir = tempdir().unwrap();

        let path = dir.path().join("test.txt");
        let mut file = File::create(&path).unwrap();
        writeln!(file, "Test content").unwrap();

        let config = ScanConfig {
            root: dir.path().to_path_buf(),
            compute_hashes: false,
            ..Default::default()
        };

        let scanner = Scanner::new(config);
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].sha256.is_none());
    }

    #[test]
    fn test_scanner_nested_directories() {
        let dir = tempdir().unwrap();

        // Create nested structure
        std::fs::create_dir_all(dir.path().join("a/b/c")).unwrap();

        File::create(dir.path().join("root.txt")).unwrap();
        File::create(dir.path().join("a/level1.txt")).unwrap();
        File::create(dir.path().join("a/b/level2.txt")).unwrap();
        File::create(dir.path().join("a/b/c/level3.txt")).unwrap();

        let config = ScanConfig {
            root: dir.path().to_path_buf(),
            recursive: true,
            compute_hashes: false,
            ..Default::default()
        };

        let scanner = Scanner::new(config);
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 4);
    }

    #[test]
    fn test_scanner_nonexistent_path() {
        let config = ScanConfig {
            root: PathBuf::from("/nonexistent/path/that/does/not/exist"),
            ..Default::default()
        };

        let scanner = Scanner::new(config);
        let result = scanner.scan();

        assert!(result.is_err());
    }

    #[test]
    fn test_scanner_file_path_instead_of_directory() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        File::create(&file_path).unwrap();

        let config = ScanConfig {
            root: file_path,
            ..Default::default()
        };

        let scanner = Scanner::new(config);
        let result = scanner.scan();

        assert!(result.is_err());
    }

    #[test]
    fn test_scanner_progress() {
        let dir = tempdir().unwrap();

        for i in 0..3 {
            let path = dir.path().join(format!("file{}.txt", i));
            File::create(&path).unwrap();
        }

        let config = ScanConfig {
            root: dir.path().to_path_buf(),
            compute_hashes: false,
            ..Default::default()
        };

        let scanner = Scanner::new(config);

        // Check initial progress
        let progress = scanner.progress();
        assert_eq!(progress.files_found, 0);
        assert_eq!(progress.files_processed, 0);

        // After scan
        let _result = scanner.scan().unwrap();
        let progress = scanner.progress();
        assert!(progress.files_found >= 3);
        assert!(progress.files_processed >= 3);
    }

    #[test]
    fn test_scanner_empty_directory() {
        let dir = tempdir().unwrap();

        let config = ScanConfig {
            root: dir.path().to_path_buf(),
            ..Default::default()
        };

        let scanner = Scanner::new(config);
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 0);
        assert_eq!(result.total_size, 0);
    }

    #[test]
    fn test_scanner_total_size() {
        let dir = tempdir().unwrap();

        let path = dir.path().join("test.txt");
        let mut file = File::create(&path).unwrap();
        file.write_all(b"12345").unwrap(); // 5 bytes

        let config = ScanConfig {
            root: dir.path().to_path_buf(),
            compute_hashes: false,
            ..Default::default()
        };

        let scanner = Scanner::new(config);
        let result = scanner.scan().unwrap();

        assert_eq!(result.total_size, 5);
    }

    #[test]
    fn test_scan_result_fields() {
        let dir = tempdir().unwrap();

        for i in 0..2 {
            let path = dir.path().join(format!("file{}.txt", i));
            let mut file = File::create(&path).unwrap();
            file.write_all(b"test").unwrap();
        }

        let config = ScanConfig {
            root: dir.path().to_path_buf(),
            compute_hashes: false,
            ..Default::default()
        };

        let scanner = Scanner::new(config);
        let result = scanner.scan().unwrap();

        assert_eq!(result.files.len(), 2);
        assert_eq!(result.total_size, 8); // 4 bytes * 2 files
        assert_eq!(result.error_count, 0);
    }

    #[test]
    fn test_process_file_extracts_metadata() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.txt");

        let mut file = File::create(&path).unwrap();
        file.write_all(b"Hello, World!").unwrap();

        let file_info = Scanner::process_file(&path, false).unwrap();

        assert_eq!(file_info.name, "test.txt");
        assert_eq!(file_info.extension, Some("txt".to_string()));
        assert_eq!(file_info.size, 13);
        assert!(file_info.sha256.is_none());
    }

    #[test]
    fn test_process_file_with_hash() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.txt");

        let mut file = File::create(&path).unwrap();
        file.write_all(b"Test content").unwrap();

        let file_info = Scanner::process_file(&path, true).unwrap();

        assert!(file_info.sha256.is_some());
        assert_eq!(file_info.sha256.as_ref().unwrap().len(), 64); // SHA-256 hex
    }
}
