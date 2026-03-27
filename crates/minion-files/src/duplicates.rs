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
}

impl Default for DuplicateFinder {
    fn default() -> Self {
        Self {
            min_size: 1024,          // 1KB minimum
            perceptual_threshold: 8, // ~12% difference allowed
            enable_exact: true,
            enable_perceptual: true,
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
}
