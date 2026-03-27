# MINION Database Schema Design

## Overview

MINION uses a multi-database architecture:
- **SQLite (libsql)**: Primary relational data
- **Tantivy**: Full-text search index
- **usearch**: Vector embeddings for semantic search

## Database Files

```
~/.minion/
├── data/
│   ├── minion.db              # Main SQLite database
│   ├── minion.db-wal          # Write-ahead log
│   ├── search/                # Tantivy index directory
│   │   ├── meta.json
│   │   └── segments/
│   └── vectors/               # Vector index
│       └── embeddings.usearch
├── vault/
│   └── credentials.enc        # Encrypted credential store
└── cache/
    └── thumbnails/            # Thumbnail cache
```

---

## Core Tables

### System Tables

```sql
-- Application configuration
CREATE TABLE config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    encrypted BOOLEAN DEFAULT FALSE,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Module registry
CREATE TABLE modules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    enabled BOOLEAN DEFAULT TRUE,
    permissions TEXT,  -- JSON array of permissions
    config TEXT,       -- JSON module-specific config
    installed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- User roles and access control
CREATE TABLE roles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    permissions TEXT NOT NULL,  -- JSON array
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE user_roles (
    user_id TEXT,
    role_id TEXT,
    PRIMARY KEY (user_id, role_id),
    FOREIGN KEY (role_id) REFERENCES roles(id)
);

-- Audit log (local only)
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    module_id TEXT,
    action TEXT NOT NULL,
    details TEXT,  -- JSON
    FOREIGN KEY (module_id) REFERENCES modules(id)
);

-- Background task queue
CREATE TABLE task_queue (
    id TEXT PRIMARY KEY,
    module_id TEXT NOT NULL,
    task_type TEXT NOT NULL,
    payload TEXT,  -- JSON
    priority INTEGER DEFAULT 50,
    status TEXT DEFAULT 'pending',  -- pending, running, completed, failed
    retry_count INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 3,
    scheduled_at TIMESTAMP,
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    error TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (module_id) REFERENCES modules(id)
);

CREATE INDEX idx_task_queue_status ON task_queue(status, priority, scheduled_at);
```

---

## Module 1: Media Intelligence

```sql
-- Media library
CREATE TABLE media_files (
    id TEXT PRIMARY KEY,
    file_path TEXT NOT NULL UNIQUE,
    file_name TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    mime_type TEXT,
    duration_seconds REAL,
    width INTEGER,
    height INTEGER,
    codec TEXT,
    bitrate INTEGER,
    sha256 TEXT,
    perceptual_hash TEXT,
    thumbnail_path TEXT,
    metadata TEXT,  -- JSON: EXIF, codec info, etc.
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_media_files_hash ON media_files(sha256);
CREATE INDEX idx_media_files_phash ON media_files(perceptual_hash);

-- YouTube accounts
CREATE TABLE youtube_accounts (
    id TEXT PRIMARY KEY,
    channel_id TEXT NOT NULL UNIQUE,
    channel_name TEXT NOT NULL,
    email TEXT,
    access_token_enc TEXT,  -- Encrypted
    refresh_token_enc TEXT, -- Encrypted
    token_expiry TIMESTAMP,
    quota_used INTEGER DEFAULT 0,
    quota_reset_at TIMESTAMP,
    is_default BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Upload queue and history
CREATE TABLE youtube_uploads (
    id TEXT PRIMARY KEY,
    media_file_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    youtube_video_id TEXT,
    
    -- Video metadata
    title TEXT NOT NULL,
    description TEXT,
    tags TEXT,           -- JSON array
    category_id INTEGER,
    privacy_status TEXT DEFAULT 'private',
    playlist_ids TEXT,   -- JSON array
    
    -- Scheduling
    scheduled_at TIMESTAMP,
    published_at TIMESTAMP,
    
    -- Generated content
    ai_title TEXT,
    ai_description TEXT,
    ai_tags TEXT,        -- JSON array
    thumbnail_path TEXT,
    
    -- Status
    status TEXT DEFAULT 'draft',  -- draft, queued, uploading, processing, published, failed
    progress_percent INTEGER DEFAULT 0,
    error TEXT,
    retry_count INTEGER DEFAULT 0,
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (media_file_id) REFERENCES media_files(id),
    FOREIGN KEY (account_id) REFERENCES youtube_accounts(id)
);

CREATE INDEX idx_youtube_uploads_status ON youtube_uploads(status);
CREATE INDEX idx_youtube_uploads_scheduled ON youtube_uploads(scheduled_at);

-- Publishing log
CREATE TABLE publish_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    upload_id TEXT NOT NULL,
    action TEXT NOT NULL,
    details TEXT,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (upload_id) REFERENCES youtube_uploads(id)
);
```

