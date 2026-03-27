//! File hashing utilities

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use crate::{Error, Result};

/// Buffer size for streaming hash computation (64KB)
const BUFFER_SIZE: usize = 64 * 1024;

/// Compute SHA-256 hash of a file
pub fn compute_sha256(path: &Path) -> Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; BUFFER_SIZE];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hex::encode(hasher.finalize()))
}

/// Compute BLAKE3 hash of a file (faster than SHA-256)
pub fn compute_blake3(path: &Path) -> Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; BUFFER_SIZE];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

/// Compute perceptual hash for an image
pub fn compute_image_phash(path: &Path) -> Result<u64> {
    let img = image::open(path).map_err(|e| Error::Hash(e.to_string()))?;

    // Resize to 32x32 for hash computation
    let thumbnail = img.resize_exact(32, 32, image::imageops::FilterType::Lanczos3);
    let gray = thumbnail.to_luma8();

    // Compute average
    let pixels: Vec<f32> = gray.pixels().map(|p| p.0[0] as f32).collect();
    let avg: f32 = pixels.iter().sum::<f32>() / pixels.len() as f32;

    // Generate hash
    let mut hash: u64 = 0;
    for (i, &pixel) in pixels.iter().enumerate().take(64) {
        if pixel > avg {
            hash |= 1 << i;
        }
    }

    Ok(hash)
}

/// Compute Hamming distance between two perceptual hashes
pub fn hamming_distance(hash1: u64, hash2: u64) -> u32 {
    (hash1 ^ hash2).count_ones()
}

/// Check if two perceptual hashes are similar (threshold-based)
pub fn is_similar(hash1: u64, hash2: u64, threshold: u32) -> bool {
    hamming_distance(hash1, hash2) <= threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_sha256() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Hello, World!").unwrap();

        let hash = compute_sha256(file.path()).unwrap();
        assert_eq!(hash.len(), 64); // SHA-256 produces 32 bytes = 64 hex chars
    }

    #[test]
    fn test_blake3() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Hello, World!").unwrap();

        let hash = compute_blake3(file.path()).unwrap();
        assert_eq!(hash.len(), 64); // BLAKE3 default output is 32 bytes
    }

    #[test]
    fn test_hamming_distance() {
        assert_eq!(hamming_distance(0, 0), 0);
        assert_eq!(hamming_distance(0b1111, 0b0000), 4);
        assert_eq!(hamming_distance(0b1010, 0b0101), 4);
    }
}
