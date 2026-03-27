//! Encrypted credential vault

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::keys::DerivedKey;
use crate::{decrypt, encrypt, Result};

/// Credential types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CredentialType {
    /// Simple password
    Password { password: String },

    /// API key
    ApiKey { key: String },

    /// OAuth tokens
    OAuth {
        access_token: String,
        refresh_token: Option<String>,
        expires_at: Option<i64>,
    },

    /// Certificate and key pair
    Certificate {
        #[serde(with = "base64_serde")]
        cert: Vec<u8>,
        key: String,
    },

    /// Generic key-value pairs
    KeyValue { values: HashMap<String, String> },
}

mod base64_serde {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        STANDARD.decode(&s).map_err(serde::de::Error::custom)
    }
}

/// A credential stored in the vault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    /// Service identifier
    pub service: String,

    /// Credential type and data
    pub credential_type: CredentialType,

    /// Optional metadata
    pub metadata: HashMap<String, String>,

    /// Created timestamp
    pub created_at: i64,

    /// Updated timestamp
    pub updated_at: i64,
}

impl Credential {
    /// Create a new password credential
    pub fn password(service: &str, password: &str) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            service: service.to_string(),
            credential_type: CredentialType::Password {
                password: password.to_string(),
            },
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new API key credential
    pub fn api_key(service: &str, key: &str) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            service: service.to_string(),
            credential_type: CredentialType::ApiKey {
                key: key.to_string(),
            },
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new OAuth credential
    pub fn oauth(
        service: &str,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_at: Option<i64>,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            service: service.to_string(),
            credential_type: CredentialType::OAuth {
                access_token: access_token.to_string(),
                refresh_token: refresh_token.map(|s| s.to_string()),
                expires_at,
            },
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// Encrypted vault storage format
#[derive(Serialize, Deserialize)]
struct VaultStorage {
    version: u8,
    salt: String,
    credentials: Vec<EncryptedCredential>,
}

/// Encrypted credential entry
#[derive(Serialize, Deserialize)]
struct EncryptedCredential {
    service: String,
    #[serde(with = "base64_serde")]
    ciphertext: Vec<u8>,
}

/// The credential vault
pub struct CredentialVault {
    path: PathBuf,
    key: DerivedKey,
    credentials: HashMap<String, Credential>,
}

impl CredentialVault {
    /// Create or open a vault at the given path
    pub fn open(path: &Path, key: DerivedKey) -> Result<Self> {
        let mut vault = Self {
            path: path.to_path_buf(),
            key,
            credentials: HashMap::new(),
        };

        if path.exists() {
            vault.load()?;
        }

        Ok(vault)
    }

    /// Load vault from disk
    fn load(&mut self) -> Result<()> {
        let data = std::fs::read(&self.path)?;
        let storage: VaultStorage = serde_json::from_slice(&data)?;

        for encrypted in storage.credentials {
            let plaintext = decrypt(self.key.as_bytes(), &encrypted.ciphertext)?;
            let credential: Credential = serde_json::from_slice(&plaintext)?;
            self.credentials.insert(encrypted.service, credential);
        }

        Ok(())
    }

    /// Save vault to disk
    fn save(&self) -> Result<()> {
        let mut encrypted_creds = Vec::new();

        for (service, credential) in &self.credentials {
            let plaintext = serde_json::to_vec(credential)?;
            let ciphertext = encrypt(self.key.as_bytes(), &plaintext)?;

            encrypted_creds.push(EncryptedCredential {
                service: service.clone(),
                ciphertext,
            });
        }

        let storage = VaultStorage {
            version: 1,
            salt: String::new(), // Salt is stored separately
            credentials: encrypted_creds,
        };

        let data = serde_json::to_vec_pretty(&storage)?;

        // Write atomically
        let temp_path = self.path.with_extension("tmp");
        std::fs::write(&temp_path, &data)?;
        std::fs::rename(&temp_path, &self.path)?;

        Ok(())
    }

    /// Store a credential
    pub fn store(&mut self, credential: Credential) -> Result<()> {
        self.credentials
            .insert(credential.service.clone(), credential);
        self.save()
    }

    /// Retrieve a credential
    pub fn get(&self, service: &str) -> Option<&Credential> {
        self.credentials.get(service)
    }

    /// Delete a credential
    pub fn delete(&mut self, service: &str) -> Result<bool> {
        let existed = self.credentials.remove(service).is_some();
        if existed {
            self.save()?;
        }
        Ok(existed)
    }

    /// List all service names
    pub fn list_services(&self) -> Vec<&str> {
        self.credentials.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a credential exists
    pub fn exists(&self, service: &str) -> bool {
        self.credentials.contains_key(service)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::MasterKey;
    use tempfile::tempdir;

    #[test]
    fn test_vault_crud() {
        let dir = tempdir().unwrap();
        let vault_path = dir.path().join("vault.enc");

        let master = MasterKey::derive("test password").unwrap();
        let vault_key = master.derive_subkey("vault");

        let mut vault = CredentialVault::open(&vault_path, vault_key).unwrap();

        // Store
        vault
            .store(Credential::api_key("test_service", "secret_key"))
            .unwrap();

        // Get
        let cred = vault.get("test_service").unwrap();
        match &cred.credential_type {
            CredentialType::ApiKey { key } => assert_eq!(key, "secret_key"),
            _ => panic!("Wrong credential type"),
        }

        // List
        assert_eq!(vault.list_services(), vec!["test_service"]);

        // Delete
        assert!(vault.delete("test_service").unwrap());
        assert!(vault.get("test_service").is_none());
    }

    #[test]
    fn test_vault_persistence() {
        let dir = tempdir().unwrap();
        let vault_path = dir.path().join("vault.enc");

        let master = MasterKey::derive("test password").unwrap();
        let salt = master.salt().to_string();
        let vault_key = master.derive_subkey("vault");

        // Create and store
        {
            let mut vault = CredentialVault::open(&vault_path, vault_key).unwrap();
            vault
                .store(Credential::api_key("test_service", "secret_key"))
                .unwrap();
        }

        // Re-open and verify
        {
            let master2 = MasterKey::derive_with_salt("test password", &salt).unwrap();
            let vault_key2 = master2.derive_subkey("vault");
            let vault = CredentialVault::open(&vault_path, vault_key2).unwrap();

            let cred = vault.get("test_service").unwrap();
            match &cred.credential_type {
                CredentialType::ApiKey { key } => assert_eq!(key, "secret_key"),
                _ => panic!("Wrong credential type"),
            }
        }
    }
}