---

## Module 2: File Intelligence

```sql
-- Scanned directories
CREATE TABLE scan_directories (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    recursive BOOLEAN DEFAULT TRUE,
    include_patterns TEXT,  -- JSON array of glob patterns
    exclude_patterns TEXT,  -- JSON array of glob patterns
    last_scan_at TIMESTAMP,
    file_count INTEGER DEFAULT 0,
    total_size INTEGER DEFAULT 0,
    enabled BOOLEAN DEFAULT TRUE
);

-- File index
CREATE TABLE file_index (
    id TEXT PRIMARY KEY,
    directory_id TEXT,
    file_path TEXT NOT NULL UNIQUE,
    file_name TEXT NOT NULL,
    extension TEXT,
    file_size INTEGER NOT NULL,
    
    -- Hashes
    sha256 TEXT,
    sha1 TEXT,
    md5 TEXT,
    perceptual_hash TEXT,     -- For images/videos
    audio_fingerprint TEXT,   -- For audio files
    
    -- Metadata
    mime_type TEXT,
    is_raw BOOLEAN DEFAULT FALSE,
    metadata TEXT,  -- JSON
    
    -- Timestamps
    created_at TIMESTAMP,
    modified_at TIMESTAMP,
    accessed_at TIMESTAMP,
    indexed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (directory_id) REFERENCES scan_directories(id)
);

CREATE INDEX idx_file_index_sha256 ON file_index(sha256);
CREATE INDEX idx_file_index_phash ON file_index(perceptual_hash);
CREATE INDEX idx_file_index_size ON file_index(file_size);
CREATE INDEX idx_file_index_ext ON file_index(extension);

-- Duplicate groups
CREATE TABLE duplicate_groups (
    id TEXT PRIMARY KEY,
    match_type TEXT NOT NULL,  -- exact, perceptual, audio, near
    similarity_score REAL,
    file_count INTEGER,
    total_size INTEGER,
    wasted_space INTEGER,  -- size * (count - 1)
    status TEXT DEFAULT 'pending',  -- pending, reviewed, resolved
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE duplicate_group_members (
    group_id TEXT,
    file_id TEXT,
    is_original BOOLEAN DEFAULT FALSE,
    keep BOOLEAN,
    PRIMARY KEY (group_id, file_id),
    FOREIGN KEY (group_id) REFERENCES duplicate_groups(id),
    FOREIGN KEY (file_id) REFERENCES file_index(id)
);

-- Storage analytics
CREATE TABLE storage_analytics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    directory_id TEXT,
    snapshot_date DATE NOT NULL,
    total_files INTEGER,
    total_size INTEGER,
    by_extension TEXT,  -- JSON: {".mp4": {count, size}, ...}
    by_age TEXT,        -- JSON: {"<1month": {count, size}, ...}
    duplicates_found INTEGER,
    duplicate_size INTEGER,
    FOREIGN KEY (directory_id) REFERENCES scan_directories(id)
);
```

---

## Module 3: Blog AI Engine

