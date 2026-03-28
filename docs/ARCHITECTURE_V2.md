# MINION v2 Architecture Document

## 1. System Architecture

```
+------------------------------------------------------------------+
|                        MINION Desktop App                         |
|                                                                   |
|  +---------------------------+  +------------------------------+  |
|  |     Tauri Shell (Rust)    |  |   SolidJS Frontend (TS)      |  |
|  |  - Window management      |  |  - Command Palette           |  |
|  |  - IPC bridge             |  |  - Multi-tab workspace       |  |
|  |  - System tray            |  |  - Module pages              |  |
|  |  - File dialogs           |  |  - Charts (D3/Canvas)        |  |
|  |  - Native notifications   |  |  - Animations (CSS/WebGL)    |  |
|  +---------------------------+  +------------------------------+  |
|                           |                                       |
|  +--------------------------------------------------------+      |
|  |                    Core Engine (Rust)                    |      |
|  |  +------------+  +-----------+  +-------------------+   |      |
|  |  | Event Bus  |  | Plugin    |  | Task Scheduler    |   |      |
|  |  | (flume)    |  | Manager   |  | (tokio + rayon)   |   |      |
|  |  +------------+  +-----------+  +-------------------+   |      |
|  |  +------------+  +-----------+  +-------------------+   |      |
|  |  | Config     |  | Credential|  | Search Engine     |   |      |
|  |  | Manager    |  | Vault     |  | (Tantivy)         |   |      |
|  |  +------------+  +-----------+  +-------------------+   |      |
|  +--------------------------------------------------------+      |
|                           |                                       |
|  +--------------------------------------------------------+      |
|  |                   Module Layer                          |      |
|  |  +--------+ +-------+ +------+ +-------+ +----------+  |      |
|  |  | Media  | | Files | | Blog | |Finance| | Fitness  |  |      |
|  |  | Engine | | Intel | | AI   | | Intel | | Wellness |  |      |
|  |  +--------+ +-------+ +------+ +-------+ +----------+  |      |
|  |  +--------+ +------------------------------------------+      |
|  |  | Reader |                                             |      |
|  |  | Engine |                                             |      |
|  |  +--------+                                             |      |
|  +--------------------------------------------------------+      |
|                           |                                       |
|  +--------------------------------------------------------+      |
|  |                  Data Layer                             |      |
|  |  +----------+  +----------+  +-------------------+     |      |
|  |  | SQLite   |  | Tantivy  |  | Encrypted FS      |     |      |
|  |  | (rusqlite)|  | Index    |  | (AES-256-GCM)     |     |      |
|  |  +----------+  +----------+  +-------------------+     |      |
|  +--------------------------------------------------------+      |
|                           |                                       |
|  +--------------------------------------------------------+      |
|  |                AI Layer (Optional)                      |      |
|  |  +------------+  +-----------+  +-------------------+   |      |
|  |  | Ollama     |  | RAG       |  | Embeddings        |   |      |
|  |  | (local LLM)|  | Pipeline  |  | (nomic-embed)     |   |      |
|  |  +------------+  +-----------+  +-------------------+   |      |
|  +--------------------------------------------------------+      |
+------------------------------------------------------------------+
```

## 2. Module Interaction Map

```
                    +----------------+
                    |   Event Bus    |
                    +-------+--------+
                            |
        +-------------------+-------------------+
        |         |         |         |         |
   +----v--+ +---v---+ +---v---+ +---v---+ +---v----+
   | Media | | Files | | Blog  | |Finance| |Fitness |
   +---+---+ +---+---+ +---+---+ +---+---+ +---+----+
       |         |         |         |         |
       |    +----v---------v----+    |         |
       |    |   Search Engine   |    |         |
       |    +-------------------+    |         |
       |                             |         |
   +---v-----------------------------v---------v---+
   |              Reader / Knowledge Base           |
   |  (Book highlights feed Blog, Finance reads,    |
   |   Fitness quotes, Media descriptions)          |
   +------------------------------------------------+
                        |
                +-------v--------+
                | Credential Vault|
                | (OAuth tokens,  |
                |  API keys)      |
                +-----------------+
```

