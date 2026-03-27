# MINION Performance Optimization Strategy

## Overview

MINION is designed to run efficiently on a wide range of hardware, from Mini PCs to high-performance workstations, while maintaining a baseline memory footprint under 300MB.

---

## Performance Targets

| Metric | Target | Stretch Goal |
|--------|--------|--------------|
| Cold startup | < 3s | < 1.5s |
| Warm startup | < 1s | < 500ms |
| Memory baseline | < 300MB | < 200MB |
| Memory per module | < 50MB | < 30MB |
| File scan rate | > 1000/s | > 5000/s |
| Search latency | < 100ms | < 50ms |
| UI frame rate | 60 FPS | 120 FPS |
| Database query | < 50ms | < 10ms |

---

## Memory Management

### Memory Budget Allocation

```
┌─────────────────────────────────────────────────────────────────┐
│                    MEMORY BUDGET (300MB)                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────────────────────────────────────┐           │
│  │ Core Engine                            50MB     │           │
│  │ ├─ Event Bus                           5MB      │           │
│  │ ├─ Plugin Manager                      10MB     │           │
│  │ ├─ Configuration                       5MB      │           │
│  │ ├─ Credential Vault                    10MB     │           │
│  │ └─ Background Workers                  20MB     │           │
│  └─────────────────────────────────────────────────┘           │
│                                                                 │
│  ┌─────────────────────────────────────────────────┐           │
│  │ UI Layer (Tauri WebView)               80MB     │           │
│  │ ├─ WebView Runtime                     60MB     │           │
│  │ ├─ UI State                            10MB     │           │
│  │ └─ Component Cache                     10MB     │           │
│  └─────────────────────────────────────────────────┘           │
│                                                                 │
│  ┌─────────────────────────────────────────────────┐           │
│  │ Active Module (Largest)                50MB     │           │
│  │ ├─ Module State                        20MB     │           │
│  │ ├─ Processing Buffers                  20MB     │           │
│  │ └─ Cache                               10MB     │           │
│  └─────────────────────────────────────────────────┘           │
│                                                                 │
│  ┌─────────────────────────────────────────────────┐           │
│  │ Database Layer                         50MB     │           │
│  │ ├─ SQLite Cache                        20MB     │           │
│  │ ├─ Query Cache                         10MB     │           │
│  │ └─ Connection Pool                     20MB     │           │
│  └─────────────────────────────────────────────────┘           │
│                                                                 │
│  ┌─────────────────────────────────────────────────┐           │
│  │ Search Index                           30MB     │           │
│  │ ├─ Tantivy Segments                    20MB     │           │
│  │ └─ Query Cache                         10MB     │           │
│  └─────────────────────────────────────────────────┘           │
│                                                                 │
│  ┌─────────────────────────────────────────────────┐           │
│  │ AI/Embedding Cache                     20MB     │           │
│  │ └─ LRU Embedding Cache                 20MB     │           │
│  └─────────────────────────────────────────────────┘           │
│                                                                 │
│  ┌─────────────────────────────────────────────────┐           │
│  │ Buffer/Headroom                        20MB     │           │
│  └─────────────────────────────────────────────────┘           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Lazy Loading Strategy

```rust
/// Module loading states for lazy initialization
pub enum ModuleState {
    /// Not loaded - zero memory
    Unloaded,
    
    /// Metadata only - ~1KB
    MetadataOnly {
        metadata: ModuleMetadata,
    },
    
    /// Initialized but suspended - ~5MB
    Suspended {
        metadata: ModuleMetadata,
        config: ModuleConfig,
    },
    
    /// Fully active - up to 50MB
    Active {
        metadata: ModuleMetadata,
        config: ModuleConfig,
        instance: Box<dyn Module>,
    },
}

/// Module manager with lazy loading
pub struct ModuleManager {
    modules: HashMap<String, ModuleState>,
    active_budget: AtomicUsize,
    max_active_budget: usize,
}

