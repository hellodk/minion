//! Storage analytics

use std::collections::HashMap;

use crate::FileInfo;

/// Storage analytics summary
#[derive(Debug, Clone)]
pub struct StorageAnalytics {
    /// Total number of files
    pub total_files: u64,

    /// Total size in bytes
    pub total_size: u64,

    /// Files grouped by extension
    pub by_extension: HashMap<String, ExtensionStats>,

    /// Files grouped by age
    pub by_age: AgeDistribution,

    /// Largest files
    pub largest_files: Vec<FileInfo>,

    /// Oldest files
    pub oldest_files: Vec<FileInfo>,

    /// Most recently modified files
    pub newest_files: Vec<FileInfo>,
}

/// Statistics for a file extension
#[derive(Debug, Clone, Default)]
pub struct ExtensionStats {
    pub count: u64,
    pub total_size: u64,
    pub avg_size: u64,
    pub largest: u64,
}

/// Age distribution of files
#[derive(Debug, Clone, Default)]
pub struct AgeDistribution {
    pub last_day: FileBucket,
    pub last_week: FileBucket,
    pub last_month: FileBucket,
    pub last_year: FileBucket,
    pub older: FileBucket,
}

/// A bucket of files
#[derive(Debug, Clone, Default)]
pub struct FileBucket {
    pub count: u64,
    pub size: u64,
}

/// Analytics calculator
pub struct AnalyticsCalculator {
    top_n: usize,
}

impl AnalyticsCalculator {
    pub fn new(top_n: usize) -> Self {
        Self { top_n }
    }

    /// Calculate analytics for a set of files
    pub fn calculate(&self, files: &[FileInfo]) -> StorageAnalytics {
        let now = chrono::Utc::now();
        let day_ago = now - chrono::Duration::days(1);
        let week_ago = now - chrono::Duration::weeks(1);
        let month_ago = now - chrono::Duration::days(30);
        let year_ago = now - chrono::Duration::days(365);

        let mut by_extension: HashMap<String, ExtensionStats> = HashMap::new();
        let mut by_age = AgeDistribution::default();
        let mut total_size = 0u64;

        for file in files {
            total_size += file.size;

            // Group by extension
            let ext = file
                .extension
                .clone()
                .unwrap_or_else(|| "no_extension".to_string());
            let stats = by_extension.entry(ext).or_default();
            stats.count += 1;
            stats.total_size += file.size;
            stats.largest = stats.largest.max(file.size);

            // Group by age
            let bucket = if file.modified >= day_ago {
                &mut by_age.last_day
            } else if file.modified >= week_ago {
                &mut by_age.last_week
            } else if file.modified >= month_ago {
                &mut by_age.last_month
            } else if file.modified >= year_ago {
                &mut by_age.last_year
            } else {
                &mut by_age.older
            };
            bucket.count += 1;
            bucket.size += file.size;
        }

        // Calculate averages
        for stats in by_extension.values_mut() {
            if stats.count > 0 {
                stats.avg_size = stats.total_size / stats.count;
            }
        }

        // Find largest files
        let mut sorted_by_size = files.to_vec();
        sorted_by_size.sort_by(|a, b| b.size.cmp(&a.size));
        let largest_files = sorted_by_size.into_iter().take(self.top_n).collect();

        // Find oldest files
        let mut sorted_by_age = files.to_vec();
        sorted_by_age.sort_by(|a, b| a.modified.cmp(&b.modified));
        let oldest_files = sorted_by_age.into_iter().take(self.top_n).collect();

        // Find newest files
        let mut sorted_by_recent = files.to_vec();
        sorted_by_recent.sort_by(|a, b| b.modified.cmp(&a.modified));
        let newest_files = sorted_by_recent.into_iter().take(self.top_n).collect();

        StorageAnalytics {
            total_files: files.len() as u64,
            total_size,
            by_extension,
            by_age,
            largest_files,
            oldest_files,
            newest_files,
        }
    }
}

/// Format bytes as human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let bytes_f = bytes as f64;
    let exp = (bytes_f.ln() / 1024_f64.ln()).floor() as usize;
    let exp = exp.min(UNITS.len() - 1);

    let size = bytes_f / 1024_f64.powi(exp as i32);

    if exp == 0 {
        format!("{} {}", bytes, UNITS[exp])
    } else {
        format!("{:.2} {}", size, UNITS[exp])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }

    #[test]
    fn test_analytics() {
        let files = vec![
            FileInfo {
                path: "/a.txt".into(),
                name: "a.txt".into(),
                extension: Some("txt".into()),
                size: 1000,
                modified: chrono::Utc::now(),
                sha256: None,
                perceptual_hash: None,
            },
            FileInfo {
                path: "/b.jpg".into(),
                name: "b.jpg".into(),
                extension: Some("jpg".into()),
                size: 5000,
                modified: chrono::Utc::now() - chrono::Duration::days(10),
                sha256: None,
                perceptual_hash: None,
            },
        ];

        let calc = AnalyticsCalculator::new(10);
        let analytics = calc.calculate(&files);

        assert_eq!(analytics.total_files, 2);
        assert_eq!(analytics.total_size, 6000);
        assert_eq!(analytics.by_extension.len(), 2);
    }
}