**Cross-module data flows:**
- Reader highlights -> Blog post references
- Reader quotes -> Fitness motivational feed
- Files scanner -> Media ingestion pipeline
- Finance data -> Dashboard analytics
- AI layer -> All modules (summaries, suggestions, scoring)
- Search -> All modules (unified search results)

## 3. Database Schema (SQLite)

```sql
-- Core tables (existing)
-- schema_migrations, config, modules, audit_log, task_queue

-- Module 1: Media Intelligence
CREATE TABLE media_projects (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    file_path TEXT NOT NULL,
    thumbnail_path TEXT,
    duration_seconds REAL,
    codec TEXT, resolution TEXT, bitrate INTEGER,
    status TEXT DEFAULT 'draft', -- draft|scheduled|published|failed
    platform TEXT, -- youtube|tiktok|custom
    platform_id TEXT, -- remote video ID after publish
    platform_url TEXT,
    scheduled_at TEXT,
    published_at TEXT,
    tags TEXT, -- JSON array
    category TEXT,
    visibility TEXT DEFAULT 'private',
    account_id TEXT REFERENCES oauth_accounts(id),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE oauth_accounts (
    id TEXT PRIMARY KEY,
    platform TEXT NOT NULL,
    account_name TEXT NOT NULL,
    access_token_encrypted BLOB,
    refresh_token_encrypted BLOB,
    token_expires_at TEXT,
    scopes TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Module 2: File Intelligence (extend existing)
CREATE TABLE file_scans (
    id TEXT PRIMARY KEY,
    root_path TEXT NOT NULL,
    total_files INTEGER, total_size INTEGER,
    duplicates_found INTEGER, scan_duration_ms INTEGER,
    completed_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE file_index (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    name TEXT, extension TEXT,
    size INTEGER, modified_at TEXT,
    sha256 TEXT, blake3 TEXT, perceptual_hash TEXT,
    exif_data TEXT, -- JSON
    scan_id TEXT REFERENCES file_scans(id),
    indexed_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_file_sha256 ON file_index(sha256);
CREATE INDEX idx_file_size ON file_index(size);

-- Module 3: Blog Engine
CREATE TABLE blog_posts (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL, slug TEXT NOT NULL,
    content TEXT, excerpt TEXT,
    status TEXT DEFAULT 'draft',
    author TEXT,
    tags TEXT, categories TEXT, -- JSON arrays
    seo_score INTEGER,
    word_count INTEGER, reading_time INTEGER,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    published_at TEXT
);

CREATE TABLE blog_platforms (
    id TEXT PRIMARY KEY,
    platform TEXT NOT NULL,
    api_url TEXT, account_id TEXT REFERENCES oauth_accounts(id),
    enabled INTEGER DEFAULT 1,
    config TEXT -- JSON
);

CREATE TABLE blog_publish_log (
    id TEXT PRIMARY KEY,
    post_id TEXT REFERENCES blog_posts(id),
    platform_id TEXT REFERENCES blog_platforms(id),
    status TEXT, remote_url TEXT,
    published_at TEXT, error TEXT
);

-- Module 4: Finance
CREATE TABLE finance_accounts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    account_type TEXT NOT NULL, -- bank|credit_card|investment|loan|wallet
    institution TEXT,
    balance REAL DEFAULT 0, currency TEXT DEFAULT 'INR',
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE finance_transactions (
    id TEXT PRIMARY KEY,
    account_id TEXT REFERENCES finance_accounts(id),
    type TEXT NOT NULL, -- credit|debit
    amount REAL NOT NULL,
    description TEXT, category TEXT,
    tags TEXT, -- JSON
    date TEXT NOT NULL,
    imported_from TEXT, -- csv|pdf|manual
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_tx_date ON finance_transactions(date);
CREATE INDEX idx_tx_category ON finance_transactions(category);

CREATE TABLE finance_investments (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    type TEXT, -- stock|mutual_fund|etf|bond|crypto|sip
    symbol TEXT, exchange TEXT, -- NSE|BSE
    purchase_price REAL, current_price REAL,
    quantity REAL,
    purchase_date TEXT,
    last_price_update TEXT
);

CREATE TABLE finance_goals (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    target_amount REAL, current_amount REAL,
    deadline TEXT, priority INTEGER DEFAULT 50,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Module 5: Fitness
CREATE TABLE fitness_workouts (
    id TEXT PRIMARY KEY,
    name TEXT, exercises TEXT, -- JSON
    duration_minutes REAL,
    calories_burned REAL,
    date TEXT NOT NULL,
    notes TEXT
);

CREATE TABLE fitness_habits (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    frequency TEXT DEFAULT 'daily',
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE fitness_habit_completions (
    id TEXT PRIMARY KEY,
    habit_id TEXT REFERENCES fitness_habits(id),
    completed_at TEXT NOT NULL
);

CREATE TABLE fitness_metrics (
    id TEXT PRIMARY KEY,
    date TEXT NOT NULL,
    weight_kg REAL, body_fat_pct REAL,
    steps INTEGER, heart_rate_avg INTEGER,
    sleep_hours REAL, sleep_quality INTEGER,
    water_ml INTEGER, calories_in INTEGER,
    notes TEXT
);

CREATE TABLE fitness_nutrition (
    id TEXT PRIMARY KEY,
    name TEXT, calories REAL,
    protein_g REAL, carbs_g REAL, fat_g REAL,
    meal_type TEXT, date TEXT NOT NULL
);

-- Module 6: Reader
CREATE TABLE reader_books (
    id TEXT PRIMARY KEY,
    title TEXT, authors TEXT, -- JSON array
    file_path TEXT NOT NULL,
    format TEXT, cover_path TEXT,
    pages INTEGER, current_position TEXT, -- JSON
    progress REAL DEFAULT 0,
    rating INTEGER, favorite INTEGER DEFAULT 0,
    tags TEXT, -- JSON
    added_at TEXT DEFAULT CURRENT_TIMESTAMP,
    last_read_at TEXT
);

CREATE TABLE reader_annotations (
    id TEXT PRIMARY KEY,
    book_id TEXT REFERENCES reader_books(id),
    type TEXT, -- highlight|note|bookmark
    chapter_index INTEGER,
    start_pos INTEGER, end_pos INTEGER,
    text TEXT, note TEXT, color TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE reader_reading_sessions (
    id TEXT PRIMARY KEY,
    book_id TEXT REFERENCES reader_books(id),
    started_at TEXT, ended_at TEXT,
    pages_read INTEGER, words_read INTEGER
);

-- Knowledge base (cross-module)
CREATE TABLE knowledge_chunks (
    id TEXT PRIMARY KEY,
    source_type TEXT, -- book|blog|file|note
    source_id TEXT,
    content TEXT NOT NULL,
    embedding BLOB, -- serialized f32 vector
    metadata TEXT, -- JSON
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Reminders & notifications
CREATE TABLE reminders (
    id TEXT PRIMARY KEY,
    module TEXT NOT NULL,
    type TEXT, -- water|breathing|reading|workout|habit
    message TEXT,
    cron_expression TEXT, -- schedule
    enabled INTEGER DEFAULT 1,
    last_triggered TEXT
);
```

