//! Cryptography error types

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Decryption error: {0}")]
    Decryption(String),

    #[error("Key derivation error: {0}")]
    KeyDerivation(String),

    #[error("Invalid key length")]
    InvalidKeyLength,

    #[error("Vault error: {0}")]
    Vault(String),

    #[error("Credential not found: {0}")]
    CredentialNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_error() {
        let err = Error::Encryption("test encryption error".to_string());
        assert!(err.to_string().contains("Encryption error"));
        assert!(err.to_string().contains("test encryption error"));
    }

    #[test]
    fn test_decryption_error() {
        let err = Error::Decryption("test decryption error".to_string());
        assert!(err.to_string().contains("Decryption error"));
    }

    #[test]
    fn test_key_derivation_error() {
        let err = Error::KeyDerivation("derivation failed".to_string());
        assert!(err.to_string().contains("Key derivation error"));
    }

    #[test]
    fn test_invalid_key_length_error() {
        let err = Error::InvalidKeyLength;
        assert!(err.to_string().contains("Invalid key length"));
    }

    #[test]
    fn test_vault_error() {
        let err = Error::Vault("vault error".to_string());
        assert!(err.to_string().contains("Vault error"));
    }

    #[test]
    fn test_credential_not_found_error() {
        let err = Error::CredentialNotFound("github".to_string());
        assert!(err.to_string().contains("Credential not found"));
        assert!(err.to_string().contains("github"));
    }

    #[test]
    fn test_result_type() {
        let ok_result: Result<i32> = Ok(42);
        assert_eq!(ok_result.unwrap(), 42);

        let err_result: Result<i32> = Err(Error::InvalidKeyLength);
        assert!(err_result.is_err());
    }

    #[test]
    fn test_error_debug() {
        let err = Error::Encryption("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Encryption"));
    }
}
