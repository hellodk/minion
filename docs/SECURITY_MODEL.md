# MINION Security Model

## Overview

MINION is designed with security as a foundational principle. This document outlines the security architecture, threat model, and mitigation strategies.

---

## Security Principles

1. **Least Privilege**: Components only have access to what they need
2. **Defense in Depth**: Multiple layers of security controls
3. **Zero Trust**: Every component must authenticate and authorize
4. **Fail Secure**: Failures result in denial of access
5. **Audit Everything**: Comprehensive local logging
6. **No Telemetry**: No data leaves the system without explicit consent

---

## Threat Model

### Assets to Protect

| Asset | Sensitivity | Impact if Compromised |
|-------|-------------|----------------------|
| OAuth tokens | Critical | Full account access |
| API keys | Critical | Service abuse, billing |
| Financial data | High | Privacy breach, fraud |
| Personal documents | High | Privacy breach |
| Book library | Medium | IP concerns |
| Usage patterns | Medium | Privacy breach |
| Configuration | Low | Service disruption |

### Threat Actors

| Actor | Capability | Motivation |
|-------|------------|------------|
| Malicious plugin | Code execution | Data theft, ransomware |
| Local malware | File access | Credential theft |
| Network attacker | MITM | Token interception |
| Physical access | Full system | Data extraction |
| Insider (family) | UI access | Privacy violation |

### Attack Vectors

```
┌─────────────────────────────────────────────────────────────────┐
│                      ATTACK SURFACE                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐        │
│  │  PLUGINS    │    │  NETWORK    │    │   LOCAL     │        │
│  │             │    │             │    │   ACCESS    │        │
│  │ • Malicious │    │ • MITM      │    │ • Physical  │        │
│  │   code      │    │ • API abuse │    │ • Malware   │        │
│  │ • Data      │    │ • OAuth     │    │ • Shoulder  │        │
│  │   exfil     │    │   phishing  │    │   surfing   │        │
│  └─────────────┘    └─────────────┘    └─────────────┘        │
│         │                 │                   │                │
│         └─────────────────┴───────────────────┘                │
│                           │                                    │
│                           ▼                                    │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │                   MINION CORE                            │  │
│  │                                                          │  │
│  │   Mitigations:                                           │  │
│  │   • Plugin sandboxing                                    │  │
│  │   • Permission model                                     │  │
│  │   • Encrypted storage                                    │  │
│  │   • Secure IPC                                           │  │
│  │   • TLS enforcement                                      │  │
│  │   • Input validation                                     │  │
│  │   • Rate limiting                                        │  │
│  │   • Audit logging                                        │  │
│  │                                                          │  │
│  └─────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Encryption Architecture

### Master Key Derivation

```
┌─────────────────────────────────────────────────────────────────┐
│                    KEY DERIVATION FLOW                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│                    User Master Password                         │
│                            │                                    │
│                            ▼                                    │
│                    ┌───────────────┐                           │
│                    │  Argon2id     │                           │
│                    │               │                           │
│                    │  Memory: 64MB │                           │
│                    │  Iterations: 3│                           │
│                    │  Parallelism:4│                           │
│                    │  Salt: random │                           │
│                    │  Output: 256b │                           │
│                    └───────────────┘                           │
│                            │                                    │
│                            ▼                                    │
│                    Master Key (256-bit)                        │
│                            │                                    │
│            ┌───────────────┼───────────────┐                   │
│            │               │               │                   │
│            ▼               ▼               ▼                   │
│    ┌─────────────┐ ┌─────────────┐ ┌─────────────┐            │
│    │HKDF-SHA256  │ │HKDF-SHA256  │ │HKDF-SHA256  │            │
│    │info:"vault" │ │info:"db"    │ │info:"files" │            │
│    └─────────────┘ └─────────────┘ └─────────────┘            │
│            │               │               │                   │
│            ▼               ▼               ▼                   │
│    ┌─────────────┐ ┌─────────────┐ ┌─────────────┐            │
│    │Credential   │ │ Database    │ │  File       │            │
│    │Vault Key    │ │ Encryption  │ │ Encryption  │            │
│    │             │ │ Key         │ │ Key         │            │
│    └─────────────┘ └─────────────┘ └─────────────┘            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Encryption Algorithms

