//! Duplicate file detection

use std::collections::HashMap;

use crate::{DuplicateGroup, DuplicateType, FileInfo};

/// Find exact duplicates (same SHA-256 hash)
pub fn find_exact_duplicates(files: &[FileInfo]) -> Vec<DuplicateGroup> {
    // Group by hash
    let mut hash_groups: HashMap<&str, Vec<&FileInfo>> = HashMap::new();

    for file in files {
        if let Some(ref hash) = file.sha256 {
            hash_groups.entry(hash.as_str()).or_default().push(file);
        }
    }

    // Filter to groups with duplicates
    hash_groups
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .map(|(_hash, files)| {
            let total_size: u64 = files.iter().map(|f| f.size).sum();
            let wasted = total_size - files[0].size; // All but one copy is "wasted"

            DuplicateGroup {
                id: uuid::Uuid::new_v4().to_string(),
                match_type: DuplicateType::Exact,
                files: files.into_iter().cloned().collect(),
                similarity: 1.0,
                wasted_bytes: wasted,
            }
        })
        .collect()
}

/// Find near-duplicates using perceptual hashing (for images)
pub fn find_perceptual_duplicates(files: &[FileInfo], threshold: u32) -> Vec<DuplicateGroup> {
    let mut groups: Vec<DuplicateGroup> = Vec::new();
    let mut processed: Vec<bool> = vec![false; files.len()];

    // Only consider files with perceptual hashes
    let files_with_phash: Vec<(usize, &FileInfo, u64)> = files
        .iter()
        .enumerate()
        .filter_map(|(i, f)| f.perceptual_hash.map(|h| (i, f, h)))
        .collect();

    for (i, (idx_i, file_i, hash_i)) in files_with_phash.iter().enumerate() {
        if processed[*idx_i] {
            continue;
        }

        let mut group_files = vec![(*file_i).clone()];
        processed[*idx_i] = true;

        for (idx_j, file_j, hash_j) in files_with_phash.iter().skip(i + 1) {
            if processed[*idx_j] {
                continue;
            }

            let distance = crate::hash::hamming_distance(*hash_i, *hash_j);
            if distance <= threshold {
                group_files.push((*file_j).clone());
                processed[*idx_j] = true;
            }
        }

        if group_files.len() > 1 {
            let total_size: u64 = group_files.iter().map(|f| f.size).sum();
            let avg_size = total_size / group_files.len() as u64;
            let wasted = total_size - avg_size;

            // Calculate similarity based on average Hamming distance
            let similarity = 1.0 - (threshold as f32 / 64.0);

            groups.push(DuplicateGroup {
                id: uuid::Uuid::new_v4().to_string(),
                match_type: DuplicateType::Perceptual,
                files: group_files,
                similarity,
                wasted_bytes: wasted,
            });
        }
    }

    groups
}

/// Find duplicates by size (quick pre-filter)
pub fn find_size_candidates(files: &[FileInfo]) -> HashMap<u64, Vec<&FileInfo>> {
    let mut size_groups: HashMap<u64, Vec<&FileInfo>> = HashMap::new();

    for file in files {
        if file.size > 0 {
            size_groups.entry(file.size).or_default().push(file);
        }
    }

    // Keep only groups with potential duplicates
    size_groups.retain(|_, files| files.len() > 1);

    size_groups
}

/// Normalize a filename for fuzzy comparison
fn normalize_filename(name: &str) -> String {
    let stem = std::path::Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);

    let mut normalized = stem.to_lowercase().trim().to_string();

    // Remove trailing " (N)" or " (copy)" pattern
    if let Some(pos) = normalized.rfind(" (") {
        if normalized.ends_with(')') {
            let between = &normalized[pos + 2..normalized.len() - 1];
            if between.chars().all(|c| c.is_ascii_digit()) || between == "copy" {
                normalized = normalized[..pos].to_string();
            }
        }
    }

    // Remove trailing "_N" or "-N" where N is digits
    if let Some(pos) = normalized.rfind('_').or_else(|| normalized.rfind('-')) {
        let after = &normalized[pos + 1..];
        if !after.is_empty() && after.chars().all(|c| c.is_ascii_digit()) {
            normalized = normalized[..pos].to_string();
        }
    }

    // Remove " copy" / "_copy" / "-copy" / " - copy" suffix
    for suffix in &[" - copy", " copy", "_copy", "-copy"] {
        if normalized.ends_with(suffix) {
            normalized = normalized[..normalized.len() - suffix.len()].to_string();
        }
    }

    // Normalize separators and whitespace for comparison
    normalized = normalized
        .replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    normalized
}

