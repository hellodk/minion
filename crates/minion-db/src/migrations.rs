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
    let migrations: &[(&str, MigrationFn)] = &[("001_initial", migrate_001_initial)];

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
        assert_eq!(count, 1);

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
