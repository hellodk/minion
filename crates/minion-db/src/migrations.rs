//! Database migrations

use crate::Result;
use rusqlite::Connection;

/// Run all migrations
pub fn run(conn: &Connection) -> Result<()> {
    // Create migrations table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    // Run each migration
    type MigrationFn = fn(&Connection) -> Result<()>;
    let migrations: &[(&str, MigrationFn)] = &[
        ("001_initial", migrate_001_initial),
        ("002_modules", migrate_002_modules),
    ];

    for (name, migrate_fn) in migrations {
        let version: i32 = name[..3].parse().unwrap_or(0);

        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?)",
            [version],
            |row| row.get(0),
        )?;

        if !exists {
            tracing::info!("Running migration: {}", name);
            migrate_fn(conn)?;
            conn.execute(
                "INSERT INTO schema_migrations (version, name) VALUES (?, ?)",
                rusqlite::params![version, name],
            )?;
        }
    }

    Ok(())
}

fn migrate_001_initial(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        -- Configuration
        CREATE TABLE IF NOT EXISTS config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            encrypted INTEGER DEFAULT 0,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        
        -- Modules
        CREATE TABLE IF NOT EXISTS modules (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            version TEXT NOT NULL,
            enabled INTEGER DEFAULT 1,
            permissions TEXT,
            config TEXT,
            installed_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        
        -- Audit log
        CREATE TABLE IF NOT EXISTS audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT DEFAULT CURRENT_TIMESTAMP,
            module_id TEXT,
            action TEXT NOT NULL,
            details TEXT
        );
        
        CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp);
        
        -- Task queue
        CREATE TABLE IF NOT EXISTS task_queue (
            id TEXT PRIMARY KEY,
            module_id TEXT NOT NULL,
            task_type TEXT NOT NULL,
            payload TEXT,
            priority INTEGER DEFAULT 50,
            status TEXT DEFAULT 'pending',
            retry_count INTEGER DEFAULT 0,
            max_retries INTEGER DEFAULT 3,
            scheduled_at TEXT,
            started_at TEXT,
            completed_at TEXT,
            error TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        
        CREATE INDEX IF NOT EXISTS idx_task_status ON task_queue(status, priority, scheduled_at);
    ",
    )?;

    Ok(())
}