## 4. API Contract Design

All Tauri IPC commands follow the pattern:
```
invoke<ResponseType>('module_action', { params }) -> Result<ResponseType, String>
```

### Media Intelligence API
```
media_import_video      { path } -> MediaProject
media_extract_metadata  { project_id } -> MediaMetadata
media_generate_title    { project_id } -> String (AI-powered)
media_generate_tags     { project_id } -> Vec<String>
media_generate_thumb    { project_id, config } -> String (path)
media_upload_youtube    { project_id } -> PublishResult
media_schedule_publish  { project_id, scheduled_at } -> ()
media_list_projects     { status?, limit? } -> Vec<MediaProject>
media_batch_publish     { project_ids } -> Vec<PublishResult>
media_oauth_connect     { platform } -> OAuthUrl
media_oauth_callback    { code } -> Account
```

### File Intelligence API
```
files_start_scan        { path } -> ScanProgress
files_start_multi_scan  { paths } -> ScanProgress
files_get_scan_progress { task_id } -> ScanProgress
files_list_duplicates   {} -> Vec<DuplicateGroup>
files_get_analytics     {} -> StorageAnalytics
files_open_file         { path } -> ()
files_bulk_delete       { paths } -> BulkResult
files_bulk_move         { paths, dest } -> BulkResult
files_get_exif          { path } -> ExifData
files_compare_files     { path_a, path_b } -> CompareResult
```

