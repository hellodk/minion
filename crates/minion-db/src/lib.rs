//! MINION Database Layer
//!
//! Provides SQLite database access with connection pooling and migrations.

use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use std::path::Path;
use thiserror::Error;

pub mod migrations;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Pool error: {0}")]
    Pool(#[from] r2d2::Error),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Database connection pool
pub struct Database {
    pool: Pool<SqliteConnectionManager>,
}

impl Database {
    /// Create a new database connection pool
    pub fn new(path: &Path, pool_size: u32) -> Result<Self> {
        let manager = SqliteConnectionManager::file(path);

        let pool = Pool::builder()
            .max_size(pool_size)
            .min_idle(Some(1))
            .build(manager)?;

        // Initialize with optimizations
        {
            let conn = pool.get()?;
            conn.execute_batch(
                "
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;
                PRAGMA cache_size = -64000;
                PRAGMA temp_store = MEMORY;
                PRAGMA mmap_size = 268435456;
            ",
            )?;
        }

        Ok(Self { pool })
    }

    /// Get a connection from the pool
    pub fn get(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        Ok(self.pool.get()?)
    }

    /// Run migrations
    pub fn migrate(&self) -> Result<()> {
        let conn = self.get()?;
        migrations::run(&conn)?;
        Ok(())
    }
}

/// In-memory database for testing
pub fn in_memory() -> Result<Database> {
    let manager = SqliteConnectionManager::memory();
    let pool = Pool::builder().max_size(1).build(manager)?;

    Ok(Database { pool })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_in_memory_database() {
        let db = in_memory().expect("Failed to create in-memory database");
        let conn = db.get().expect("Failed to get connection");

        // Test basic query
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY)", [])
            .expect("Failed to create table");
        conn.execute("INSERT INTO test (id) VALUES (1)", [])
            .expect("Failed to insert");

        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM test", [], |row| row.get(0))
            .expect("Failed to query");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_file_database_creation() {
        let dir = tempdir().expect("Failed to create temp dir");
        let db_path = dir.path().join("test.db");

        let db = Database::new(&db_path, 4).expect("Failed to create database");
        let conn = db.get().expect("Failed to get connection");

        // Verify WAL mode is enabled
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("Failed to query journal mode");
        assert_eq!(journal_mode.to_lowercase(), "wal");

        // Verify file was created
        assert!(db_path.exists());
    }

    #[test]
    fn test_connection_pool() {
        let db = in_memory().expect("Failed to create database");

        // Get multiple connections (pool size is 1 for in_memory)
        let conn1 = db.get().expect("Failed to get first connection");
        drop(conn1);

        let conn2 = db.get().expect("Failed to get second connection");
        drop(conn2);
    }

    #[test]
    fn test_database_migrations() {
        let db = in_memory().expect("Failed to create database");
        db.migrate().expect("Failed to run migrations");

        let conn = db.get().expect("Failed to get connection");

        // Verify migrations table exists
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_migrations')",
                [],
                |row| row.get(0),
            )
            .expect("Failed to check migrations table");
        assert!(exists);

        // Verify config table was created
        let config_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='config')",
                [],
                |row| row.get(0),
            )
            .expect("Failed to check config table");
        assert!(config_exists);

        // Verify task_queue table was created
        let task_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='task_queue')",
                [],
                |row| row.get(0),
            )
            .expect("Failed to check task_queue table");
        assert!(task_exists);
    }

    #[test]
    fn test_migrations_idempotent() {
        let db = in_memory().expect("Failed to create database");

        // Run migrations twice
        db.migrate().expect("First migration failed");
        db.migrate().expect("Second migration failed");

        let conn = db.get().expect("Failed to get connection");

        // Should still have the same number of migrations
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("Failed to count migrations");
        assert_eq!(count, 8);
    }

    #[test]
    fn test_database_pragmas() {
        let dir = tempdir().expect("Failed to create temp dir");
        let db_path = dir.path().join("test.db");

        let db = Database::new(&db_path, 4).expect("Failed to create database");
        let conn = db.get().expect("Failed to get connection");

        // Verify synchronous mode
        let sync_mode: i32 = conn
            .query_row("PRAGMA synchronous", [], |row| row.get(0))
            .expect("Failed to query synchronous mode");
        assert_eq!(sync_mode, 1); // NORMAL = 1

        // Verify temp_store
        let temp_store: i32 = conn
            .query_row("PRAGMA temp_store", [], |row| row.get(0))
            .expect("Failed to query temp_store");
        assert_eq!(temp_store, 2); // MEMORY = 2
    }

    #[test]
    fn test_error_handling() {
        let db = in_memory().expect("Failed to create database");
        let conn = db.get().expect("Failed to get connection");

        // Try to query non-existent table
        let result = conn.query_row("SELECT * FROM nonexistent", [], |_| Ok(()));
        assert!(result.is_err());
    }
}