impl ModuleManager {
    const MAX_CONCURRENT_ACTIVE: usize = 3;
    
    pub async fn activate(&mut self, module_id: &str) -> Result<&dyn Module> {
        // Check if already active
        if let ModuleState::Active { instance, .. } = self.modules.get(module_id).unwrap() {
            return Ok(instance.as_ref());
        }
        
        // Check budget
        if self.count_active() >= Self::MAX_CONCURRENT_ACTIVE {
            // Suspend least recently used module
            self.suspend_lru().await?;
        }
        
        // Load and activate module
        self.do_activate(module_id).await
    }
    
    pub async fn suspend(&mut self, module_id: &str) -> Result<()> {
        if let Some(ModuleState::Active { metadata, config, instance }) = self.modules.remove(module_id) {
            // Save state
            instance.save_state().await?;
            
            // Transition to suspended
            self.modules.insert(module_id.to_string(), ModuleState::Suspended {
                metadata,
                config,
            });
        }
        Ok(())
    }
}
```

### Memory Pooling

```rust
/// Reusable buffer pool to reduce allocations
pub struct BufferPool {
    small: ArrayQueue<Vec<u8>>,   // 4KB buffers
    medium: ArrayQueue<Vec<u8>>,  // 64KB buffers
    large: ArrayQueue<Vec<u8>>,   // 1MB buffers
}

impl BufferPool {
    pub fn new(small_count: usize, medium_count: usize, large_count: usize) -> Self {
        let small = ArrayQueue::new(small_count);
        let medium = ArrayQueue::new(medium_count);
        let large = ArrayQueue::new(large_count);
        
        // Pre-allocate buffers
        for _ in 0..small_count {
            let _ = small.push(Vec::with_capacity(4 * 1024));
        }
        for _ in 0..medium_count {
            let _ = medium.push(Vec::with_capacity(64 * 1024));
        }
        for _ in 0..large_count {
            let _ = large.push(Vec::with_capacity(1024 * 1024));
        }
        
        Self { small, medium, large }
    }
    
    pub fn acquire(&self, min_size: usize) -> PooledBuffer {
        let buffer = if min_size <= 4 * 1024 {
            self.small.pop().unwrap_or_else(|| Vec::with_capacity(4 * 1024))
        } else if min_size <= 64 * 1024 {
            self.medium.pop().unwrap_or_else(|| Vec::with_capacity(64 * 1024))
        } else {
            self.large.pop().unwrap_or_else(|| Vec::with_capacity(1024 * 1024))
        };
        
        PooledBuffer { buffer, pool: self }
    }
}

pub struct PooledBuffer<'a> {
    buffer: Vec<u8>,
    pool: &'a BufferPool,
}

impl Drop for PooledBuffer<'_> {
    fn drop(&mut self) {
        let mut buffer = std::mem::take(&mut self.buffer);
        buffer.clear();
        
        // Return to appropriate pool
        let capacity = buffer.capacity();
        if capacity <= 4 * 1024 {
            let _ = self.pool.small.push(buffer);
        } else if capacity <= 64 * 1024 {
            let _ = self.pool.medium.push(buffer);
        } else {
            let _ = self.pool.large.push(buffer);
        }
    }
}
```

---

## Startup Optimization

### Startup Phases

```
┌─────────────────────────────────────────────────────────────────┐
│                    STARTUP TIMELINE                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Phase 1: Critical Path (< 500ms)                              │
│  ├─ Load configuration                      50ms               │
│  ├─ Initialize logging                      10ms               │
│  ├─ Open database connection               100ms               │
│  ├─ Start event bus                         20ms               │
│  ├─ Initialize UI runtime                  300ms               │
│  └─ Show splash screen                      20ms               │
│                                                                 │
│  Phase 2: Core Services (< 1000ms) [parallel]                  │
│  ├─ Load credential vault                  200ms               │
│  ├─ Initialize plugin manager              100ms               │
│  ├─ Start background worker pool           100ms               │
│  ├─ Load search index metadata             200ms               │
│  └─ Initialize API router                  100ms               │
│                                                                 │
│  Phase 3: Module Metadata (< 500ms) [parallel]                 │
│  ├─ Scan installed modules                 100ms               │
│  ├─ Load module metadata                   200ms               │
│  └─ Initialize default module              200ms               │
│                                                                 │
│  Phase 4: Deferred (after UI ready)                            │
│  ├─ File system watcher setup                                  │
│  ├─ Network connectivity check                                 │
│  ├─ Background sync tasks                                      │
│  └─ Plugin initialization                                      │
│                                                                 │
│  Total: < 2000ms to interactive                                │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Parallel Initialization