| Purpose | Algorithm | Key Size | Notes |
|---------|-----------|----------|-------|
| Key derivation | Argon2id | 256-bit output | Memory-hard |
| Symmetric encryption | AES-256-GCM | 256-bit | AEAD |
| Key derivation (sub-keys) | HKDF-SHA256 | 256-bit | RFC 5869 |
| Hashing (non-crypto) | BLAKE3 | 256-bit | Fast |
| Random generation | ChaCha20 | - | CSPRNG |

### Credential Vault Structure

```rust
/// Encrypted credential storage format
pub struct EncryptedCredential {
    /// Version for migration support
    version: u8,
    
    /// Service identifier (plaintext for lookup)
    service_id: String,
    
    /// Nonce for AES-GCM (12 bytes)
    nonce: [u8; 12],
    
    /// Encrypted credential data
    ciphertext: Vec<u8>,
    
    /// Authentication tag (16 bytes, appended to ciphertext in GCM)
    
    /// Timestamp of last modification
    modified_at: i64,
}

/// Decrypted credential structure
pub struct Credential {
    /// Credential type
    credential_type: CredentialType,
    
    /// The actual secret data
    data: SecureString,
    
    /// Additional metadata
    metadata: HashMap<String, String>,
}

pub enum CredentialType {
    Password,
    APIKey,
    OAuthToken {
        access_token: SecureString,
        refresh_token: Option<SecureString>,
        expires_at: Option<i64>,
    },
    Certificate {
        cert: Vec<u8>,
        key: SecureString,
    },
}
```

### Secure Memory Handling

```rust
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Secure string that zeroes memory on drop
#[derive(Zeroize, ZeroizeOnDrop)]
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
}

/// Secure bytes that zero memory on drop
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SecureBytes {
    inner: Vec<u8>,
}
```

---

## OAuth Security

### Token Isolation

```
┌─────────────────────────────────────────────────────────────────┐
│                    OAUTH TOKEN ISOLATION                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Each module gets isolated token storage:                      │
│                                                                 │
│   ┌─────────────────────────────────────────────────────────┐  │
│   │                    TOKEN VAULT                           │  │
│   │                                                          │  │
│   │   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐  │  │
│   │   │   Media     │   │    Blog     │   │  Analytics  │  │  │
│   │   │   Module    │   │   Module    │   │   Module    │  │  │
│   │   │             │   │             │   │             │  │  │
│   │   │ YouTube ────┼───┼─────────────┼───┼──────────── │  │  │
│   │   │ tokens      │   │ WordPress   │   │ Google      │  │  │
│   │   │             │   │ Medium      │   │ Analytics   │  │  │
│   │   │ ISOLATED    │   │ Dev.to      │   │ tokens      │  │  │
│   │   │             │   │ tokens      │   │             │  │  │
│   │   └─────────────┘   └─────────────┘   └─────────────┘  │  │
│   │         │                 │                 │          │  │
│   │         │   Access Denied │   Access Denied │          │  │
│   │         │◄────────────────┤◄────────────────┤          │  │
│   │         │                 │                 │          │  │
│   └─────────────────────────────────────────────────────────┘  │
│                                                                 │
│   - Module A cannot access Module B's tokens                   │
│   - Each module has separate encryption key                     │
│   - Token refresh isolated per module                          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### OAuth Flow Security

```rust
/// Secure OAuth state management
pub struct OAuthState {
    /// Random state parameter (CSRF protection)
    state: String,
    
    /// PKCE code verifier
    code_verifier: SecureString,
    
    /// PKCE code challenge (sent to auth server)
    code_challenge: String,
    
    /// Timestamp for expiration
    created_at: Instant,
    
    /// Associated module
    module_id: String,
}