### Blog AI API
```
blog_create_post        { title, content, author } -> BlogPost
blog_update_post        { id, content } -> ()
blog_publish            { id, platform_ids } -> Vec<PublishResult>
blog_schedule           { id, platforms, scheduled_at } -> ()
blog_analyze_seo        { id } -> SeoAnalysis
blog_generate_tags      { content } -> Vec<String>
blog_suggest_topics     { keywords } -> Vec<TopicSuggestion>
blog_list_posts         { status? } -> Vec<BlogPost>
```

### Finance API
```
finance_import_csv      { path, account_id } -> ImportResult
finance_import_pdf      { path } -> ImportResult
finance_add_account     { name, type, currency } -> Account
finance_add_transaction { account_id, amount, ... } -> Transaction
finance_get_summary     {} -> FinancialSummary
finance_spending_by_cat { from?, to? } -> HashMap<String, f64>
finance_net_worth       {} -> NetWorthBreakdown
finance_track_investment { symbol, exchange } -> Investment
finance_calc_cagr       { initial, final, years } -> f64
finance_fire_projection { ... } -> FireProjection
finance_tax_estimate    { year } -> TaxEstimate
```

### Fitness API
```
fitness_log_workout     { name, exercises, duration } -> Workout
fitness_log_food        { name, calories, ... } -> FoodEntry
fitness_log_metric      { weight?, steps?, ... } -> BodyMetric
fitness_get_dashboard   {} -> FitnessDashboard
fitness_get_streaks     {} -> Vec<HabitStreak>
fitness_toggle_habit    { id } -> Habit
fitness_connect_gfit    {} -> OAuthUrl
fitness_sync_gfit       {} -> SyncResult
fitness_health_score    {} -> HealthAnalysis (AI)
```

### Reader API
```
reader_open_book        { path } -> BookContent
reader_list_books       { directory } -> Vec<BookInfo>
reader_import_book      { path } -> Book
reader_add_annotation   { book_id, type, ... } -> Annotation
reader_search_books     { query } -> Vec<SearchResult>
reader_ask_question     { question, book_ids? } -> AIAnswer (RAG)
reader_summarize_chapter { book_id, chapter } -> String (AI)
reader_generate_flashcards { book_id, chapter } -> Vec<Flashcard>
reader_reading_stats    {} -> ReadingAnalytics
```

## 5. Plugin Interface Spec

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn metadata(&self) -> PluginMetadata;
    async fn initialize(&mut self, ctx: PluginContext) -> Result<()>;
    async fn shutdown(&mut self) -> Result<()>;
    fn capabilities(&self) -> Vec<Capability>;
    async fn handle_event(&self, event: &EventEnvelope) -> Result<Option<Event>>;
    async fn handle_command(&self, cmd: &str, args: Value) -> Result<Value>;
}

pub struct PluginContext {
    pub data_dir: PathBuf,      // Plugin-specific data
    pub cache_dir: PathBuf,     // Temp storage
    pub config: Value,          // Plugin config from user
    pub event_bus: Arc<EventBus>,
    pub permissions: Vec<Permission>,
    pub db: Arc<Database>,      // Shared DB access
    pub search: Arc<SearchIndex>,
}