```rust
/// Parallel startup with dependency ordering
pub async fn startup() -> Result<Application> {
    // Phase 1: Critical path (sequential)
    let config = load_config().await?;
    init_logging(&config)?;
    let db = open_database(&config).await?;
    let event_bus = EventBus::new();
    
    // Show UI as early as possible
    let ui_handle = spawn_ui(&config)?;
    
    // Phase 2: Core services (parallel)
    let (vault, plugins, workers, search, api) = tokio::try_join!(
        async { CredentialVault::load(&config).await },
        async { PluginManager::new(&config).await },
        async { WorkerPool::new(&config) },
        async { SearchIndex::load(&config).await },
        async { ApiRouter::new(&config) },
    )?;
    
    // Phase 3: Module metadata (parallel)
    let modules = scan_modules(&config).await?;
    let metadata = load_module_metadata(&modules).await?;
    
    // Initialize default module
    let default_module = activate_default_module(&metadata, &config).await?;
    
    // Notify UI that core is ready
    ui_handle.send(UiMessage::CoreReady)?;
    
    // Phase 4: Deferred initialization (background)
    tokio::spawn(async move {
        setup_file_watcher(&config).await;
        check_network_connectivity().await;
        schedule_background_sync().await;
        initialize_remaining_plugins().await;
    });
    
    Ok(Application {
        config,
        db,
        event_bus,
        vault,
        plugins,
        workers,
        search,
        api,
        modules: metadata,
        ui: ui_handle,
    })
}
```

---

## File Scanning Performance

### Parallel Directory Walker

```rust
/// High-performance parallel directory scanner
pub struct ParallelScanner {
    workers: usize,
    batch_size: usize,
    buffer_pool: Arc<BufferPool>,
}

impl ParallelScanner {
    pub async fn scan(&self, root: &Path) -> Result<ScanResult> {
        let (tx, rx) = flume::bounded(10000);
        let counter = Arc::new(AtomicUsize::new(0));
        
        // Spawn directory walker
        let walker = WalkDir::new(root)
            .parallelism(jwalk::Parallelism::RayonNewPool(self.workers))
            .skip_hidden(false);
        
        // Process entries in batches
        let processor = {
            let counter = counter.clone();
            let buffer_pool = self.buffer_pool.clone();
            
            tokio::spawn(async move {
                let mut batch = Vec::with_capacity(1000);
                
                while let Ok(entry) = rx.recv_async().await {
                    batch.push(entry);
                    
                    if batch.len() >= 1000 {
                        process_batch(&batch, &buffer_pool).await?;
                        counter.fetch_add(batch.len(), Ordering::Relaxed);
                        batch.clear();
                    }
                }
                
                // Process remaining
                if !batch.is_empty() {
                    process_batch(&batch, &buffer_pool).await?;
                    counter.fetch_add(batch.len(), Ordering::Relaxed);
                }
                
                Ok::<_, Error>(())
            })
        };
        
        // Walk directory
        for entry in walker {
            if let Ok(entry) = entry {
                tx.send_async(entry).await?;
            }
        }
        drop(tx);
        
        processor.await??;
        
        Ok(ScanResult {
            files_scanned: counter.load(Ordering::Relaxed),
        })
    }
}
```