impl OAuthState {
    pub fn new(module_id: &str) -> Self {
        let code_verifier = generate_code_verifier();  // 43-128 chars
        let code_challenge = generate_code_challenge(&code_verifier);  // SHA256
        
        Self {
            state: generate_random_string(32),
            code_verifier: SecureString::new(code_verifier),
            code_challenge,
            created_at: Instant::now(),
            module_id: module_id.to_string(),
        }
    }
    
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > Duration::from_secs(300)  // 5 min timeout
    }
}
```

### Token Refresh Strategy

```rust
/// Automatic token refresh with security checks
pub async fn refresh_token_if_needed(
    module_id: &str,
    service: &str,
    token: &OAuthToken,
) -> Result<OAuthToken, AuthError> {
    // Check if refresh is needed (with buffer)
    if !token.needs_refresh(Duration::from_secs(300)) {
        return Ok(token.clone());
    }
    
    // Acquire refresh lock (prevent concurrent refreshes)
    let _lock = TOKEN_REFRESH_LOCKS
        .get(&(module_id.to_string(), service.to_string()))
        .ok_or(AuthError::LockError)?
        .lock()
        .await;
    
    // Double-check after acquiring lock
    let current_token = get_token(module_id, service).await?;
    if !current_token.needs_refresh(Duration::from_secs(300)) {
        return Ok(current_token);
    }
    
    // Perform refresh
    let new_token = perform_token_refresh(service, &current_token).await?;
    
    // Store new token (encrypted)
    store_token(module_id, service, &new_token).await?;
    
    // Audit log
    audit_log(AuditEvent::TokenRefreshed { module_id, service });
    
    Ok(new_token)
}
```

---

## Plugin Security

### Sandbox Implementation

```rust
/// Plugin sandbox configuration
pub struct PluginSandbox {
    /// Wasmtime engine for WASM plugins
    wasm_engine: wasmtime::Engine,
    
    /// Resource limits
    limits: ResourceLimits,
    
    /// Permission checker
    permissions: PermissionChecker,
    
    /// Audit logger
    audit: AuditLogger,
}

pub struct ResourceLimits {
    /// Maximum memory (bytes)
    max_memory: u64,
    
    /// Maximum execution time per call
    max_execution_time: Duration,
    
    /// Maximum concurrent operations
    max_concurrent_ops: u32,
    
    /// Maximum file size for read/write
    max_file_size: u64,
    
    /// Maximum database query result size
    max_query_results: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory: 256 * 1024 * 1024,  // 256 MB
            max_execution_time: Duration::from_secs(30),
            max_concurrent_ops: 10,
            max_file_size: 100 * 1024 * 1024,  // 100 MB
            max_query_results: 10000,
        }
    }
}
```

### Permission Enforcement

```rust
/// Permission checker with audit logging
pub struct PermissionChecker {
    /// Granted permissions per plugin
    grants: HashMap<String, HashSet<Permission>>,
    
    /// Denied permission cache (for rate limiting)
    denials: RwLock<HashMap<(String, Permission), Instant>>,
}

impl PermissionChecker {
    pub fn check(
        &self,
        plugin_id: &str,
        permission: &Permission,
        context: &PermissionContext,
    ) -> Result<(), PermissionError> {
        // Check if permission is granted
        let grants = self.grants.get(plugin_id)
            .ok_or(PermissionError::PluginNotFound)?;
        
        if !self.matches_permission(grants, permission) {
            // Log denial
            audit_log(AuditEvent::PermissionDenied {
                plugin_id: plugin_id.to_string(),
                permission: permission.clone(),
                context: context.clone(),
            });
            
            return Err(PermissionError::Denied(permission.clone()));
        }
        
        // Log grant (for sensitive permissions only)
        if permission.is_sensitive() {
            audit_log(AuditEvent::PermissionUsed {
                plugin_id: plugin_id.to_string(),
                permission: permission.clone(),
                context: context.clone(),
            });
        }
        
        Ok(())
    }
}
```

### Plugin Code Signing

```rust
/// Plugin signature verification
pub struct PluginSignature {
    /// Signature algorithm
    algorithm: SignatureAlgorithm,
    
    /// Signature bytes
    signature: Vec<u8>,
    
    /// Signing certificate chain
    certificate_chain: Vec<Certificate>,
    
    /// Timestamp (for expiration checking)
    timestamp: DateTime<Utc>,
}

pub enum SignatureAlgorithm {
    Ed25519,
    RSA4096_SHA256,
}

