//! Duplicate file detection

use std::collections::{HashMap, HashSet};

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

/// Normalize a filename for fuzzy comparison.
/// "My Video (1080p) [2024].mp4" -> "my video 1080p 2024"
fn normalize_filename(name: &str) -> String {
    let stem = std::path::Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);

    // Remove common suffixes: (1), (2), (copy), _copy, - Copy
    let cleaned = stem
        .to_lowercase()
        .replace(['(', ')', '[', ']', '{', '}', '_', '-'], " ")
        .split_whitespace()
        .filter(|w| !matches!(*w, "copy" | "1" | "2" | "3" | "4" | "5"))
        .collect::<Vec<_>>()
        .join(" ");

    cleaned.trim().to_string()
}

/// Calculate similarity between two strings (0.0 to 1.0).
/// Uses trigram similarity (simple, no deps needed).
fn string_similarity(a: &str, b: &str) -> f32 {
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let trigrams_a: HashSet<&str> = (0..a.len().saturating_sub(2))
        .filter_map(|i| a.get(i..i + 3))
        .collect();
    let trigrams_b: HashSet<&str> = (0..b.len().saturating_sub(2))
        .filter_map(|i| b.get(i..i + 3))
        .collect();

    if trigrams_a.is_empty() || trigrams_b.is_empty() {
        return 0.0;
    }

    let intersection = trigrams_a.intersection(&trigrams_b).count();
    let union = trigrams_a.union(&trigrams_b).count();

    if union == 0 {
        return 0.0;
    }
    intersection as f32 / union as f32
}

/// Find duplicates by fuzzy filename matching.
pub fn find_fuzzy_name_duplicates(files: &[FileInfo], threshold: f32) -> Vec<DuplicateGroup> {
    let mut groups: Vec<DuplicateGroup> = Vec::new();
    let mut processed = vec![false; files.len()];

    // Pre-compute normalized names
    let normalized: Vec<String> = files.iter().map(|f| normalize_filename(&f.name)).collect();

    for i in 0..files.len() {
        if processed[i] || normalized[i].len() < 3 {
            continue;
        }

        let mut group_files = vec![files[i].clone()];
        processed[i] = true;
        let mut best_similarity = 0.0f32;

        for j in (i + 1)..files.len() {
            if processed[j] {
                continue;
            }

            let sim = string_similarity(&normalized[i], &normalized[j]);
            if sim >= threshold {
                group_files.push(files[j].clone());
                processed[j] = true;
                if sim > best_similarity {
                    best_similarity = sim;
                }
            }
        }

        if group_files.len() > 1 {
            let total_size: u64 = group_files.iter().map(|f| f.size).sum();
            let avg_size = total_size / group_files.len() as u64;

            groups.push(DuplicateGroup {
                id: uuid::Uuid::new_v4().to_string(),
                match_type: DuplicateType::Near,
                files: group_files,
                similarity: best_similarity,
                wasted_bytes: total_size - avg_size,
            });
        }
    }

    groups
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
    pub enable_fuzzy_name: bool,

    /// Threshold for fuzzy filename similarity (0.0-1.0, default 0.6)
    pub fuzzy_threshold: f32,
}