pub enum Permission {
    FileRead { patterns: Vec<String> },
    FileWrite { patterns: Vec<String> },
    NetworkHttp { hosts: Vec<String> },
    DatabaseRead { tables: Vec<String> },
    DatabaseWrite { tables: Vec<String> },
    CredentialAccess { services: Vec<String> },
    AIModel { models: Vec<String> },
    // ... more
}
```

## 6. Tech Stack Comparison

| Component | Choice | Why | Alternatives Considered |
|-----------|--------|-----|------------------------|
| Backend | Rust | Memory safety, speed, Tauri native | Go (higher RAM), C++ (unsafe) |
| Frontend | SolidJS + TypeScript | Reactive, tiny bundle, fast | React (larger), Svelte (smaller ecosystem) |
| Desktop | Tauri 2 | Native, small binary, secure | Electron (300MB+), Qt (complex) |
| Database | SQLite (rusqlite) | Zero-config, embedded, fast | PostgreSQL (overkill for local) |
| Search | Tantivy | Rust-native Lucene equivalent | MeiliSearch (separate process) |
| Encryption | AES-256-GCM + Argon2id | Industry standard | ChaCha20 (alternative) |
| AI/LLM | Ollama (local) | Privacy, offline, GPU optional | OpenAI API (cloud dependency) |
| Embeddings | nomic-embed-text | Good quality, runs on Ollama | all-MiniLM (smaller, less accurate) |
| Video | FFmpeg (CLI wrapper) | Universal codec support | GStreamer (complex API) |
| Charts | Canvas/SVG (hand-rolled) | No dependency, fast | Chart.js (heavier), D3 (complex) |
| CSS | Tailwind CSS | Utility-first, small output | Plain CSS (slower dev) |
| HTTP | reqwest | Rust-native, async | hyper (lower level) |

## 7. Build System

```
cargo build --workspace              # All Rust crates
cargo tauri dev                      # Dev mode (hot reload)
cargo tauri build                    # Release (.deb, .AppImage, .dmg, .msi)

# Per-platform:
# Linux:   .deb + .AppImage
# macOS:   .dmg
# Windows: .msi + .exe

# CI (GitHub Actions):
# - cargo fmt + clippy + test on every PR
# - Multi-platform release on tag push
```

## 8. Security Model

```
+-------------------+
| User Space        |
| +---------------+ |
| | Tauri Webview | |  <- CSP enforced, no eval(), no remote scripts
| | (SolidJS UI)  | |
| +-------+-------+ |
|         | IPC      |  <- Typed commands only, no arbitrary eval
| +-------v-------+ |
| | Rust Backend  | |  <- Memory safe, no buffer overflows
| | +----------+  | |
| | | Credential|  | |  <- AES-256-GCM encrypted
| | | Vault     |  | |  <- Argon2id key derivation
| | +----------+  | |  <- Zeroize on drop
| | +----------+  | |
| | | OAuth     |  | |  <- Token isolation per service
| | | Manager   |  | |  <- Auto-refresh, encrypted storage
| | +----------+  | |
| | +----------+  | |
| | | File I/O  |  | |  <- Scoped to user-selected dirs
| | +----------+  | |
| +---------------+ |
+-------------------+

