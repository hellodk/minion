//! MINION Cryptography Module
//!
//! Provides secure encryption, key derivation, and credential storage.

pub mod error;
pub mod keys;
pub mod vault;

pub use error::{Error, Result};
pub use keys::{DerivedKey, MasterKey};
pub use vault::{Credential, CredentialType, CredentialVault};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use zeroize::Zeroize;

/// AES-256-GCM nonce size in bytes
pub const NONCE_SIZE: usize = 12;

/// AES-256-GCM tag size in bytes
pub const TAG_SIZE: usize = 16;

/// Encrypt data using AES-256-GCM
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| Error::Encryption(e.to_string()))?;

    // Generate random nonce
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| Error::Encryption(e.to_string()))?;

    // Prepend nonce to ciphertext
    let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Decrypt data using AES-256-GCM
pub fn decrypt(key: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if ciphertext.len() < NONCE_SIZE + TAG_SIZE {
        return Err(Error::Decryption("Ciphertext too short".to_string()));
    }

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| Error::Decryption(e.to_string()))?;

    // Extract nonce
    let nonce = Nonce::from_slice(&ciphertext[..NONCE_SIZE]);

    // Decrypt
    let plaintext = cipher
        .decrypt(nonce, &ciphertext[NONCE_SIZE..])
        .map_err(|e| Error::Decryption(e.to_string()))?;

    Ok(plaintext)
}

/// Secure string that zeroes memory on drop
#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct SecureString {
    inner: String,
}

impl SecureString {
    pub fn new(s: String) -> Self {
        Self { inner: s }
    }

    pub fn as_str(&self) -> &str {
        &self.inner
    }

    pub fn into_inner(mut self) -> String {
        std::mem::take(&mut self.inner)
    }
}

impl From<String> for SecureString {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for SecureString {
    fn from(s: &str) -> Self {
        Self::new(s.to_string())
    }
}

/// Secure bytes that zero memory on drop
#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct SecureBytes {
    inner: Vec<u8>,
}

impl SecureBytes {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { inner: bytes }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.inner
    }

    pub fn into_inner(mut self) -> Vec<u8> {
        std::mem::take(&mut self.inner)
    }
}

impl From<Vec<u8>> for SecureBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self::new(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = [0u8; 32];
        let plaintext = b"Hello, MINION!";

        let ciphertext = encrypt(&key, plaintext).unwrap();
        let decrypted = decrypt(&key, &ciphertext).unwrap();

        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_empty() {
        let key = [0u8; 32];
        let plaintext = b"";

        let ciphertext = encrypt(&key, plaintext).unwrap();
        let decrypted = decrypt(&key, &ciphertext).unwrap();

        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_large_data() {
        let key = [0u8; 32];
        let plaintext: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();

        let ciphertext = encrypt(&key, &plaintext).unwrap();
        let decrypted = decrypt(&key, &ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_produces_different_ciphertext() {
        let key = [0u8; 32];
        let plaintext = b"Same message";

        // Due to random nonce, each encryption should produce different ciphertext
        let ciphertext1 = encrypt(&key, plaintext).unwrap();
        let ciphertext2 = encrypt(&key, plaintext).unwrap();

        assert_ne!(ciphertext1, ciphertext2);

        // But both should decrypt to the same plaintext
        let decrypted1 = decrypt(&key, &ciphertext1).unwrap();
        let decrypted2 = decrypt(&key, &ciphertext2).unwrap();

        assert_eq!(decrypted1, decrypted2);
    }

    #[test]
    fn test_decrypt_wrong_key() {
        let key1 = [0u8; 32];
        let key2 = [1u8; 32];
        let plaintext = b"Secret message";

        let ciphertext = encrypt(&key1, plaintext).unwrap();
        let result = decrypt(&key2, &ciphertext);

        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_corrupted_ciphertext() {
        let key = [0u8; 32];
        let plaintext = b"Secret message";

        let mut ciphertext = encrypt(&key, plaintext).unwrap();

        // Corrupt the ciphertext
        if let Some(byte) = ciphertext.last_mut() {
            *byte ^= 0xFF;
        }

        let result = decrypt(&key, &ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_too_short() {
        let key = [0u8; 32];
        let short_ciphertext = vec![0u8; NONCE_SIZE + TAG_SIZE - 1];

        let result = decrypt(&key, &short_ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_ciphertext_length() {
        let key = [0u8; 32];
        let plaintext = b"Test message";

        let ciphertext = encrypt(&key, plaintext).unwrap();

        // Ciphertext = NONCE + plaintext + TAG
        assert_eq!(ciphertext.len(), NONCE_SIZE + plaintext.len() + TAG_SIZE);
    }

    #[test]
    fn test_secure_string_new() {
        let secret = SecureString::new("secret password".to_string());
        assert_eq!(secret.as_str(), "secret password");
    }

    #[test]
    fn test_secure_string_from_string() {
        let secret: SecureString = "test".to_string().into();
        assert_eq!(secret.as_str(), "test");
    }

    #[test]
    fn test_secure_string_from_str() {
        let secret: SecureString = "test".into();
        assert_eq!(secret.as_str(), "test");
    }

    #[test]
    fn test_secure_string_into_inner() {
        let secret = SecureString::new("secret".to_string());
        let inner = secret.into_inner();
        assert_eq!(inner, "secret");
    }

    #[test]
    fn test_secure_string_clone() {
        let secret = SecureString::new("secret".to_string());
        let cloned = secret.clone();
        assert_eq!(cloned.as_str(), "secret");
    }

    #[test]
    fn test_secure_bytes_new() {
        let bytes = SecureBytes::new(vec![1, 2, 3, 4]);
        assert_eq!(bytes.as_slice(), &[1, 2, 3, 4]);
    }

    #[test]
    fn test_secure_bytes_from_vec() {
        let bytes: SecureBytes = vec![5, 6, 7, 8].into();
        assert_eq!(bytes.as_slice(), &[5, 6, 7, 8]);
    }

    #[test]
    fn test_secure_bytes_into_inner() {
        let bytes = SecureBytes::new(vec![1, 2, 3]);
        let inner = bytes.into_inner();
        assert_eq!(inner, vec![1, 2, 3]);
    }

    #[test]
    fn test_secure_bytes_clone() {
        let bytes = SecureBytes::new(vec![1, 2, 3]);
        let cloned = bytes.clone();
        assert_eq!(cloned.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_nonce_size_constant() {
        assert_eq!(NONCE_SIZE, 12);
    }

    #[test]
    fn test_tag_size_constant() {
        assert_eq!(TAG_SIZE, 16);
    }
}