```sql
-- Blog platforms
CREATE TABLE blog_platforms (
    id TEXT PRIMARY KEY,
    platform_type TEXT NOT NULL,  -- wordpress, medium, hashnode, devto, custom
    name TEXT NOT NULL,
    api_endpoint TEXT,
    credentials_ref TEXT,  -- Reference to credential vault
    is_default BOOLEAN DEFAULT FALSE,
    config TEXT,  -- JSON platform-specific config
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Blog posts
CREATE TABLE blog_posts (
    id TEXT PRIMARY KEY,
    
    -- Content
    title TEXT NOT NULL,
    slug TEXT,
    content_markdown TEXT,
    content_html TEXT,
    excerpt TEXT,
    
    -- SEO
    meta_title TEXT,
    meta_description TEXT,
    keywords TEXT,       -- JSON array
    seo_score INTEGER,
    
    -- Categorization
    category TEXT,
    tags TEXT,           -- JSON array
    
    -- AI-generated
    ai_suggestions TEXT,  -- JSON: {titles: [], tags: [], keywords: []}
    
    -- Status
    status TEXT DEFAULT 'draft',  -- draft, scheduled, published
    
    -- Timestamps
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    scheduled_at TIMESTAMP,
    published_at TIMESTAMP
);

CREATE INDEX idx_blog_posts_status ON blog_posts(status);

-- Cross-post tracking
CREATE TABLE blog_publications (
    id TEXT PRIMARY KEY,
    post_id TEXT NOT NULL,
    platform_id TEXT NOT NULL,
    external_id TEXT,         -- ID on the platform
    external_url TEXT,
    status TEXT DEFAULT 'pending',
    published_at TIMESTAMP,
    error TEXT,
    FOREIGN KEY (post_id) REFERENCES blog_posts(id),
    FOREIGN KEY (platform_id) REFERENCES blog_platforms(id)
);

-- SEO tracking
CREATE TABLE seo_rankings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id TEXT NOT NULL,
    keyword TEXT NOT NULL,
    position INTEGER,
    search_engine TEXT DEFAULT 'google',
    tracked_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (post_id) REFERENCES blog_posts(id)
);

-- Traffic analytics cache
CREATE TABLE traffic_analytics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id TEXT,
    platform_id TEXT,
    date DATE NOT NULL,
    views INTEGER DEFAULT 0,
    unique_visitors INTEGER DEFAULT 0,
    avg_time_seconds INTEGER,
    bounce_rate REAL,
    source TEXT,  -- JSON: {organic, social, direct, referral}
    FOREIGN KEY (post_id) REFERENCES blog_posts(id),
    FOREIGN KEY (platform_id) REFERENCES blog_platforms(id)
);
```

---

## Module 4: Finance Intelligence