### Streaming Hash Computation

```rust
/// Stream-based hash computation for large files
pub async fn compute_file_hash(path: &Path) -> Result<FileHashes> {
    const BUFFER_SIZE: usize = 64 * 1024;  // 64KB chunks
    
    let file = tokio::fs::File::open(path).await?;
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);
    
    let mut sha256 = Sha256::new();
    let mut blake3 = blake3::Hasher::new();
    let mut buffer = vec![0u8; BUFFER_SIZE];
    
    loop {
        let bytes_read = reader.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }
        
        // Update both hashes in parallel
        let chunk = &buffer[..bytes_read];
        sha256.update(chunk);
        blake3.update(chunk);
    }
    
    Ok(FileHashes {
        sha256: hex::encode(sha256.finalize()),
        blake3: blake3.finalize().to_hex().to_string(),
    })
}
```

### Perceptual Hash Pipeline

```rust
/// Parallel perceptual hash computation for images
pub async fn compute_perceptual_hashes(
    paths: Vec<PathBuf>,
    parallelism: usize,
) -> Result<HashMap<PathBuf, PerceptualHash>> {
    let results = Arc::new(Mutex::new(HashMap::new()));
    
    // Process in parallel using rayon
    paths.par_iter()
        .with_max_len(parallelism)
        .for_each(|path| {
            if let Ok(hash) = compute_image_phash(path) {
                results.lock().unwrap().insert(path.clone(), hash);
            }
        });
    
    Ok(Arc::try_unwrap(results).unwrap().into_inner().unwrap())
}

fn compute_image_phash(path: &Path) -> Result<PerceptualHash> {
    // Load and resize image
    let img = image::open(path)?;
    let thumbnail = img.resize_exact(32, 32, image::imageops::FilterType::Lanczos3);
    let gray = thumbnail.to_luma8();
    
    // Compute DCT-based hash
    let mut hash = 0u64;
    let pixels: Vec<f32> = gray.pixels().map(|p| p.0[0] as f32).collect();
    
    // Simple average hash for speed
    let avg: f32 = pixels.iter().sum::<f32>() / pixels.len() as f32;
    for (i, &pixel) in pixels.iter().enumerate().take(64) {
        if pixel > avg {
            hash |= 1 << i;
        }
    }
    
    Ok(PerceptualHash(hash))
}
```

---

## Database Performance

### Connection Pooling

```rust
/// SQLite connection pool with prepared statement caching
pub struct DatabasePool {
    pool: r2d2::Pool<SqliteConnectionManager>,
    statement_cache: Arc<RwLock<HashMap<String, String>>>,
}

impl DatabasePool {
    pub fn new(path: &Path, pool_size: u32) -> Result<Self> {
        let manager = SqliteConnectionManager::file(path)
            .with_init(|conn| {
                // Optimize SQLite for performance
                conn.execute_batch("
                    PRAGMA journal_mode = WAL;
                    PRAGMA synchronous = NORMAL;
                    PRAGMA cache_size = -64000;  -- 64MB cache
                    PRAGMA temp_store = MEMORY;
                    PRAGMA mmap_size = 268435456;  -- 256MB mmap
                    PRAGMA page_size = 4096;
                ")?;
                Ok(())
            });
        
        let pool = r2d2::Pool::builder()
            .max_size(pool_size)
            .min_idle(Some(2))
            .build(manager)?;
        
        Ok(Self {
            pool,
            statement_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    pub fn get(&self) -> Result<PooledConnection> {
        Ok(self.pool.get()?)
    }
}
```

### Query Optimization

