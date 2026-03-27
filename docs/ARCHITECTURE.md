# MINION System Architecture

## Table of Contents

1. [Overview](#overview)
2. [System Architecture Diagram](#system-architecture-diagram)
3. [Module Interaction Map](#module-interaction-map)
4. [Tech Stack Comparison](#tech-stack-comparison)
5. [Core Components](#core-components)
6. [Plugin Architecture](#plugin-architecture)
7. [Security Model](#security-model)
8. [Performance Strategy](#performance-strategy)
9. [Offline-First Strategy](#offline-first-strategy)
10. [Scaling Model](#scaling-model)

---

## Overview

MINION is designed as a **hub-and-spoke architecture** where the Core Engine acts as the central coordinator, with modules operating as independent units that communicate through a well-defined event bus and API layer.

### Design Principles

1. **Isolation**: Each module is sandboxed with explicit permissions
2. **Event-Driven**: Loose coupling through async message passing
3. **Plugin-First**: Core functionality exposed through plugin APIs
4. **Fail-Safe**: Graceful degradation when modules fail
5. **Resource-Conscious**: Lazy loading, memory pooling, efficient indexing

---

## System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              MINION APPLICATION                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         PRESENTATION LAYER                          │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌───────────┐  │   │
│  │  │   Tauri     │  │  Command    │  │   Global    │  │  Theme    │  │   │
│  │  │   WebView   │  │  Palette    │  │   Search    │  │  Engine   │  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └───────────┘  │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │                    SolidJS UI Components                     │   │   │
│  │  │   Dashboard │ Module Views │ Settings │ Analytics │ Tabs    │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │ IPC                                    │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                          CORE ENGINE (Rust)                         │   │
│  │  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌─────────────────┐  │   │
│  │  │  Plugin   │  │   Event   │  │   Task    │  │    API Router   │  │   │
│  │  │  Manager  │  │    Bus    │  │  Scheduler│  │    (Axum)       │  │   │
│  │  └───────────┘  └───────────┘  └───────────┘  └─────────────────┘  │   │
│  │  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌─────────────────┐  │   │
│  │  │ Credential│  │  Config   │  │   State   │  │  Background     │  │   │
│  │  │   Vault   │  │  Manager  │  │  Machine  │  │  Worker Pool    │  │   │
│  │  └───────────┘  └───────────┘  └───────────┘  └─────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│         ┌──────────────────────────┼──────────────────────────┐            │
│         ▼                          ▼                          ▼            │
│  ┌─────────────┐  ┌─────────────────────────────┐  ┌─────────────────┐    │
│  │  MODULE     │  │        MODULE LAYER         │  │    AI LAYER     │    │
│  │  REGISTRY   │  │                             │  │                 │    │
│  │             │  │  ┌───────┐ ┌───────┐        │  │ ┌─────────────┐ │    │
│  │ • Media     │  │  │ Media │ │ Files │        │  │ │ Ollama      │ │    │
│  │ • Files     │  │  │ Intel │ │ Intel │        │  │ │ Connector   │ │    │
│  │ • Blog      │  │  └───────┘ └───────┘        │  │ └─────────────┘ │    │
│  │ • Finance   │  │  ┌───────┐ ┌───────┐        │  │ ┌─────────────┐ │    │
│  │ • Fitness   │  │  │ Blog  │ │Finance│        │  │ │ Embedding   │ │    │
│  │ • Reader    │  │  │  AI   │ │ Intel │        │  │ │ Engine      │ │    │
│  │ • Plugins   │  │  └───────┘ └───────┘        │  │ └─────────────┘ │    │
│  └─────────────┘  │  ┌───────┐ ┌───────┐        │  │ ┌─────────────┐ │    │
│                   │  │Fitness│ │ Book  │        │  │ │ RAG Engine  │ │    │
│                   │  │Wellnes│ │Reader │        │  │ │             │ │    │
│                   │  └───────┘ └───────┘        │  │ └─────────────┘ │    │
│                   └─────────────────────────────┘  └─────────────────┘    │
│                                    │                          │            │
│                                    ▼                          ▼            │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                          DATA LAYER                                 │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌───────────┐  │   │
│  │  │   SQLite    │  │  Tantivy    │  │   Vector    │  │  File     │  │   │
│  │  │  (libsql)   │  │  Search     │  │   Store     │  │  System   │  │   │
│  │  │             │  │  Index      │  │  (usearch)  │  │  Cache    │  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └───────────┘  │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │               Encrypted Storage Layer (AES-256-GCM)          │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                     │
                    ┌────────────────┼────────────────┐
                    ▼                ▼                ▼
            ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
            │  External   │  │   Local     │  │   OAuth     │
            │   APIs      │  │   LLM       │  │  Providers  │
            │             │  │  (Ollama)   │  │             │
            │ • YouTube   │  │             │  │ • Google    │
            │ • WordPress │  │             │  │ • Medium    │
            │ • NSE/BSE   │  │             │  │ • Dev.to    │
            │ • Analytics │  │             │  │             │
            └─────────────┘  └─────────────┘  └─────────────┘
```

---

## Module Interaction Map

```
                                    ┌──────────────┐
                                    │  EVENT BUS   │
                                    │  (tokio mpsc)│
                                    └──────┬───────┘
                                           │
           ┌───────────────────────────────┼───────────────────────────────┐
           │                               │                               │
           ▼                               ▼                               ▼
    ┌──────────────┐              ┌──────────────┐              ┌──────────────┐
    │    MEDIA     │◄────────────►│    FILES     │◄────────────►│    BLOG      │
    │ INTELLIGENCE │              │ INTELLIGENCE │              │   ENGINE     │
    └──────┬───────┘              └──────┬───────┘              └──────┬───────┘
           │                             │                             │
           │  Thumbnail                  │  Duplicate                  │  Quote
           │  requests                   │  detection                  │  extraction
           │                             │                             │
           └─────────────┬───────────────┴─────────────┬───────────────┘
                         │                             │
                         ▼                             ▼
                  ┌──────────────┐              ┌──────────────┐
                  │    READER    │◄────────────►│   FINANCE    │
                  │   (Books)    │              │ INTELLIGENCE │
                  └──────┬───────┘              └──────┬───────┘
                         │                             │
                         │  Knowledge                  │  Spending
                         │  sharing                    │  insights
                         │                             │
                         └─────────────┬───────────────┘
                                       │
                                       ▼
                                ┌──────────────┐
                                │   FITNESS    │
                                │  & WELLNESS  │
                                └──────────────┘

INTERACTION TYPES:
─────────────────
→ Direct API Call (sync)
⇢ Event Emission (async)  
◄─► Bidirectional data flow

KEY INTERACTIONS:
─────────────────
1. Reader → Blog: Extract highlights/quotes for blog posts
2. Files → Media: Detect duplicate videos before upload
3. Finance → Fitness: Correlate spending on health/fitness
4. Blog → Reader: Auto-generate reading recommendations
5. Media → Files: Storage cleanup after publishing
6. Reader → All: Knowledge base queries across modules
```

---

## Tech Stack Comparison

### Backend Language Decision

| Criteria | Rust | Go | Decision |
|----------|------|-----|----------|
| Memory Safety | Compile-time guarantees | GC-managed | **Rust** |
| Concurrency | Tokio (zero-cost async) | Goroutines (good) | **Rust** |
| Binary Size | ~5-10MB | ~10-15MB | **Rust** |
| Memory Footprint | Excellent (<100MB baseline) | Good (~150MB) | **Rust** |
| FFI for AI libs | Native C interop | CGO (complex) | **Rust** |
| Cross-compile | Excellent | Excellent | Tie |
| GUI Framework | Tauri (native) | Limited | **Rust** |
| Learning Curve | Steep | Gentle | Go |
| Ecosystem for task | Growing rapidly | Mature | Tie |

**Decision**: **Rust** with Tokio async runtime

### Frontend Framework Decision

| Criteria | Tauri + SolidJS | Electron | Native (Qt/GTK) |
|----------|-----------------|----------|-----------------|
| Bundle Size | ~3MB | ~150MB | ~20MB |
| Memory Usage | Excellent | Poor | Good |
| Cross-platform | Yes | Yes | Complex |
| Development Speed | Fast | Fast | Slow |
| Native Integration | Excellent | Good | Excellent |
| GPU Acceleration | Via WebGL/Canvas | Via Chromium | Direct |

**Decision**: **Tauri + SolidJS** for optimal size/performance balance

### Database Decision

| Criteria | SQLite | PostgreSQL | RocksDB |
|----------|--------|------------|---------|
| Embedded | Yes | No | Yes |
| Offline-first | Perfect | Requires server | Yes |
| ACID | Yes | Yes | Yes |
| Full-text search | Limited | Good | No |
| Complexity | Low | High | Medium |

**Decision**: **SQLite (libsql)** + **Tantivy** for search + **usearch** for vectors

### AI Integration Decision

| Approach | Pros | Cons | Use Case |
|----------|------|------|----------|
| Ollama API | Simple, maintained | Network call | Default |
| llama.cpp bindings | Direct, fast | Maintenance burden | Performance-critical |
| Python microservice | Rich ecosystem | Extra process | Complex ML tasks |
| ONNX Runtime | Portable, fast | Model conversion | Embedding models |

**Decision**: **Ollama API** primary + **ONNX Runtime** for embeddings

---

## Core Components

### 1. Plugin Manager

```rust
// Plugin trait that all modules implement
pub trait Plugin: Send + Sync {
    fn metadata(&self) -> PluginMetadata;
    fn initialize(&mut self, ctx: &PluginContext) -> Result<()>;
    fn shutdown(&mut self) -> Result<()>;
    fn capabilities(&self) -> Vec<Capability>;
    fn handle_event(&self, event: &Event) -> Result<Option<Event>>;
}

pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub version: semver::Version,
    pub author: String,
    pub permissions: Vec<Permission>,
    pub dependencies: Vec<Dependency>,
}
```

### 2. Event Bus

```rust
pub enum Event {
    // Core events
    ModuleLoaded { module_id: String },
    ModuleUnloaded { module_id: String },
    ConfigChanged { key: String, value: Value },
    
    // Cross-module events
    FileDiscovered { path: PathBuf, metadata: FileMetadata },
    ContentCreated { content_type: ContentType, id: Uuid },
    AIRequestComplete { request_id: Uuid, response: AIResponse },
    
    // User events
    UserAction { action: String, payload: Value },
}
```

### 3. Credential Vault

```rust
pub struct CredentialVault {
    master_key: DerivedKey,
    storage: EncryptedStorage,
}

impl CredentialVault {
    pub fn store(&self, service: &str, credential: Credential) -> Result<()>;
    pub fn retrieve(&self, service: &str) -> Result<Credential>;
    pub fn delete(&self, service: &str) -> Result<()>;
    pub fn list_services(&self) -> Result<Vec<String>>;
}
```

### 4. Background Worker Pool

```rust
pub struct WorkerPool {
    workers: Vec<Worker>,
    task_queue: mpsc::Sender<Task>,
    priority_queue: BinaryHeap<PriorityTask>,
}

pub enum TaskPriority {
    Critical,    // UI-blocking operations
    High,        // User-initiated tasks
    Normal,      // Background processing
    Low,         // Maintenance tasks
    Idle,        // Only when system is idle
}
```

---

## Plugin Architecture

### Plugin Lifecycle

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│  Discovery  │───►│  Validation │───►│   Loading   │───►│   Running   │
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
                                                               │
                                                               ▼
                                                        ┌─────────────┐
                                                        │  Shutdown   │
                                                        └─────────────┘
```

### Plugin Permission Model

```rust
pub enum Permission {
    // File system
    FileRead(PathPattern),
    FileWrite(PathPattern),
    
    // Network
    NetworkAccess(Vec<String>),  // Allowed hosts
    
    // System
    ProcessSpawn,
    ClipboardAccess,
    NotificationSend,
    
    // Data
    DatabaseRead(Vec<String>),   // Table names
    DatabaseWrite(Vec<String>),
    CredentialAccess(Vec<String>), // Service names
    
    // AI
    AIModelAccess,
    EmbeddingAccess,
}
```

### Plugin Communication

```
┌──────────────────────────────────────────────────────────────────┐
│                         PLUGIN A                                 │
│  ┌─────────────┐                                                │
│  │   Export    │──────► Event Bus ──────► Plugin B Import       │
│  │  Interface  │                                                │
│  └─────────────┘                                                │
│  ┌─────────────┐                                                │
│  │   Import    │◄────── Shared State ◄────── Plugin C Export    │
│  │  Interface  │                                                │
│  └─────────────┘                                                │
└──────────────────────────────────────────────────────────────────┘
```

---

## Security Model

### Threat Model

| Threat | Mitigation |
|--------|------------|
| Credential theft | AES-256-GCM encryption, key derivation from master password |
| Memory attacks | Rust memory safety, zeroing sensitive data |
| Plugin malware | Sandboxed execution, permission model |
| Data exfiltration | No telemetry, network allowlist per plugin |
| API key exposure | Credential vault, never in plaintext configs |
| Unauthorized access | Role-based module access |

### Encryption Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     KEY DERIVATION                              │
│                                                                 │
│   Master Password                                               │
│         │                                                       │
│         ▼                                                       │
│   ┌─────────────┐                                              │
│   │   Argon2id  │  (memory: 64MB, iterations: 3, parallelism: 4)│
│   └─────────────┘                                              │
│         │                                                       │
│         ▼                                                       │
│   Master Key (256-bit)                                         │
│         │                                                       │
│         ├──────────────┬──────────────┬──────────────┐         │
│         ▼              ▼              ▼              ▼         │
│   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐   │
│   │Credential│   │ Database │   │  File    │   │  Config  │   │
│   │   Key    │   │   Key    │   │   Key    │   │   Key    │   │
│   └──────────┘   └──────────┘   └──────────┘   └──────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### OAuth Token Isolation

```rust
pub struct OAuthIsolation {
    // Each module gets isolated token storage
    module_tokens: HashMap<ModuleId, TokenStore>,
    
    // Tokens are encrypted per-module
    encryption_keys: HashMap<ModuleId, DerivedKey>,
}

impl OAuthIsolation {
    // Module A cannot access Module B's tokens
    pub fn get_token(&self, module_id: &ModuleId, service: &str) -> Result<Token>;
}
```

---

## Performance Strategy

### Memory Management

```
Target: < 300MB baseline RAM

┌─────────────────────────────────────────────────────────────────┐
│                    MEMORY BUDGET                                │
├─────────────────────────────────────────────────────────────────┤
│  Core Engine          │  ~50MB   │  Always loaded              │
│  UI (Tauri WebView)   │  ~80MB   │  Always loaded              │
│  Active Module        │  ~50MB   │  Per active module          │
│  Database Connections │  ~20MB   │  Connection pooling         │
│  Search Index Cache   │  ~30MB   │  LRU eviction               │
│  AI Embedding Cache   │  ~50MB   │  Configurable limit         │
│  Buffer               │  ~20MB   │  Safety margin              │
├─────────────────────────────────────────────────────────────────┤
│  TOTAL                │  ~300MB  │                             │
└─────────────────────────────────────────────────────────────────┘
```

### Lazy Loading Strategy

```rust
pub enum ModuleState {
    Unloaded,           // Not in memory
    Metadata,           // Only metadata loaded
    Initialized,        // Ready but not active
    Active,             // Fully loaded and running
    Suspended,          // Paused, minimal memory
}

// Modules load on-demand
pub async fn activate_module(id: &ModuleId) -> Result<()> {
    match current_state(id) {
        ModuleState::Unloaded => {
            load_metadata(id).await?;
            initialize_module(id).await?;
            activate(id).await?;
        }
        ModuleState::Suspended => {
            resume(id).await?;
        }
        _ => {}
    }
    Ok(())
}
```

### Indexing Strategy

```
┌─────────────────────────────────────────────────────────────────┐
│                    INDEXING ARCHITECTURE                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   File System Watcher (notify-rs)                              │
│         │                                                       │
│         ▼                                                       │
│   ┌─────────────┐                                              │
│   │  Debouncer  │  (100ms window)                              │
│   └─────────────┘                                              │
│         │                                                       │
│         ▼                                                       │
│   ┌─────────────┐                                              │
│   │ Index Queue │  (bounded channel, backpressure)             │
│   └─────────────┘                                              │
│         │                                                       │
│    ┌────┴────┐                                                 │
│    ▼         ▼                                                 │
│ ┌──────┐ ┌──────┐                                              │
│ │Worker│ │Worker│  (parallel indexing)                         │
│ └──────┘ └──────┘                                              │
│    │         │                                                 │
│    └────┬────┘                                                 │
│         ▼                                                       │
│   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐     │
│   │   SQLite    │     │  Tantivy    │     │   Vector    │     │
│   │  Metadata   │     │  Full-text  │     │   Index     │     │
│   └─────────────┘     └─────────────┘     └─────────────┘     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Offline-First Strategy

### Sync Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    OFFLINE-FIRST DESIGN                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌─────────────────────────────────────────────────────────┐  │
│   │                    LOCAL STATE                           │  │
│   │  ┌───────────┐  ┌───────────┐  ┌───────────┐           │  │
│   │  │   Queue   │  │  Pending  │  │   Cache   │           │  │
│   │  │  Actions  │  │  Uploads  │  │   Data    │           │  │
│   │  └───────────┘  └───────────┘  └───────────┘           │  │
│   └─────────────────────────────────────────────────────────┘  │
│                              │                                  │
│                              ▼                                  │
│   ┌─────────────────────────────────────────────────────────┐  │
│   │                  SYNC ENGINE                             │  │
│   │                                                          │  │
│   │  1. Check connectivity                                   │  │
│   │  2. Process pending queue (FIFO)                        │  │
│   │  3. Resolve conflicts (last-write-wins or user prompt)  │  │
│   │  4. Update local state                                   │  │
│   │  5. Invalidate stale cache                              │  │
│   │                                                          │  │
│   └─────────────────────────────────────────────────────────┘  │
│                              │                                  │
│                              ▼                                  │
│   ┌─────────────────────────────────────────────────────────┐  │
│   │                  EXTERNAL SERVICES                       │  │
│   │  YouTube │ WordPress │ Medium │ Google Analytics        │  │
│   └─────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Offline Capabilities Per Module

| Module | Full Offline | Partial Offline | Online Required |
|--------|--------------|-----------------|-----------------|
| Media Intelligence | ✓ Processing | - | Upload, scheduling |
| File Intelligence | ✓ Full | - | - |
| Blog Engine | ✓ Writing | - | Publishing |
| Finance | ✓ Local tracking | - | Live prices |
| Fitness | ✓ Full | - | - |
| Book Reader | ✓ Full | - | Cover fetch |

---

## Scaling Model

### 10TB File Handling Strategy

```
┌─────────────────────────────────────────────────────────────────┐
│                    LARGE SCALE HANDLING                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  CHUNKED SCANNING:                                              │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Directory Iterator (walkdir)                            │   │
│  │       │                                                  │   │
│  │       ▼                                                  │   │
│  │  ┌─────────┐   ┌─────────┐   ┌─────────┐               │   │
│  │  │ Chunk 1 │   │ Chunk 2 │   │ Chunk N │  (1000 files) │   │
│  │  └─────────┘   └─────────┘   └─────────┘               │   │
│  │       │             │             │                      │   │
│  │       └─────────────┼─────────────┘                      │   │
│  │                     ▼                                    │   │
│  │              Process & Commit                            │   │
│  │              (memory bounded)                            │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  INCREMENTAL INDEXING:                                          │
│  • File system watcher for real-time updates                   │
│  • Full re-scan only on user request                           │
│  • Delta computation for changed files                          │
│                                                                 │
│  STREAMING HASH COMPUTATION:                                    │
│  • 64KB buffer for file reading                                │
│  • Parallel hash computation (rayon)                           │
│  • Progress reporting every 1000 files                          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Performance Targets

| Scale | Files | Expected Time | Memory |
|-------|-------|---------------|--------|
| Small | < 10K | < 30 seconds | < 200MB |
| Medium | 10K-100K | < 5 minutes | < 300MB |
| Large | 100K-1M | < 30 minutes | < 400MB |
| Massive | 1M-10M | < 4 hours | < 500MB |