fn migrate_002_modules(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        -- OAuth accounts (shared across modules)
        CREATE TABLE IF NOT EXISTS oauth_accounts (
            id TEXT PRIMARY KEY,
            platform TEXT NOT NULL,
            account_name TEXT NOT NULL,
            access_token_encrypted TEXT,
            refresh_token_encrypted TEXT,
            token_expires_at TEXT,
            scopes TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        -- Finance: Accounts
        CREATE TABLE IF NOT EXISTS finance_accounts (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            account_type TEXT NOT NULL,
            institution TEXT,
            balance REAL DEFAULT 0,
            currency TEXT DEFAULT 'INR',
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        -- Finance: Transactions
        CREATE TABLE IF NOT EXISTS finance_transactions (
            id TEXT PRIMARY KEY,
            account_id TEXT REFERENCES finance_accounts(id),
            type TEXT NOT NULL,
            amount REAL NOT NULL,
            description TEXT,
            category TEXT,
            tags TEXT,
            date TEXT NOT NULL,
            imported_from TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_fin_tx_date ON finance_transactions(date);
        CREATE INDEX IF NOT EXISTS idx_fin_tx_category ON finance_transactions(category);
        CREATE INDEX IF NOT EXISTS idx_fin_tx_account ON finance_transactions(account_id);

        -- Finance: Investments
        CREATE TABLE IF NOT EXISTS finance_investments (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            type TEXT,
            symbol TEXT,
            exchange TEXT,
            purchase_price REAL,
            current_price REAL,
            quantity REAL,
            purchase_date TEXT,
            last_price_update TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        -- Finance: Goals
        CREATE TABLE IF NOT EXISTS finance_goals (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            target_amount REAL NOT NULL,
            current_amount REAL DEFAULT 0,
            deadline TEXT,
            priority INTEGER DEFAULT 50,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        -- Fitness: Workouts
        CREATE TABLE IF NOT EXISTS fitness_workouts (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            exercises TEXT,
            duration_minutes REAL,
            calories_burned REAL,
            date TEXT NOT NULL,
            notes TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        -- Fitness: Habits
        CREATE TABLE IF NOT EXISTS fitness_habits (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            frequency TEXT DEFAULT 'daily',
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        -- Fitness: Habit completions
        CREATE TABLE IF NOT EXISTS fitness_habit_completions (
            id TEXT PRIMARY KEY,
            habit_id TEXT NOT NULL REFERENCES fitness_habits(id),
            completed_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_habit_comp ON fitness_habit_completions(habit_id, completed_at);

        -- Fitness: Body metrics
        CREATE TABLE IF NOT EXISTS fitness_metrics (
            id TEXT PRIMARY KEY,
            date TEXT NOT NULL,
            weight_kg REAL,
            body_fat_pct REAL,
            steps INTEGER,
            heart_rate_avg INTEGER,
            sleep_hours REAL,
            sleep_quality INTEGER,
            water_ml INTEGER,
            calories_in INTEGER,
            notes TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_fitness_date ON fitness_metrics(date);

        -- Fitness: Nutrition log
        CREATE TABLE IF NOT EXISTS fitness_nutrition (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            calories REAL,
            protein_g REAL,
            carbs_g REAL,
            fat_g REAL,
            meal_type TEXT,
            date TEXT NOT NULL,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        -- Reader: Books
        CREATE TABLE IF NOT EXISTS reader_books (
            id TEXT PRIMARY KEY,
            title TEXT,
            authors TEXT,
            file_path TEXT NOT NULL,
            format TEXT,
            cover_path TEXT,
            pages INTEGER,
            current_position TEXT,
            progress REAL DEFAULT 0,
            rating INTEGER,
            favorite INTEGER DEFAULT 0,
            tags TEXT,
            added_at TEXT DEFAULT CURRENT_TIMESTAMP,
            last_read_at TEXT
        );

        -- Reader: Annotations
        CREATE TABLE IF NOT EXISTS reader_annotations (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL REFERENCES reader_books(id),
            type TEXT NOT NULL,
            chapter_index INTEGER,
            start_pos INTEGER,
            end_pos INTEGER,
            text TEXT,
            note TEXT,
            color TEXT DEFAULT 'yellow',
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_annot_book ON reader_annotations(book_id);

        -- Reader: Reading sessions
        CREATE TABLE IF NOT EXISTS reader_reading_sessions (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL REFERENCES reader_books(id),
            started_at TEXT NOT NULL,
            ended_at TEXT,
            pages_read INTEGER DEFAULT 0,
            words_read INTEGER DEFAULT 0
        );

        -- Blog: Posts
        CREATE TABLE IF NOT EXISTS blog_posts (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            slug TEXT NOT NULL,
            content TEXT,
            excerpt TEXT,
            status TEXT DEFAULT 'draft',
            author TEXT,
            tags TEXT,
            categories TEXT,
            seo_score INTEGER,
            word_count INTEGER,
            reading_time INTEGER,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
            published_at TEXT
        );

        -- Blog: Publish log
        CREATE TABLE IF NOT EXISTS blog_publish_log (
            id TEXT PRIMARY KEY,
            post_id TEXT REFERENCES blog_posts(id),
            platform TEXT NOT NULL,
            status TEXT,
            remote_url TEXT,
            published_at TEXT,
            error TEXT
        );

        -- Media: Projects
        CREATE TABLE IF NOT EXISTS media_projects (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT,
            file_path TEXT NOT NULL,
            thumbnail_path TEXT,
            duration_seconds REAL,
            codec TEXT,
            resolution TEXT,
            status TEXT DEFAULT 'draft',
            platform TEXT,
            platform_id TEXT,
            platform_url TEXT,
            scheduled_at TEXT,
            published_at TEXT,
            tags TEXT,
            category TEXT,
            visibility TEXT DEFAULT 'private',
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        -- File scan history
        CREATE TABLE IF NOT EXISTS file_scans (
            id TEXT PRIMARY KEY,
            root_path TEXT NOT NULL,
            total_files INTEGER,
            total_size INTEGER,
            duplicates_found INTEGER,
            scan_duration_ms INTEGER,
            completed_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        -- Reminders
        CREATE TABLE IF NOT EXISTS reminders (
            id TEXT PRIMARY KEY,
            module TEXT NOT NULL,
            type TEXT,
            message TEXT,
            cron_expression TEXT,
            enabled INTEGER DEFAULT 1,
            last_triggered TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
    ",
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        Connection::open_in_memory().expect("Failed to create in-memory database")
    }

    #[test]
    fn test_run_migrations() {
        let conn = setup_test_db();
        run(&conn).expect("Failed to run migrations");

        // Verify schema_migrations table exists and has correct entry
        let version: i32 = conn
            .query_row(
                "SELECT version FROM schema_migrations WHERE name = '001_initial'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to query migration version");
        assert_eq!(version, 1);
    }

    #[test]
    fn test_migration_creates_config_table() {
        let conn = setup_test_db();
        run(&conn).expect("Failed to run migrations");

        // Insert and retrieve config
        conn.execute(
            "INSERT INTO config (key, value) VALUES ('test_key', 'test_value')",
            [],
        )
        .expect("Failed to insert config");

        let value: String = conn
            .query_row(
                "SELECT value FROM config WHERE key = 'test_key'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to query config");
        assert_eq!(value, "test_value");
    }

    #[test]
    fn test_migration_creates_modules_table() {
        let conn = setup_test_db();
        run(&conn).expect("Failed to run migrations");

        conn.execute(
            "INSERT INTO modules (id, name, version) VALUES ('test_module', 'Test Module', '1.0.0')",
            [],
        )
        .expect("Failed to insert module");

        let name: String = conn
            .query_row(
                "SELECT name FROM modules WHERE id = 'test_module'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to query module");
        assert_eq!(name, "Test Module");
    }

    #[test]
    fn test_migration_creates_audit_log_table() {
        let conn = setup_test_db();
        run(&conn).expect("Failed to run migrations");

        conn.execute(
            "INSERT INTO audit_log (action, details) VALUES ('test_action', 'test details')",
            [],
        )
        .expect("Failed to insert audit log");

        let action: String = conn
            .query_row("SELECT action FROM audit_log WHERE id = 1", [], |row| {
                row.get(0)
            })
            .expect("Failed to query audit log");
        assert_eq!(action, "test_action");
    }

    #[test]
    fn test_migration_creates_task_queue_table() {
        let conn = setup_test_db();
        run(&conn).expect("Failed to run migrations");

        conn.execute(
            "INSERT INTO task_queue (id, module_id, task_type, status) VALUES ('task1', 'mod1', 'scan', 'pending')",
            [],
        )
        .expect("Failed to insert task");

        let status: String = conn
            .query_row(
                "SELECT status FROM task_queue WHERE id = 'task1'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to query task");
        assert_eq!(status, "pending");
    }

    #[test]
    fn test_migrations_are_recorded() {
        let conn = setup_test_db();
        run(&conn).expect("Failed to run migrations");

        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("Failed to count migrations");
        assert_eq!(count, 2);

        // Verify applied_at is set
        let has_timestamp: bool = conn
            .query_row(
                "SELECT applied_at IS NOT NULL FROM schema_migrations WHERE version = 1",
                [],
                |row| row.get(0),
            )
            .expect("Failed to check timestamp");
        assert!(has_timestamp);
    }

    #[test]
    fn test_task_queue_index_exists() {
        let conn = setup_test_db();
        run(&conn).expect("Failed to run migrations");

        let index_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_task_status')",
                [],
                |row| row.get(0),
            )
            .expect("Failed to check index");
        assert!(index_exists);
    }

    #[test]
    fn test_audit_log_index_exists() {
        let conn = setup_test_db();
        run(&conn).expect("Failed to run migrations");

        let index_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_audit_timestamp')",
                [],
                |row| row.get(0),
            )
            .expect("Failed to check index");
        assert!(index_exists);
    }
}