impl PluginSignature {
    pub fn verify(&self, manifest_hash: &[u8], trusted_roots: &[Certificate]) -> Result<(), SignatureError> {
        // Verify certificate chain
        self.verify_certificate_chain(trusted_roots)?;
        
        // Verify signature
        let public_key = &self.certificate_chain[0].public_key;
        match self.algorithm {
            SignatureAlgorithm::Ed25519 => {
                ed25519_verify(public_key, manifest_hash, &self.signature)?;
            }
            SignatureAlgorithm::RSA4096_SHA256 => {
                rsa_verify(public_key, manifest_hash, &self.signature)?;
            }
        }
        
        // Check timestamp (not expired)
        if self.timestamp < Utc::now() - Duration::days(365) {
            return Err(SignatureError::Expired);
        }
        
        Ok(())
    }
}
```

---

## Network Security

### TLS Configuration

```rust
/// Secure TLS configuration for all network requests
pub fn create_tls_config() -> rustls::ClientConfig {
    let mut config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(webpki_roots::TLS_SERVER_ROOTS.clone())
        .with_no_client_auth();
    
    // Require TLS 1.3 minimum
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    
    config
}
```

### Network Request Security

```rust
/// Secure HTTP client with built-in protections
pub struct SecureHttpClient {
    client: reqwest::Client,
    allowed_hosts: HashSet<String>,
    rate_limiter: RateLimiter,
}

impl SecureHttpClient {
    pub async fn request(&self, req: Request) -> Result<Response, NetworkError> {
        // Validate host is allowed
        let host = req.url().host_str()
            .ok_or(NetworkError::InvalidUrl)?;
        
        if !self.is_host_allowed(host) {
            return Err(NetworkError::HostNotAllowed(host.to_string()));
        }
        
        // Rate limit check
        self.rate_limiter.check(host)?;
        
        // Prevent SSRF (no private IPs)
        if is_private_ip(host) {
            return Err(NetworkError::PrivateIPNotAllowed);
        }
        
        // Add security headers
        let req = req
            .header("User-Agent", format!("MINION/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30));
        
        // Execute request
        let response = self.client.execute(req).await?;
        
        // Audit log
        audit_log(AuditEvent::NetworkRequest {
            host: host.to_string(),
            method: req.method().to_string(),
            status: response.status().as_u16(),
        });
        
        Ok(response)
    }
}
```

---

## Audit Logging

### Audit Events

```rust
/// All auditable events in the system
#[derive(Debug, Clone, Serialize)]
pub enum AuditEvent {
    // Authentication
    MasterPasswordEntered { success: bool },
    VaultUnlocked,
    VaultLocked,
    
    // Credentials
    CredentialAccessed { service: String, module_id: String },
    CredentialStored { service: String },
    CredentialDeleted { service: String },
    
    // OAuth
    OAuthStarted { service: String, module_id: String },
    OAuthCompleted { service: String, module_id: String, success: bool },
    TokenRefreshed { module_id: String, service: String },
    
    // Plugins
    PluginInstalled { plugin_id: String, version: String },
    PluginUninstalled { plugin_id: String },
    PluginEnabled { plugin_id: String },
    PluginDisabled { plugin_id: String },
    PermissionDenied { plugin_id: String, permission: Permission, context: PermissionContext },
    PermissionUsed { plugin_id: String, permission: Permission, context: PermissionContext },
    
    // Data access
    DataExported { module_id: String, data_type: String },
    DataImported { module_id: String, data_type: String },
    SensitiveDataAccessed { module_id: String, data_type: String },
    
    // Network
    NetworkRequest { host: String, method: String, status: u16 },
    
    // System
    ApplicationStarted,
    ApplicationStopped,
    ConfigurationChanged { key: String },
    ErrorOccurred { module_id: Option<String>, error: String },
}
```

### Audit Log Storage

```rust
/// Secure audit log storage
pub struct AuditLogger {
    /// SQLite connection for audit storage
    db: Connection,
    
    /// Log rotation config
    rotation: RotationConfig,
    
    /// Encryption for sensitive fields
    encryption_key: DerivedKey,
}

impl AuditLogger {
    pub fn log(&self, event: AuditEvent) -> Result<(), AuditError> {
        let entry = AuditEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type: event.event_type(),
            event_data: self.maybe_encrypt_sensitive(&event)?,
            source_ip: None,  // Desktop app, no IP
            user_agent: None,
        };
        
        self.db.execute(
            "INSERT INTO audit_log (id, timestamp, event_type, event_data) VALUES (?, ?, ?, ?)",
            params![entry.id.to_string(), entry.timestamp, entry.event_type, entry.event_data],
        )?;
        
        Ok(())
    }
    
    /// Rotate logs (keep last N days)
    pub fn rotate(&self) -> Result<(), AuditError> {
        let cutoff = Utc::now() - chrono::Duration::days(self.rotation.retain_days as i64);
        
        self.db.execute(
            "DELETE FROM audit_log WHERE timestamp < ?",
            params![cutoff],
        )?;
        
        Ok(())
    }
}
```

---

## Role-Based Access Control

### Role Definition

```rust
/// System roles with granular permissions
pub struct Role {
    pub id: String,
    pub name: String,
    pub permissions: Vec<SystemPermission>,
}

pub enum SystemPermission {
    // Module access
    ModuleAccess(String),           // Access specific module
    ModuleFullAccess,               // Access all modules
    