```sql
-- Accounts
CREATE TABLE finance_accounts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    account_type TEXT NOT NULL,  -- bank, credit_card, investment, loan, wallet
    institution TEXT,
    account_number_masked TEXT,  -- Last 4 digits only
    currency TEXT DEFAULT 'INR',
    current_balance REAL,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Transactions
CREATE TABLE transactions (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    date DATE NOT NULL,
    description TEXT NOT NULL,
    amount REAL NOT NULL,
    transaction_type TEXT NOT NULL,  -- credit, debit
    
    -- Categorization
    category TEXT,
    subcategory TEXT,
    tags TEXT,  -- JSON array
    
    -- Source
    import_source TEXT,  -- manual, csv, pdf, api
    original_description TEXT,
    
    -- AI categorization
    ai_category TEXT,
    ai_confidence REAL,
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (account_id) REFERENCES finance_accounts(id)
);

CREATE INDEX idx_transactions_date ON transactions(date);
CREATE INDEX idx_transactions_category ON transactions(category);
CREATE INDEX idx_transactions_account ON transactions(account_id);

-- Categories
CREATE TABLE expense_categories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    parent_id TEXT,
    icon TEXT,
    color TEXT,
    budget_monthly REAL,
    FOREIGN KEY (parent_id) REFERENCES expense_categories(id)
);

-- Investment holdings
CREATE TABLE investment_holdings (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    symbol TEXT NOT NULL,
    name TEXT NOT NULL,
    holding_type TEXT NOT NULL,  -- stock, mutual_fund, etf, bond, crypto
    
    -- Position
    quantity REAL NOT NULL,
    avg_buy_price REAL NOT NULL,
    current_price REAL,
    current_value REAL,
    
    -- Performance
    total_gain_loss REAL,
    total_gain_loss_percent REAL,
    day_gain_loss REAL,
    
    -- Metadata
    exchange TEXT,  -- NSE, BSE
    isin TEXT,
    
    last_updated TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (account_id) REFERENCES finance_accounts(id)
);

CREATE INDEX idx_holdings_symbol ON investment_holdings(symbol);

-- Investment transactions
CREATE TABLE investment_transactions (
    id TEXT PRIMARY KEY,
    holding_id TEXT NOT NULL,
    transaction_type TEXT NOT NULL,  -- buy, sell, dividend, split, bonus
    date DATE NOT NULL,
    quantity REAL NOT NULL,
    price REAL NOT NULL,
    total_amount REAL NOT NULL,
    fees REAL DEFAULT 0,
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (holding_id) REFERENCES investment_holdings(id)
);

-- SIP tracking
CREATE TABLE sip_investments (
    id TEXT PRIMARY KEY,
    holding_id TEXT NOT NULL,
    amount REAL NOT NULL,
    frequency TEXT NOT NULL,  -- monthly, weekly, quarterly
    start_date DATE NOT NULL,
    end_date DATE,
    next_date DATE,
    is_active BOOLEAN DEFAULT TRUE,
    total_invested REAL DEFAULT 0,
    units_accumulated REAL DEFAULT 0,
    FOREIGN KEY (holding_id) REFERENCES investment_holdings(id)
);

-- Financial goals
CREATE TABLE financial_goals (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    target_amount REAL NOT NULL,
    current_amount REAL DEFAULT 0,
    target_date DATE,
    goal_type TEXT,  -- emergency_fund, retirement, house, car, education, vacation, custom
    priority INTEGER DEFAULT 50,
    linked_accounts TEXT,  -- JSON array of account IDs
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Net worth snapshots
CREATE TABLE net_worth_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_date DATE NOT NULL UNIQUE,
    total_assets REAL NOT NULL,
    total_liabilities REAL NOT NULL,
    net_worth REAL NOT NULL,
    breakdown TEXT,  -- JSON: {bank: X, investments: Y, ...}
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Tax records
CREATE TABLE tax_records (
    id TEXT PRIMARY KEY,
    financial_year TEXT NOT NULL,
    income_salary REAL DEFAULT 0,
    income_interest REAL DEFAULT 0,
    income_dividends REAL DEFAULT 0,
    capital_gains_short REAL DEFAULT 0,
    capital_gains_long REAL DEFAULT 0,
    deductions_80c REAL DEFAULT 0,
    deductions_80d REAL DEFAULT 0,
    deductions_other REAL DEFAULT 0,
    tax_paid REAL DEFAULT 0,
    tax_liability REAL DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

---

## Module 5: Fitness & Wellness

```sql
-- User profile
CREATE TABLE fitness_profile (
    id TEXT PRIMARY KEY DEFAULT 'default',
    name TEXT,
    birth_date DATE,
    gender TEXT,
    height_cm REAL,
    target_weight_kg REAL,
    activity_level TEXT,  -- sedentary, light, moderate, active, very_active
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Weight tracking
CREATE TABLE weight_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date DATE NOT NULL UNIQUE,
    weight_kg REAL NOT NULL,
    body_fat_percent REAL,
    muscle_mass_kg REAL,
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Workouts
CREATE TABLE workouts (
    id TEXT PRIMARY KEY,
    date DATE NOT NULL,
    name TEXT NOT NULL,
    workout_type TEXT,  -- strength, cardio, flexibility, sports
    duration_minutes INTEGER,
    calories_burned INTEGER,
    notes TEXT,
    rating INTEGER,  -- 1-5
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE workout_exercises (
    id TEXT PRIMARY KEY,
    workout_id TEXT NOT NULL,
    exercise_name TEXT NOT NULL,
    sets INTEGER,
    reps TEXT,  -- Can be "12,10,8" for drop sets
    weight_kg REAL,
    duration_seconds INTEGER,
    distance_km REAL,
    notes TEXT,
    order_index INTEGER,
    FOREIGN KEY (workout_id) REFERENCES workouts(id)
);

-- Exercise library
CREATE TABLE exercises (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    category TEXT,  -- chest, back, legs, shoulders, arms, core, cardio
    equipment TEXT,
    instructions TEXT,
    video_url TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Nutrition logging
CREATE TABLE nutrition_log (
    id TEXT PRIMARY KEY,
    date DATE NOT NULL,
    meal_type TEXT,  -- breakfast, lunch, dinner, snack
    food_name TEXT NOT NULL,
    quantity REAL,
    unit TEXT,
    calories INTEGER,
    protein_g REAL,
    carbs_g REAL,
    fat_g REAL,
    fiber_g REAL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_nutrition_date ON nutrition_log(date);

-- Habits
CREATE TABLE habits (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    frequency TEXT DEFAULT 'daily',  -- daily, weekly, custom
    target_count INTEGER DEFAULT 1,
    reminder_time TIME,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE habit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    habit_id TEXT NOT NULL,
    date DATE NOT NULL,
    completed BOOLEAN DEFAULT FALSE,
    count INTEGER DEFAULT 0,
    notes TEXT,
    FOREIGN KEY (habit_id) REFERENCES habits(id),
    UNIQUE(habit_id, date)
);

-- Sleep tracking
CREATE TABLE sleep_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date DATE NOT NULL UNIQUE,
    bedtime TIME,
    wake_time TIME,
    duration_hours REAL,
    quality INTEGER,  -- 1-5
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Water intake
CREATE TABLE water_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date DATE NOT NULL,
    time TIME,
    amount_ml INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_water_date ON water_log(date);

-- 75 Hard tracking
CREATE TABLE challenge_75hard (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date DATE NOT NULL UNIQUE,
    day_number INTEGER NOT NULL,
    workout_1_done BOOLEAN DEFAULT FALSE,
    workout_2_outdoor_done BOOLEAN DEFAULT FALSE,
    diet_followed BOOLEAN DEFAULT FALSE,
    water_gallon_done BOOLEAN DEFAULT FALSE,
    reading_done BOOLEAN DEFAULT FALSE,
    progress_photo_done BOOLEAN DEFAULT FALSE,
    no_alcohol BOOLEAN DEFAULT TRUE,
    day_completed BOOLEAN DEFAULT FALSE,
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

---

## Module 6: Book Reader

```sql
-- Library
CREATE TABLE books (
    id TEXT PRIMARY KEY,
    
    -- File info
    file_path TEXT NOT NULL UNIQUE,
    file_format TEXT NOT NULL,  -- epub, pdf, mobi, azw, md, html, txt
    file_size INTEGER,
    file_hash TEXT,
    
    -- Metadata
    title TEXT NOT NULL,
    subtitle TEXT,
    authors TEXT,      -- JSON array
    publisher TEXT,
    publish_date DATE,
    isbn TEXT,
    language TEXT,
    
    -- Cover
    cover_path TEXT,
    cover_color TEXT,  -- Dominant color for placeholder
    
    -- Reading info
    total_pages INTEGER,
    total_words INTEGER,
    estimated_hours REAL,
    
    -- Content
    table_of_contents TEXT,  -- JSON
    
    -- User data
    date_added TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_opened TIMESTAMP,
    is_favorite BOOLEAN DEFAULT FALSE,
    rating INTEGER,  -- 1-5
    
    -- Collections
    collections TEXT,  -- JSON array of collection IDs
    tags TEXT          -- JSON array
);

CREATE INDEX idx_books_title ON books(title);
CREATE INDEX idx_books_authors ON books(authors);

-- Reading progress
CREATE TABLE reading_progress (
    book_id TEXT PRIMARY KEY,
    current_position TEXT,  -- Format-specific position
    current_page INTEGER,
    progress_percent REAL,
    time_spent_minutes INTEGER DEFAULT 0,
    started_at TIMESTAMP,
    finished_at TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (book_id) REFERENCES books(id)
);

-- Reading sessions
CREATE TABLE reading_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_id TEXT NOT NULL,
    started_at TIMESTAMP NOT NULL,
    ended_at TIMESTAMP,
    duration_minutes INTEGER,
    pages_read INTEGER,
    start_position TEXT,
    end_position TEXT,
    FOREIGN KEY (book_id) REFERENCES books(id)
);

-- Highlights and annotations
CREATE TABLE book_annotations (
    id TEXT PRIMARY KEY,
    book_id TEXT NOT NULL,
    
    -- Location
    chapter_index INTEGER,
    chapter_title TEXT,
    position_start TEXT,
    position_end TEXT,
    page_number INTEGER,
    
    -- Content
    highlighted_text TEXT,
    annotation_text TEXT,
    annotation_type TEXT DEFAULT 'highlight',  -- highlight, note, bookmark
    color TEXT DEFAULT 'yellow',
    
    -- AI-generated
    ai_summary TEXT,
    ai_tags TEXT,  -- JSON array
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (book_id) REFERENCES books(id)
);

CREATE INDEX idx_annotations_book ON book_annotations(book_id);

-- Book collections
CREATE TABLE book_collections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    cover_books TEXT,  -- JSON: first 4 book IDs for cover grid
    sort_order INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Reading goals
CREATE TABLE reading_goals (
    id TEXT PRIMARY KEY,
    goal_type TEXT NOT NULL,  -- books_per_year, pages_per_day, minutes_per_day
    target_value INTEGER NOT NULL,
    current_value INTEGER DEFAULT 0,
    year INTEGER,
    month INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Daily reading stats
CREATE TABLE reading_stats (
    date DATE PRIMARY KEY,
    minutes_read INTEGER DEFAULT 0,
    pages_read INTEGER DEFAULT 0,
    books_count INTEGER DEFAULT 0,
    streak_days INTEGER DEFAULT 0
);

-- Knowledge base (for RAG)
CREATE TABLE book_knowledge (
    id TEXT PRIMARY KEY,
    book_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    chapter_index INTEGER,
    content TEXT NOT NULL,
    embedding_id TEXT,  -- Reference to vector store
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (book_id) REFERENCES books(id)
);

CREATE INDEX idx_knowledge_book ON book_knowledge(book_id);

-- Cross-book concepts
CREATE TABLE book_concepts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    linked_annotations TEXT,  -- JSON array of annotation IDs
    linked_books TEXT,        -- JSON array of book IDs
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

---

## Vector Store Schema (usearch)

```
Embeddings Index Structure:
├── book_chunks      (dim: 384/768/1536, metric: cosine)
├── blog_content     (dim: 384/768/1536, metric: cosine)
├── file_metadata    (dim: 384/768/1536, metric: cosine)
└── user_queries     (dim: 384/768/1536, metric: cosine)

Metadata stored in SQLite, vectors in usearch index.
```

---

## Migration Strategy

```sql
-- migrations/001_initial.sql
-- migrations/002_add_indexes.sql
-- migrations/003_add_vector_support.sql
-- ...

-- Version tracking
CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```