/// Find fuzzy filename duplicates by grouping files with similar normalized names
pub fn find_fuzzy_name_duplicates(files: &[FileInfo]) -> Vec<DuplicateGroup> {
    let mut name_groups: HashMap<String, Vec<&FileInfo>> = HashMap::new();

    for file in files {
        let normalized = normalize_filename(&file.name);
        if !normalized.is_empty() {
            name_groups.entry(normalized).or_default().push(file);
        }
    }

    name_groups
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .map(|(_, files)| {
            let total_size: u64 = files.iter().map(|f| f.size).sum();
            let max_size = files.iter().map(|f| f.size).max().unwrap_or(0);
            let wasted = total_size - max_size;

            DuplicateGroup {
                id: uuid::Uuid::new_v4().to_string(),
                match_type: DuplicateType::Near,
                files: files.into_iter().cloned().collect(),
                similarity: 0.9,
                wasted_bytes: wasted,
            }
        })
        .collect()
}

/// Duplicate finder with configurable strategies
pub struct DuplicateFinder {
    /// Minimum file size to consider
    pub min_size: u64,

    /// Threshold for perceptual hash similarity (0-64)
    pub perceptual_threshold: u32,

    /// Enable exact hash matching
    pub enable_exact: bool,

    /// Enable perceptual matching for images
    pub enable_perceptual: bool,

    /// Enable fuzzy filename matching
    pub enable_fuzzy_names: bool,
}

impl Default for DuplicateFinder {
    fn default() -> Self {
        Self {
            min_size: 1024,          // 1KB minimum
            perceptual_threshold: 8, // ~12% difference allowed
            enable_exact: true,
            enable_perceptual: true,
            enable_fuzzy_names: true,
        }
    }
}

impl DuplicateFinder {
    /// Find all duplicates in the given files
    pub fn find(&self, files: &[FileInfo]) -> Vec<DuplicateGroup> {
        let mut all_groups = Vec::new();

        // Filter by minimum size
        let candidates: Vec<&FileInfo> = files.iter().filter(|f| f.size >= self.min_size).collect();

        if self.enable_exact {
            // Use size as pre-filter for exact matching
            let files_for_size: Vec<FileInfo> = candidates.iter().copied().cloned().collect();
            let size_candidates = find_size_candidates(&files_for_size);

            for (_, group) in size_candidates {
                let group_files: Vec<FileInfo> = group.into_iter().cloned().collect();
                let duplicates = find_exact_duplicates(&group_files);
                all_groups.extend(duplicates);
            }
        }

        if self.enable_perceptual {
            // Perceptual matching for images
            let image_files: Vec<FileInfo> = candidates
                .iter()
                .filter(|f| is_image_file(f))
                .cloned()
                .cloned()
                .collect();

            let perceptual_groups =
                find_perceptual_duplicates(&image_files, self.perceptual_threshold);
            all_groups.extend(perceptual_groups);
        }

        if self.enable_fuzzy_names {
            // Collect paths already grouped by exact/perceptual matching
            let mut already_grouped: std::collections::HashSet<&std::path::Path> =
                std::collections::HashSet::new();
            for group in &all_groups {
                for f in &group.files {
                    already_grouped.insert(&f.path);
                }
            }

            // Only run fuzzy matching on files not already in a group
            let ungrouped: Vec<FileInfo> = candidates
                .iter()
                .filter(|f| !already_grouped.contains(f.path.as_path()))
                .cloned()
                .cloned()
                .collect();

            let fuzzy_groups = find_fuzzy_name_duplicates(&ungrouped);
            all_groups.extend(fuzzy_groups);
        }

        all_groups
    }
}