```rust
/// Batch insert for high-volume data
pub async fn batch_insert_files(conn: &Connection, files: &[FileRecord]) -> Result<()> {
    const BATCH_SIZE: usize = 1000;
    
    for chunk in files.chunks(BATCH_SIZE) {
        let mut stmt = conn.prepare_cached(
            "INSERT OR REPLACE INTO file_index 
             (id, path, name, size, sha256, modified_at) 
             VALUES (?, ?, ?, ?, ?, ?)"
        )?;
        
        let tx = conn.transaction()?;
        
        for file in chunk {
            stmt.execute(params![
                file.id,
                file.path.to_string_lossy(),
                file.name,
                file.size,
                file.sha256,
                file.modified_at,
            ])?;
        }
        
        tx.commit()?;
    }
    
    Ok(())
}
```

### Index Strategy

```sql
-- Strategic indexes for common queries

-- File Intelligence
CREATE INDEX idx_files_sha256 ON file_index(sha256);
CREATE INDEX idx_files_size_sha256 ON file_index(file_size, sha256);  -- Duplicate finding
CREATE INDEX idx_files_ext_size ON file_index(extension, file_size);  -- Analytics
CREATE INDEX idx_files_modified ON file_index(modified_at DESC);      -- Recent files

-- Finance
CREATE INDEX idx_txn_date_category ON transactions(date, category);
CREATE INDEX idx_txn_account_date ON transactions(account_id, date);
CREATE INDEX idx_holdings_symbol ON investment_holdings(symbol);

-- Reader
CREATE INDEX idx_books_title ON books(title COLLATE NOCASE);
CREATE INDEX idx_books_author ON books(authors);  -- JSON index if supported
CREATE INDEX idx_annotations_book ON book_annotations(book_id);

-- Search optimization
CREATE INDEX idx_fts_content ON blog_posts USING fts5(title, content_markdown);
```

---

## UI Performance

### Virtual List for Large Data

```typescript
// SolidJS virtual list for efficient rendering
import { createVirtualizer } from '@tanstack/solid-virtual';

function FileList(props: { files: FileRecord[] }) {
  let parentRef: HTMLDivElement;
  
  const virtualizer = createVirtualizer({
    get count() { return props.files.length; },
    getScrollElement: () => parentRef,
    estimateSize: () => 48,  // Row height
    overscan: 5,
  });
  
  return (
    <div 
      ref={parentRef!} 
      style={{ height: '100%', overflow: 'auto' }}
    >
      <div style={{ height: `${virtualizer.getTotalSize()}px`, position: 'relative' }}>
        <For each={virtualizer.getVirtualItems()}>
          {(virtualItem) => (
            <div
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                height: `${virtualItem.size}px`,
                transform: `translateY(${virtualItem.start}px)`,
              }}
            >
              <FileRow file={props.files[virtualItem.index]} />
            </div>
          )}
        </For>
      </div>
    </div>
  );
}
```

### Debounced Search

```typescript
// Debounced search input
function SearchInput(props: { onSearch: (query: string) => void }) {
  const [query, setQuery] = createSignal('');
  
  // Debounce search by 300ms
  const debouncedSearch = debounce((q: string) => {
    props.onSearch(q);
  }, 300);
  
  createEffect(() => {
    debouncedSearch(query());
  });
  
  return (
    <input
      type="text"
      placeholder="Search..."
      value={query()}
      onInput={(e) => setQuery(e.currentTarget.value)}
    />
  );
}
```

### GPU Acceleration

```css
/* GPU-accelerated animations */
.page-transition {
  will-change: transform, opacity;
  transform: translateZ(0);  /* Force GPU layer */
}

.book-page {
  transform: perspective(1000px) rotateY(0deg);
  transform-origin: left center;
  transition: transform 0.6s cubic-bezier(0.645, 0.045, 0.355, 1);
}

.book-page.turning {
  transform: perspective(1000px) rotateY(-180deg);
}

/* Reduce layout thrashing */
.virtual-list-item {
  contain: layout style paint;
}
```

---

## Background Processing

### Task Priority Queue

