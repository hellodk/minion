//! Integration tests for MINION cross-crate functionality
//!
//! These tests verify that different crates work together correctly.

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    /// Test that the database and crypto modules work together
    /// for storing encrypted configuration
    #[test]
    fn test_database_with_crypto_encryption() {
        // Create a temporary database
        let db = minion_db::in_memory().expect("Failed to create in-memory database");
        db.migrate().expect("Failed to run migrations");

        let conn = db.get().expect("Failed to get connection");

        // Derive an encryption key
        let master_key =
            minion_crypto::MasterKey::derive("test_password").expect("Key derivation failed");
        let db_key = master_key.derive_subkey("database");

        // Store some encrypted config
        let secret_value = "super_secret_api_key";
        let encrypted = minion_crypto::encrypt(db_key.as_bytes(), secret_value.as_bytes())
            .expect("Encryption failed");

        conn.execute(
            "INSERT INTO config (key, value, encrypted) VALUES (?, ?, 1)",
            rusqlite::params!["api_key", hex::encode(&encrypted)],
        )
        .expect("Failed to insert encrypted config");

        // Retrieve and decrypt
        let stored: String = conn
            .query_row(
                "SELECT value FROM config WHERE key = ?",
                ["api_key"],
                |row| row.get(0),
            )
            .expect("Failed to query config");

        let ciphertext = hex::decode(&stored).expect("Failed to decode hex");
        let decrypted =
            minion_crypto::decrypt(db_key.as_bytes(), &ciphertext).expect("Decryption failed");

        assert_eq!(String::from_utf8(decrypted).unwrap(), secret_value);
    }

    /// Test file scanning and duplicate detection together
    #[test]
    fn test_file_scanner_with_duplicates() {
        use std::fs::File;
        use std::io::Write;

        let dir = tempdir().expect("Failed to create temp dir");

        // Create files with same content (duplicates)
        let content = "This is duplicate content";
        for name in &["file1.txt", "file2.txt", "file3.txt"] {
            let path = dir.path().join(name);
            let mut file = File::create(&path).expect("Failed to create file");
            file.write_all(content.as_bytes()).expect("Failed to write");
        }

        // Create a unique file
        let unique_path = dir.path().join("unique.txt");
        let mut unique_file = File::create(&unique_path).expect("Failed to create unique file");
        unique_file
            .write_all(b"This is unique content")
            .expect("Failed to write");

        // Scan the directory
        let config = minion_files::ScanConfig {
            root: dir.path().to_path_buf(),
            compute_hashes: true,
            ..Default::default()
        };

        let scanner = minion_files::Scanner::new(config);
        let result = scanner.scan().expect("Scan failed");

        assert_eq!(result.files.len(), 4);

        // Find duplicates
        let duplicates = minion_files::duplicates::find_exact_duplicates(&result.files);

        // Should find one group of 3 duplicates
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0].files.len(), 3);
        assert_eq!(duplicates[0].match_type, minion_files::DuplicateType::Exact);
    }

    /// Test the crypto vault with persistence
    #[test]
    fn test_crypto_vault_persistence() {
        let dir = tempdir().expect("Failed to create temp dir");
        let vault_path = dir.path().join("vault.enc");

        let master_key =
            minion_crypto::MasterKey::derive("vault_password").expect("Key derivation failed");
        let salt = master_key.salt().to_string();

        // Create vault and store credentials
        {
            let vault_key = master_key.derive_subkey("vault");
            let mut vault = minion_crypto::CredentialVault::open(&vault_path, vault_key)
                .expect("Failed to open vault");

            vault
                .store(minion_crypto::Credential::api_key(
                    "github",
                    "ghp_test_token",
                ))
                .expect("Failed to store credential");
            vault
                .store(minion_crypto::Credential::password(
                    "database",
                    "db_password",
                ))
                .expect("Failed to store credential");

            assert_eq!(vault.list_services().len(), 2);
        }

        // Re-open vault with same password and verify
        {
            let master_key2 = minion_crypto::MasterKey::derive_with_salt("vault_password", &salt)
                .expect("Key derivation failed");
            let vault_key2 = master_key2.derive_subkey("vault");
            let vault = minion_crypto::CredentialVault::open(&vault_path, vault_key2)
                .expect("Failed to open vault");

            assert!(vault.exists("github"));
            assert!(vault.exists("database"));

            let github = vault.get("github").expect("Credential not found");
            match &github.credential_type {
                minion_crypto::CredentialType::ApiKey { key } => {
                    assert_eq!(key, "ghp_test_token");
                }
                _ => panic!("Wrong credential type"),
            }
        }
    }

    /// Test core config with file persistence
    #[test]
    fn test_core_config_save_load() {
        let dir = tempdir().expect("Failed to create temp dir");
        let config_path = dir.path().join("config.toml");

        // Create and modify config
        let mut config = minion_core::Config::default();
        config.config_dir = dir.path().to_path_buf();
        config.ui.theme = "dark".to_string();
        config.ui.animations = false;
        config.ai.default_model = "mistral:7b".to_string();
        config.database.pool_size = 8;

        // Save
        config.save().expect("Failed to save config");
        assert!(config_path.exists());

        // Load and verify
        let loaded = minion_core::Config::load_from(&config_path).expect("Failed to load config");
        assert_eq!(loaded.ui.theme, "dark");
        assert!(!loaded.ui.animations);
        assert_eq!(loaded.ai.default_model, "mistral:7b");
        assert_eq!(loaded.database.pool_size, 8);
    }

    /// Test database migration creates all expected tables
    #[test]
    fn test_database_migration_creates_all_tables() {
        let db = minion_db::in_memory().expect("Failed to create database");
        db.migrate().expect("Failed to run migrations");

        let conn = db.get().expect("Failed to get connection");

        let expected_tables = [
            "schema_migrations",
            "config",
            "modules",
            "audit_log",
            "task_queue",
        ];

        for table in &expected_tables {
            let exists: bool = conn
                .query_row(
                    &format!(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='{}')",
                    table
                ),
                    [],
                    |row| row.get(0),
                )
                .expect(&format!("Failed to check table {}", table));

            assert!(exists, "Table {} should exist", table);
        }
    }

    /// Test storage analytics calculation
    #[test]
    fn test_file_analytics_calculation() {
        use chrono::{Duration, Utc};

        let now = Utc::now();

        let files = vec![
            minion_files::FileInfo {
                path: "/a.txt".into(),
                name: "a.txt".into(),
                extension: Some("txt".into()),
                size: 1000,
                modified: now,
                sha256: None,
                perceptual_hash: None,
            },
            minion_files::FileInfo {
                path: "/b.txt".into(),
                name: "b.txt".into(),
                extension: Some("txt".into()),
                size: 2000,
                modified: now - Duration::days(10),
                sha256: None,
                perceptual_hash: None,
            },
            minion_files::FileInfo {
                path: "/c.jpg".into(),
                name: "c.jpg".into(),
                extension: Some("jpg".into()),
                size: 5000,
                modified: now - Duration::days(100),
                sha256: None,
                perceptual_hash: None,
            },
        ];

        let calculator = minion_files::AnalyticsCalculator::new(10);
        let analytics = calculator.calculate(&files);

        assert_eq!(analytics.total_files, 3);
        assert_eq!(analytics.total_size, 8000);
        assert_eq!(analytics.by_extension.len(), 2);

        // Check txt extension stats
        let txt_stats = analytics.by_extension.get("txt").unwrap();
        assert_eq!(txt_stats.count, 2);
        assert_eq!(txt_stats.total_size, 3000);

        // Check jpg extension stats
        let jpg_stats = analytics.by_extension.get("jpg").unwrap();
        assert_eq!(jpg_stats.count, 1);
        assert_eq!(jpg_stats.total_size, 5000);

        // Check age distribution
        // a.txt: now -> last_day
        // b.txt: now - 10 days -> last_month (7-30 days ago)
        // c.jpg: now - 100 days -> last_year (30-365 days ago)
        assert_eq!(analytics.by_age.last_day.count, 1);
        assert_eq!(analytics.by_age.last_month.count, 1);
        assert_eq!(analytics.by_age.last_year.count, 1);
    }

    /// Test AI embedding similarity functions
    #[test]
    fn test_ai_embedding_operations() {
        use minion_ai::embeddings::{cosine_similarity, euclidean_distance};

        // Create mock embeddings
        let doc1 = vec![0.5, 0.5, 0.5, 0.5];
        let doc2 = vec![0.5, 0.5, 0.5, 0.5]; // Same as doc1
        let doc3 = vec![0.0, 1.0, 0.0, 0.0]; // Different direction

        // Test similarity
        let sim_same = cosine_similarity(&doc1, &doc2);
        assert!(
            (sim_same - 1.0).abs() < 0.001,
            "Same vectors should have similarity 1.0"
        );

        let sim_diff = cosine_similarity(&doc1, &doc3);
        assert!(
            sim_diff < sim_same,
            "Different vectors should have lower similarity"
        );

        // Test distance
        let dist_same = euclidean_distance(&doc1, &doc2);
        assert!(
            dist_same.abs() < 0.001,
            "Same vectors should have distance 0"
        );

        let dist_diff = euclidean_distance(&doc1, &doc3);
        assert!(
            dist_diff > 0.0,
            "Different vectors should have positive distance"
        );
    }

    /// Test reader format detection
    #[test]
    fn test_reader_format_detection() {
        use minion_reader::BookFormat;

        let test_cases = [
            ("book.epub", Some(BookFormat::Epub)),
            ("book.EPUB", Some(BookFormat::Epub)),
            ("document.pdf", Some(BookFormat::Pdf)),
            ("ebook.mobi", Some(BookFormat::Mobi)),
            ("kindle.azw3", Some(BookFormat::Azw)),
            ("notes.md", Some(BookFormat::Markdown)),
            ("page.html", Some(BookFormat::Html)),
            ("plain.txt", Some(BookFormat::Txt)),
            ("unknown.xyz", None),
            ("noextension", None),
        ];

        for (filename, expected) in test_cases {
            let ext = std::path::Path::new(filename)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            let format = BookFormat::from_extension(ext);
            assert_eq!(format, expected, "Format mismatch for {}", filename);
        }
    }

    /// Test annotation manager workflow
    #[test]
    fn test_reader_annotation_workflow() {
        use minion_reader::annotations::{Annotation, AnnotationManager};

        let mut manager = AnnotationManager::new();

        // Add highlights
        manager.add(Annotation::highlight("book1", 0, 10, 50, "Important quote"));
        manager.add(Annotation::highlight("book1", 1, 20, 60, "Another quote"));
        manager.add(Annotation::bookmark("book1", 2, 100));

        // Different book
        manager.add(Annotation::highlight("book2", 0, 0, 10, "Book 2 highlight"));

        // Test filtering
        assert_eq!(manager.for_book("book1").len(), 3);
        assert_eq!(manager.for_book("book2").len(), 1);
        assert_eq!(manager.for_chapter("book1", 0).len(), 1);
        assert_eq!(manager.for_chapter("book1", 1).len(), 1);

        // Test export
        let markdown = manager.export_markdown("book1");
        assert!(markdown.contains("Important quote"));
        assert!(markdown.contains("Another quote"));
        assert!(markdown.contains("📌 Bookmark"));
    }

    /// Test hash functions consistency
    #[test]
    fn test_hash_consistency() {
        use std::fs::File;
        use std::io::Write;

        let dir = tempdir().expect("Failed to create temp dir");

        // Create test files with same content
        let content = b"Test content for hashing";

        let path1 = dir.path().join("file1.txt");
        let path2 = dir.path().join("file2.txt");

        File::create(&path1).unwrap().write_all(content).unwrap();
        File::create(&path2).unwrap().write_all(content).unwrap();

        // Hash both files
        let hash1 = minion_files::hash::compute_sha256(&path1).expect("Hash failed");
        let hash2 = minion_files::hash::compute_sha256(&path2).expect("Hash failed");

        // Same content should produce same hash
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA-256 = 64 hex chars

        // BLAKE3 should also be consistent
        let blake1 = minion_files::hash::compute_blake3(&path1).expect("BLAKE3 failed");
        let blake2 = minion_files::hash::compute_blake3(&path2).expect("BLAKE3 failed");

        assert_eq!(blake1, blake2);
        assert_eq!(blake1.len(), 64);
    }

    /// Test hamming distance for perceptual hashing
    #[test]
    fn test_perceptual_hash_hamming() {
        use minion_files::hash::{hamming_distance, is_similar};

        // Identical hashes
        assert_eq!(hamming_distance(0, 0), 0);
        assert_eq!(hamming_distance(0xFFFFFFFFFFFFFFFF, 0xFFFFFFFFFFFFFFFF), 0);

        // One bit different
        assert_eq!(hamming_distance(0b1000, 0b0000), 1);

        // All bits different
        assert_eq!(hamming_distance(0xFF, 0x00), 8);

        // Test similarity with threshold
        assert!(is_similar(0, 0, 0));
        assert!(is_similar(0b1111, 0b1110, 1)); // 1 bit difference, threshold 1
        assert!(!is_similar(0b1111, 0b0000, 3)); // 4 bit difference, threshold 3
    }
} // end of tests module