Zero telemetry. All data local. No cloud sync unless explicit.
Audit log for all credential access.
```

## 9. Scaling Model

| Scale | Files | Books | Transactions | Strategy |
|-------|-------|-------|-------------|----------|
| Small | <10K | <100 | <1K | In-memory + SQLite |
| Medium | <100K | <1K | <50K | SQLite WAL + Tantivy index |
| Large | <1M | <5K | <500K | Sharded SQLite + background indexing |
| Extreme | 10M+ | 10K+ | 1M+ | SQLite per module + lazy loading |

**Performance targets:**
- Startup: <2 seconds
- Search: <100ms for 1M documents
- File scan: 10K files/second
- RAM baseline: <200MB idle, <500MB active scan

## 10. Offline-First Strategy

- All core features work without internet
- SQLite for all data (no cloud dependency)
- Ollama for local AI (no API keys needed for basic AI)
- Tantivy for local search
- Credential vault works offline
- Sync features (Google Fit, YouTube, blog platforms) queue operations
  and execute when connectivity returns
- Queue stored in `task_queue` table with retry logic

## 11. Roadmap

### v1.0 (Current) - Foundation
- [x] Core engine (event bus, plugins, task scheduler, config)
- [x] SQLite with migrations
- [x] AES-256 credential vault
- [x] File scanner with duplicate detection
- [x] Book reader (EPUB/PDF/TXT/MD)
- [x] Full-text search (Tantivy)
- [x] Basic UI (6 pages, dark mode, sidebar)
- [x] 588 tests passing

### v1.5 - Data Layer + Persistence
- [ ] DB persistence for all modules (migrate from in-memory)
- [ ] CSV/PDF import for Finance
- [ ] EXIF metadata extraction for Files
- [ ] Reading analytics + session tracking for Reader
- [ ] Habit persistence + streak history for Fitness

### v2.0 - API Integrations
- [ ] YouTube Data API v3 (upload, schedule, manage)
- [ ] Google Fit API (sync health data)
- [ ] WordPress REST API (publish blog posts)
- [ ] Medium/Hashnode/Dev.to APIs
- [ ] NSE/BSE market data API
- [ ] OAuth2 flow for all platforms

### v2.5 - AI Layer
- [ ] AI chapter summaries (Ollama)
- [ ] RAG-powered book Q&A
- [ ] AI expense categorization
- [ ] AI health insights
- [ ] AI blog topic generation
- [ ] Semantic search across all modules
- [ ] Flashcard generation

### v3.0 - Premium Features
- [ ] Content calendar (cross-module scheduling)
- [ ] Command palette + keyboard-first workflow
- [ ] Multi-tab workspace
- [ ] FFmpeg video processing pipeline
- [ ] Text-to-speech (offline)
- [ ] Spaced repetition system
- [ ] Physics-based page animations (WebGL)
- [ ] Plugin marketplace
- [ ] Multi-account management

### v4.0 - Intelligence
- [ ] Cross-module knowledge graph
- [ ] Auto concept maps from books
- [ ] Portfolio optimization AI
- [ ] FIRE simulation engine
- [ ] Content performance analytics
- [ ] Smart daily briefing

## 12. Monetization Model

| Tier | Price | Features |
|------|-------|----------|
| **Free** | $0 | Core modules, 3 books, 1 account, basic scan |
| **Pro** | $9/mo | Unlimited everything, AI features, all integrations |
| **Lifetime** | $149 | Pro forever, early access to new modules |
| **Team** | $29/mo | Shared knowledge base, team dashboards |

Revenue streams:
1. Desktop app license (one-time or subscription)
2. Pro module unlocks
3. AI credits for cloud models (optional)
4. Premium themes/plugins marketplace (30% cut)
5. Enterprise licensing

## 13. AI Models Per Module

| Module | Task | Model | Runs On |
|--------|------|-------|---------|
| Media | Title generation | llama3.2:3b | Ollama (local) |
| Media | Tag generation | llama3.2:3b | Ollama |
| Media | Thumbnail text | llama3.2:3b | Ollama |
| Files | Image similarity | CLIP ViT-B/32 | Local (candle) |
| Blog | Topic ideation | llama3.2:3b | Ollama |
| Blog | SEO optimization | Rule-based + LLM | Hybrid |
| Finance | Categorization | llama3.2:3b fine-tuned | Ollama |
| Finance | Insights | llama3.2:3b | Ollama |
| Fitness | Health analysis | llama3.2:3b | Ollama |
| Fitness | Meal suggestions | llama3.2:3b | Ollama |
| Reader | Summaries | llama3.2:3b | Ollama |
| Reader | Q&A (RAG) | llama3.2:3b + nomic-embed | Ollama |
| Reader | Flashcards | llama3.2:3b | Ollama |
| Search | Embeddings | nomic-embed-text | Ollama |
| All | Semantic search | nomic-embed-text | Ollama |

**No cloud AI dependency.** All models run locally via Ollama.
Optional: Claude/GPT-4 API for higher quality (user provides key).

## 14. Performance Optimization Strategy

| Area | Strategy | Target |
|------|----------|--------|
| Startup | Lazy module loading, pre-compiled SQLite | <2s |
| File scan | rayon parallel + jwalk + size-first hashing | 10K files/s |
| Search | Tantivy with pre-built index, mmap | <100ms |
| UI render | SolidJS fine-grained reactivity, virtual lists | 60fps |
| Memory | Stream processing, no full-file buffers | <300MB idle |
| Database | WAL mode, prepared statements, connection pool | <5ms queries |
| AI | Ollama with GPU acceleration, batched embeddings | <2s response |
| Encryption | Hardware AES-NI instructions | <1ms per op |
| Build | LTO, strip, panic=abort in release | <15MB binary |
| Images | Lazy loading, thumbnail cache, WebP conversion | Instant scroll |