    // Data operations
    DataRead(String),               // Read from module
    DataWrite(String),              // Write to module
    DataDelete(String),             // Delete from module
    DataExport(String),             // Export data
    
    // System operations
    PluginInstall,                  // Install plugins
    PluginManage,                   // Enable/disable plugins
    CredentialManage,               // Manage credentials
    ConfigurationManage,            // Change settings
    
    // Administrative
    RoleManage,                     // Manage roles
    AuditRead,                      // View audit logs
    SystemAdmin,                    // Full system access
}

/// Default roles
pub fn default_roles() -> Vec<Role> {
    vec![
        Role {
            id: "admin".to_string(),
            name: "Administrator".to_string(),
            permissions: vec![SystemPermission::SystemAdmin],
        },
        Role {
            id: "standard".to_string(),
            name: "Standard User".to_string(),
            permissions: vec![
                SystemPermission::ModuleFullAccess,
                SystemPermission::DataRead("*".to_string()),
                SystemPermission::DataWrite("*".to_string()),
            ],
        },
        Role {
            id: "restricted".to_string(),
            name: "Restricted User".to_string(),
            permissions: vec![
                SystemPermission::ModuleAccess("reader".to_string()),
                SystemPermission::ModuleAccess("fitness".to_string()),
                SystemPermission::DataRead("*".to_string()),
            ],
        },
    ]
}
```

---

## Security Checklist

### Development Security

- [ ] All dependencies audited (`cargo audit`)
- [ ] No unsafe code without justification
- [ ] Input validation on all external data
- [ ] SQL injection prevention (parameterized queries)
- [ ] Path traversal prevention
- [ ] Memory safety verified

### Deployment Security

- [ ] Release builds only (no debug symbols)
- [ ] Code signing enabled
- [ ] Automatic updates signed
- [ ] Telemetry disabled by default

### Runtime Security

- [ ] Master password required
- [ ] Session timeout configured
- [ ] Plugin sandboxing active
- [ ] Network allowlist enforced
- [ ] Audit logging enabled
- [ ] Encrypted storage active

---

## Incident Response

### Security Event Detection

```rust
/// Security event detection and alerting
pub struct SecurityMonitor {
    /// Anomaly thresholds
    thresholds: SecurityThresholds,
    
    /// Recent events for pattern detection
    recent_events: RingBuffer<AuditEvent>,
}

impl SecurityMonitor {
    pub fn check_event(&mut self, event: &AuditEvent) -> Option<SecurityAlert> {
        self.recent_events.push(event.clone());
        
        // Check for suspicious patterns
        match event {
            AuditEvent::PermissionDenied { .. } => {
                if self.count_recent_denials() > self.thresholds.max_permission_denials {
                    return Some(SecurityAlert::ExcessivePermissionDenials);
                }
            }
            AuditEvent::MasterPasswordEntered { success: false } => {
                if self.count_recent_failed_logins() > self.thresholds.max_failed_logins {
                    return Some(SecurityAlert::BruteForceAttempt);
                }
            }
            AuditEvent::NetworkRequest { host, .. } => {
                if self.is_suspicious_host(host) {
                    return Some(SecurityAlert::SuspiciousNetworkActivity);
                }
            }
            _ => {}
        }
        
        None
    }
}
```

### Response Actions

| Alert | Automatic Response | User Notification |
|-------|-------------------|-------------------|
| Brute force attempt | Lock vault for 5 min | Show warning |
| Excessive permission denials | Disable plugin | Show warning |
| Suspicious network activity | Block host | Show warning |
| Certificate error | Block connection | Show error |
| Data exfiltration attempt | Block operation | Show alert |