impl Default for DuplicateFinder {
    fn default() -> Self {
        Self {
            min_size: 1024,          // 1KB minimum
            perceptual_threshold: 8, // ~12% difference allowed
            enable_exact: true,
            enable_perceptual: true,
            enable_fuzzy_name: true,
            fuzzy_threshold: 0.6,
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

        if self.enable_fuzzy_name {
            let fuzzy_groups = find_fuzzy_name_duplicates(
                &candidates.iter().copied().cloned().collect::<Vec<_>>(),
                self.fuzzy_threshold,
            );
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

    #[test]
    fn test_normalize_filename_removes_brackets_and_noise() {
        assert_eq!(
            normalize_filename("My Video (1080p) [2024].mp4"),
            "my video 1080p 2024"
        );
        assert_eq!(
            normalize_filename("document_final_copy.pdf"),
            "document final"
        );
        assert_eq!(normalize_filename("photo (1).jpg"), "photo");
        assert_eq!(normalize_filename("report-2024.txt"), "report 2024");
        assert_eq!(normalize_filename("notes {draft}.md"), "notes draft");
    }

    #[test]
    fn test_normalize_filename_no_extension_strip() {
        // Stem extraction should remove extension
        assert_eq!(normalize_filename("README.md"), "readme");
        assert_eq!(normalize_filename("archive.tar.gz"), "archive.tar");
    }

    #[test]
    fn test_normalize_filename_copy_suffixes() {
        assert_eq!(normalize_filename("file_copy.txt"), "file");
        assert_eq!(normalize_filename("file - Copy.txt"), "file");
        assert_eq!(normalize_filename("image (2).png"), "image");
        assert_eq!(normalize_filename("image (3).png"), "image");
    }

    #[test]
    fn test_normalize_filename_empty_and_short() {
        assert_eq!(normalize_filename(""), "");
        assert_eq!(normalize_filename("a"), "a");
        assert_eq!(normalize_filename(".hidden"), ".hidden");
    }

    #[test]
    fn test_string_similarity_identical() {
        assert!((string_similarity("hello world", "hello world") - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_string_similarity_empty() {
        assert!((string_similarity("", "hello")).abs() < 0.001);
        assert!((string_similarity("hello", "")).abs() < 0.001);
        // Two empty strings are identical
        assert!((string_similarity("", "") - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_string_similarity_very_short() {
        // Identical short strings still match via the equality check
        assert!((string_similarity("ab", "ab") - 1.0).abs() < 0.001);
        // Different short strings with no trigrams return 0.0
        assert!((string_similarity("ab", "cd")).abs() < 0.001);
    }

    #[test]
    fn test_string_similarity_known_pairs() {
        // Similar strings should have high similarity
        let sim = string_similarity("my document final", "my document draft");
        assert!(sim > 0.3, "Expected > 0.3, got {}", sim);

        // Very different strings should have low similarity
        let sim = string_similarity("vacation photo", "quarterly report");
        assert!(sim < 0.2, "Expected < 0.2, got {}", sim);
    }

    #[test]
    fn test_string_similarity_near_identical() {
        let sim = string_similarity("project report 2024", "project report 2023");
        assert!(
            sim > 0.6,
            "Expected > 0.6 for near-identical strings, got {}",
            sim
        );
    }

    fn make_file(name: &str, size: u64) -> FileInfo {
        FileInfo {
            path: std::path::PathBuf::from(format!("/test/{}", name)),
            name: name.to_string(),
            extension: std::path::Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_string()),
            size,
            modified: chrono::Utc::now(),
            sha256: None,
            perceptual_hash: None,
        }
    }

    #[test]
    fn test_find_fuzzy_name_duplicates_basic() {
        let files = vec![
            make_file("vacation_photo.jpg", 5000),
            make_file("vacation_photo (1).jpg", 5000),
            make_file("vacation_photo_copy.jpg", 5000),
            make_file("totally_different.txt", 200),
        ];

        let groups = find_fuzzy_name_duplicates(&files, 0.6);
        assert_eq!(
            groups.len(),
            1,
            "Expected 1 fuzzy group, got {}",
            groups.len()
        );
        assert_eq!(
            groups[0].files.len(),
            3,
            "Expected 3 files in group, got {}",
            groups[0].files.len()
        );
        assert_eq!(groups[0].match_type, DuplicateType::Near);
    }

    #[test]
    fn test_find_fuzzy_name_duplicates_no_match() {
        let files = vec![
            make_file("alpha.txt", 100),
            make_file("beta.txt", 200),
            make_file("gamma.txt", 300),
        ];

        let groups = find_fuzzy_name_duplicates(&files, 0.6);
        assert!(groups.is_empty(), "Expected no groups for dissimilar files");
    }

    #[test]
    fn test_find_fuzzy_name_duplicates_threshold() {
        let files = vec![
            make_file("project_report_2024.pdf", 10000),
            make_file("project_report_2023.pdf", 9500),
        ];

        // Low threshold should match
        let groups_low = find_fuzzy_name_duplicates(&files, 0.4);
        assert_eq!(groups_low.len(), 1, "Low threshold should find a match");

        // Very high threshold should not match
        let groups_high = find_fuzzy_name_duplicates(&files, 0.99);
        assert!(
            groups_high.is_empty(),
            "Very high threshold should find no match"
        );
    }

    #[test]
    fn test_find_fuzzy_name_duplicates_wasted_bytes() {
        let files = vec![
            make_file("document_final.pdf", 1000),
            make_file("document_final (1).pdf", 1200),
        ];

        let groups = find_fuzzy_name_duplicates(&files, 0.5);
        assert_eq!(groups.len(), 1);
        // Total = 2200, avg = 1100, wasted = 2200 - 1100 = 1100
        assert_eq!(groups[0].wasted_bytes, 1100);
    }

    #[test]
    fn test_fuzzy_name_in_duplicate_finder() {
        let files = vec![
            make_file("report_q4.pdf", 2000),
            make_file("report_q4 (1).pdf", 2000),
            make_file("unrelated_spreadsheet.xlsx", 5000),
        ];

        let finder = DuplicateFinder {
            min_size: 100,
            perceptual_threshold: 8,
            enable_exact: false,
            enable_perceptual: false,
            enable_fuzzy_name: true,
            fuzzy_threshold: 0.5,
        };

        let groups = finder.find(&files);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].match_type, DuplicateType::Near);
    }
}