/// Check if a file is an image based on extension
fn is_image_file(file: &FileInfo) -> bool {
    match file.extension.as_deref() {
        Some(ext) => matches!(
            ext.to_lowercase().as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tiff" | "tif"
        ),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_filename_copy_suffix() {
        // "video (1).mp4" and "video.mp4" -> same normalized name
        assert_eq!(normalize_filename("video (1).mp4"), normalize_filename("video.mp4"));
        assert_eq!(normalize_filename("video (1).mp4"), "video");
    }

    #[test]
    fn test_normalize_filename_underscore_copy() {
        // "photo_copy.jpg" and "photo.jpg" -> same
        assert_eq!(normalize_filename("photo_copy.jpg"), normalize_filename("photo.jpg"));
        assert_eq!(normalize_filename("photo_copy.jpg"), "photo");
    }

    #[test]
    fn test_normalize_filename_trailing_number() {
        // "document_2.pdf" and "document.pdf" -> same
        assert_eq!(normalize_filename("document_2.pdf"), normalize_filename("document.pdf"));
        assert_eq!(normalize_filename("document_2.pdf"), "document");
    }

    #[test]
    fn test_normalize_filename_dash_copy() {
        // "song - Copy.mp3" and "song.mp3" -> same
        assert_eq!(
            normalize_filename("song - Copy.mp3"),
            normalize_filename("song.mp3")
        );
        assert_eq!(normalize_filename("song - Copy.mp3"), "song");
    }

    #[test]
    fn test_normalize_filename_parenthetical_number() {
        // "report (2).docx" and "report.docx" -> same
        assert_eq!(
            normalize_filename("report (2).docx"),
            normalize_filename("report.docx")
        );
    }

    #[test]
    fn test_normalize_filename_dash_number() {
        // "image-3.png" and "image.png" -> same
        assert_eq!(
            normalize_filename("image-3.png"),
            normalize_filename("image.png")
        );
    }

    #[test]
    fn test_normalize_filename_distinct_files() {
        // Truly different files should not match
        assert_ne!(normalize_filename("report.pdf"), normalize_filename("invoice.pdf"));
        assert_ne!(normalize_filename("photo1.jpg"), normalize_filename("photo2.jpg"));
    }

    #[test]
    fn test_find_fuzzy_name_duplicates_basic() {
        let files = vec![
            FileInfo {
                path: "/dir/video.mp4".into(),
                name: "video.mp4".into(),
                extension: Some("mp4".into()),
                size: 1000,
                modified: chrono::Utc::now(),
                sha256: None,
                perceptual_hash: None,
            },
            FileInfo {
                path: "/dir/video (1).mp4".into(),
                name: "video (1).mp4".into(),
                extension: Some("mp4".into()),
                size: 1000,
                modified: chrono::Utc::now(),
                sha256: None,
                perceptual_hash: None,
            },
        ];

        let groups = find_fuzzy_name_duplicates(&files);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
        assert_eq!(groups[0].match_type, DuplicateType::Near);
    }

    #[test]
    fn test_find_fuzzy_name_duplicates_no_match() {
        let files = vec![
            FileInfo {
                path: "/dir/readme.txt".into(),
                name: "readme.txt".into(),
                extension: Some("txt".into()),
                size: 100,
                modified: chrono::Utc::now(),
                sha256: None,
                perceptual_hash: None,
            },
            FileInfo {
                path: "/dir/license.txt".into(),
                name: "license.txt".into(),
                extension: Some("txt".into()),
                size: 200,
                modified: chrono::Utc::now(),
                sha256: None,
                perceptual_hash: None,
            },
        ];

        let groups = find_fuzzy_name_duplicates(&files);
        assert_eq!(groups.len(), 0);
    }

    #[test]
    fn test_find_exact_duplicates() {
        let files = vec![
            FileInfo {
                path: "/a.txt".into(),
                name: "a.txt".into(),
                extension: Some("txt".into()),
                size: 100,
                modified: chrono::Utc::now(),
                sha256: Some("abc123".into()),
                perceptual_hash: None,
            },
            FileInfo {
                path: "/b.txt".into(),
                name: "b.txt".into(),
                extension: Some("txt".into()),
                size: 100,
                modified: chrono::Utc::now(),
                sha256: Some("abc123".into()), // Same hash
                perceptual_hash: None,
            },
            FileInfo {
                path: "/c.txt".into(),
                name: "c.txt".into(),
                extension: Some("txt".into()),
                size: 200,
                modified: chrono::Utc::now(),
                sha256: Some("xyz789".into()), // Different hash
                perceptual_hash: None,
            },
        ];

        let duplicates = find_exact_duplicates(&files);
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0].files.len(), 2);
    }
}
