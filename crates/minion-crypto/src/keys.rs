//! Key derivation and management

use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHasher,
};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{Error, Result};

/// Argon2id parameters for key derivation
const ARGON2_MEMORY_KB: u32 = 65536; // 64MB
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;

/// Master key derived from user password
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct MasterKey {
    key: [u8; 32],
    salt: String,
}

impl MasterKey {
    /// Derive a master key from a password
    pub fn derive(password: &str) -> Result<Self> {
        let salt = SaltString::generate(&mut OsRng);

        let argon2 = Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            argon2::Params::new(
                ARGON2_MEMORY_KB,
                ARGON2_ITERATIONS,
                ARGON2_PARALLELISM,
                Some(32),
            )
            .map_err(|e| Error::KeyDerivation(e.to_string()))?,
        );

        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| Error::KeyDerivation(e.to_string()))?;

        let hash_bytes = hash
            .hash
            .ok_or_else(|| Error::KeyDerivation("Hash generation failed".to_string()))?;

        let mut key = [0u8; 32];
        key.copy_from_slice(hash_bytes.as_bytes());

        Ok(Self {
            key,
            salt: salt.to_string(),
        })
    }

    /// Derive a master key from a password with a known salt
    pub fn derive_with_salt(password: &str, salt_str: &str) -> Result<Self> {
        let salt =
            SaltString::from_b64(salt_str).map_err(|e| Error::KeyDerivation(e.to_string()))?;

        let argon2 = Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            argon2::Params::new(
                ARGON2_MEMORY_KB,
                ARGON2_ITERATIONS,
                ARGON2_PARALLELISM,
                Some(32),
            )
            .map_err(|e| Error::KeyDerivation(e.to_string()))?,
        );

        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| Error::KeyDerivation(e.to_string()))?;

        let hash_bytes = hash
            .hash
            .ok_or_else(|| Error::KeyDerivation("Hash generation failed".to_string()))?;

        let mut key = [0u8; 32];
        key.copy_from_slice(hash_bytes.as_bytes());

        Ok(Self {
            key,
            salt: salt.to_string(),
        })
    }

    /// Get the salt for storage
    pub fn salt(&self) -> &str {
        &self.salt
    }

    /// Get the raw key bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }

    /// Derive a sub-key for a specific purpose
    pub fn derive_subkey(&self, info: &str) -> DerivedKey {
        DerivedKey::derive_from(&self.key, info)
    }
}

/// A key derived from the master key for a specific purpose
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct DerivedKey {
    key: [u8; 32],
}

impl DerivedKey {
    /// Derive a key from a master key using HKDF
    pub fn derive_from(master: &[u8; 32], info: &str) -> Self {
        let hkdf = Hkdf::<Sha256>::new(None, master);
        let mut key = [0u8; 32];
        hkdf.expand(info.as_bytes(), &mut key)
            .expect("HKDF expand failed");

        Self { key }
    }

    /// Get the raw key bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_master_key_derivation() {
        let master = MasterKey::derive("test password").unwrap();
        assert_eq!(master.as_bytes().len(), 32);
    }

    #[test]
    fn test_master_key_with_salt() {
        let master1 = MasterKey::derive("test password").unwrap();
        let salt = master1.salt().to_string();

        let master2 = MasterKey::derive_with_salt("test password", &salt).unwrap();
        assert_eq!(master1.as_bytes(), master2.as_bytes());
    }

    #[test]
    fn test_subkey_derivation() {
        let master = MasterKey::derive("test password").unwrap();

        let vault_key = master.derive_subkey("vault");
        let db_key = master.derive_subkey("database");

        // Different info should produce different keys
        assert_ne!(vault_key.as_bytes(), db_key.as_bytes());

        // Same info should produce same key
        let vault_key2 = master.derive_subkey("vault");
        assert_eq!(vault_key.as_bytes(), vault_key2.as_bytes());
    }
}