```rust
/// Priority-based task scheduler
pub struct TaskScheduler {
    queues: [ArrayQueue<Task>; 5],  // One queue per priority level
    workers: Vec<JoinHandle<()>>,
    shutdown: AtomicBool,
}

impl TaskScheduler {
    pub fn submit(&self, task: Task) {
        let priority = task.priority as usize;
        let _ = self.queues[priority].push(task);
    }
    
    fn worker_loop(&self) {
        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                break;
            }
            
            // Check queues in priority order
            for queue in &self.queues {
                if let Some(task) = queue.pop() {
                    self.execute_task(task);
                    continue;
                }
            }
            
            // No tasks available, sleep briefly
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Critical = 0,  // UI-blocking
    High = 1,      // User-initiated
    Normal = 2,    // Background processing
    Low = 3,       // Maintenance
    Idle = 4,      // Only when system is idle
}
```

### Incremental Processing

```rust
/// Incremental file indexing with checkpointing
pub async fn incremental_index(directory: &Path) -> Result<()> {
    let checkpoint = load_checkpoint(directory).await?;
    
    let entries = WalkDir::new(directory)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
        .filter_map(|e| e.ok());
    
    let mut processed = 0;
    let mut batch = Vec::with_capacity(100);
    
    for entry in entries {
        // Skip already indexed files
        if let Some(cp) = &checkpoint {
            if entry.path() <= cp.last_path {
                continue;
            }
        }
        
        batch.push(index_file(entry.path()).await?);
        processed += 1;
        
        // Checkpoint every 1000 files
        if processed % 1000 == 0 {
            commit_batch(&batch).await?;
            save_checkpoint(directory, entry.path()).await?;
            batch.clear();
        }
    }
    
    // Final batch
    if !batch.is_empty() {
        commit_batch(&batch).await?;
    }
    
    Ok(())
}
```

---

## Profiling and Monitoring

### Built-in Profiler

```rust
/// Simple profiling infrastructure
pub struct Profiler {
    spans: RwLock<HashMap<String, SpanStats>>,
}

impl Profiler {
    pub fn span(&self, name: &str) -> ProfileSpan {
        ProfileSpan {
            name: name.to_string(),
            start: Instant::now(),
            profiler: self,
        }
    }
}

pub struct ProfileSpan<'a> {
    name: String,
    start: Instant,
    profiler: &'a Profiler,
}

impl Drop for ProfileSpan<'_> {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        let mut spans = self.profiler.spans.write().unwrap();
        let stats = spans.entry(self.name.clone()).or_default();
        stats.count += 1;
        stats.total_time += duration;
        stats.max_time = stats.max_time.max(duration);
    }
}

// Usage
let _span = profiler.span("file_scan");
// ... do work ...
// Automatically recorded when _span drops
```

### Memory Tracking

```rust
/// Memory usage tracker
pub fn get_memory_stats() -> MemoryStats {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
        // Parse VmRSS, VmSize, etc.
    }
    
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::ProcessStatus::*;
        // Use GetProcessMemoryInfo
    }
    
    #[cfg(target_os = "macos")]
    {
        // Use mach APIs
    }
}
```

---

## Configuration Tuning

```toml
# ~/.minion/config/performance.toml

[memory]
# Maximum memory for search index cache (MB)
search_cache_mb = 30

# Maximum embedding cache entries
embedding_cache_size = 10000

# Maximum concurrent active modules
max_active_modules = 3

[threading]
# Worker threads for background tasks
background_workers = 4

# File scanning parallelism
scan_parallelism = 8

# Hash computation parallelism
hash_parallelism = 4

[database]
# SQLite connection pool size
pool_size = 4

# Page cache size (negative = KB)
cache_size = -64000

# Memory-mapped I/O size (bytes)
mmap_size = 268435456

[ui]
# Virtual list overscan rows
virtual_list_overscan = 5

# Debounce delay for search (ms)
search_debounce_ms = 300

# Animation frame budget (ms)
frame_budget_ms = 16
```
