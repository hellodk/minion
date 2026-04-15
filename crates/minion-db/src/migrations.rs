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
        ("003_collections", migrate_003_collections),
        ("004_calendar", migrate_004_calendar),
        ("005_calendar_accounts", migrate_005_calendar_accounts),
        ("006_health_vault", migrate_006_health_vault),
        ("007_health_ingestion", migrate_007_health_ingestion),
        ("008_llm_endpoints", migrate_008_llm_endpoints),
        ("009_health_timeline", migrate_009_health_timeline),
        ("010_health_analysis", migrate_010_health_analysis),
        ("011_health_fk_repair", migrate_011_health_fk_repair),
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

fn migrate_003_collections(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS reader_collections (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            color TEXT DEFAULT '#0ea5e9',
            sort_order INTEGER DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS reader_collection_books (
            collection_id TEXT NOT NULL REFERENCES reader_collections(id) ON DELETE CASCADE,
            book_id TEXT NOT NULL REFERENCES reader_books(id) ON DELETE CASCADE,
            added_at TEXT DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (collection_id, book_id)
        );
        CREATE INDEX IF NOT EXISTS idx_coll_books ON reader_collection_books(collection_id);

        -- O'Reilly/Safari downloaded books tracking
        CREATE TABLE IF NOT EXISTS reader_downloads (
            id TEXT PRIMARY KEY,
            source TEXT NOT NULL DEFAULT 'oreilly',
            remote_id TEXT,
            title TEXT NOT NULL,
            authors TEXT,
            cover_url TEXT,
            download_path TEXT,
            status TEXT DEFAULT 'pending',
            progress REAL DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            completed_at TEXT
        );
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

fn migrate_004_calendar(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS calendar_events (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT,
            start_time TEXT NOT NULL,
            end_time TEXT,
            all_day INTEGER DEFAULT 0,
            location TEXT,
            color TEXT DEFAULT '#0ea5e9',
            source TEXT DEFAULT 'local',
            remote_id TEXT,
            calendar_name TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_cal_start ON calendar_events(start_time);
        CREATE INDEX IF NOT EXISTS idx_cal_source ON calendar_events(source);
    ",
    )?;
    Ok(())
}

fn migrate_005_calendar_accounts(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS calendar_accounts (
            id TEXT PRIMARY KEY,
            provider TEXT NOT NULL,
            email TEXT,
            access_token TEXT NOT NULL,
            refresh_token TEXT,
            expires_at TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_cal_accounts_provider ON calendar_accounts(provider);

        ALTER TABLE calendar_events ADD COLUMN account_id TEXT;
        CREATE INDEX IF NOT EXISTS idx_cal_account ON calendar_events(account_id);
    ",
    )?;
    Ok(())
}

/// Health Vault: longitudinal clinical records.
/// Supports multiple patients (primary user + family members) keyed by
/// phone number. Covers medical records, lab tests, medications,
/// conditions, vitals, life events, symptoms, and family history.
fn migrate_006_health_vault(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        -- =====================================================
        -- PATIENTS (multi-user support)
        -- =====================================================
        CREATE TABLE IF NOT EXISTS patients (
            id TEXT PRIMARY KEY,
            phone_number TEXT UNIQUE NOT NULL,
            full_name TEXT NOT NULL,
            date_of_birth TEXT,
            sex TEXT,
            blood_group TEXT,
            relationship TEXT NOT NULL,
            is_primary INTEGER DEFAULT 0,
            avatar_color TEXT,
            notes TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_primary_patient
            ON patients(is_primary) WHERE is_primary = 1;
        CREATE INDEX IF NOT EXISTS idx_patient_phone ON patients(phone_number);

        -- =====================================================
        -- CONSENT
        -- =====================================================
        CREATE TABLE IF NOT EXISTS health_consent (
            id INTEGER PRIMARY KEY,
            accepted_at TEXT NOT NULL,
            version TEXT NOT NULL,
            local_only_mode INTEGER DEFAULT 1,
            drive_sync_enabled INTEGER DEFAULT 0,
            cloud_llm_allowed INTEGER DEFAULT 0,
            user_signature TEXT
        );

        -- =====================================================
        -- NORMALIZED ENTITIES (doctors, labs, meds, tests)
        -- =====================================================
        CREATE TABLE IF NOT EXISTS health_entities (
            id TEXT PRIMARY KEY,
            entity_type TEXT NOT NULL,
            canonical_name TEXT NOT NULL,
            aliases TEXT,
            metadata TEXT,
            first_seen_at TEXT,
            UNIQUE(entity_type, canonical_name)
        );
        CREATE INDEX IF NOT EXISTS idx_entity_type ON health_entities(entity_type);

        -- =====================================================
        -- EPISODES (groups of related events)
        -- =====================================================
        CREATE TABLE IF NOT EXISTS episodes (
            id TEXT PRIMARY KEY,
            patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            description TEXT,
            start_date TEXT NOT NULL,
            end_date TEXT,
            primary_condition TEXT,
            ai_generated INTEGER DEFAULT 0,
            user_confirmed INTEGER DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_episodes_patient ON episodes(patient_id, start_date DESC);

        -- =====================================================
        -- MEDICAL RECORDS (visits, diagnoses, procedures)
        -- =====================================================
        CREATE TABLE IF NOT EXISTS medical_records (
            id TEXT PRIMARY KEY,
            patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            record_type TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT,
            doctor_id TEXT REFERENCES health_entities(id),
            facility_id TEXT REFERENCES health_entities(id),
            date TEXT NOT NULL,
            tags TEXT,
            document_file_id TEXT,
            episode_id TEXT REFERENCES episodes(id),
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_medical_patient_date
            ON medical_records(patient_id, date DESC);
        CREATE INDEX IF NOT EXISTS idx_medical_type
            ON medical_records(patient_id, record_type);

        -- =====================================================
        -- LAB TESTS
        -- =====================================================
        CREATE TABLE IF NOT EXISTS lab_tests (
            id TEXT PRIMARY KEY,
            patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            record_id TEXT REFERENCES medical_records(id),
            test_name TEXT NOT NULL,
            canonical_name TEXT,
            test_category TEXT,
            value REAL NOT NULL,
            unit TEXT,
            reference_low REAL,
            reference_high REAL,
            reference_text TEXT,
            flag TEXT,
            lab_entity_id TEXT REFERENCES health_entities(id),
            collected_at TEXT NOT NULL,
            reported_at TEXT,
            source TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_lab_patient_name_date
            ON lab_tests(patient_id, canonical_name, collected_at);
        CREATE INDEX IF NOT EXISTS idx_lab_category
            ON lab_tests(patient_id, test_category);
        CREATE INDEX IF NOT EXISTS idx_lab_date
            ON lab_tests(patient_id, collected_at DESC);

        -- =====================================================
        -- MEDICATIONS
        -- =====================================================
        CREATE TABLE IF NOT EXISTS medications_v2 (
            id TEXT PRIMARY KEY,
            patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            generic_name TEXT,
            dose TEXT,
            frequency TEXT,
            route TEXT,
            start_date TEXT,
            end_date TEXT,
            prescribing_doctor_id TEXT REFERENCES health_entities(id),
            indication TEXT,
            notes TEXT,
            record_id TEXT REFERENCES medical_records(id),
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_meds_v2_patient
            ON medications_v2(patient_id, start_date DESC);
        CREATE INDEX IF NOT EXISTS idx_meds_v2_active
            ON medications_v2(patient_id) WHERE end_date IS NULL;

        -- =====================================================
        -- CONDITIONS (chronic, allergies, surgeries)
        -- =====================================================
        CREATE TABLE IF NOT EXISTS health_conditions (
            id TEXT PRIMARY KEY,
            patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            condition_type TEXT,
            severity TEXT,
            diagnosed_at TEXT,
            resolved_at TEXT,
            notes TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_conditions_patient
            ON health_conditions(patient_id);

        -- =====================================================
        -- VITALS (BP, glucose, temperature, etc.)
        -- =====================================================
        CREATE TABLE IF NOT EXISTS vitals (
            id TEXT PRIMARY KEY,
            patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            measurement_type TEXT NOT NULL,
            value REAL NOT NULL,
            unit TEXT,
            measured_at TEXT NOT NULL,
            context TEXT,
            notes TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_vitals_patient_type_date
            ON vitals(patient_id, measurement_type, measured_at DESC);

        -- =====================================================
        -- FAMILY HISTORY
        -- =====================================================
        CREATE TABLE IF NOT EXISTS family_history (
            id TEXT PRIMARY KEY,
            patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            relation TEXT NOT NULL,
            condition TEXT NOT NULL,
            age_at_diagnosis INTEGER,
            notes TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_family_patient ON family_history(patient_id);

        -- =====================================================
        -- LIFE EVENTS (correlation layer)
        -- =====================================================
        CREATE TABLE IF NOT EXISTS life_events (
            id TEXT PRIMARY KEY,
            patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            category TEXT NOT NULL,
            subcategory TEXT,
            title TEXT NOT NULL,
            description TEXT,
            intensity INTEGER,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            tags TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_life_patient_dates
            ON life_events(patient_id, started_at, ended_at);
        CREATE INDEX IF NOT EXISTS idx_life_category
            ON life_events(patient_id, category);

        -- =====================================================
        -- SYMPTOMS (free text with LLM classification)
        -- =====================================================
        CREATE TABLE IF NOT EXISTS symptoms (
            id TEXT PRIMARY KEY,
            patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            description TEXT NOT NULL,
            canonical_name TEXT,
            body_part TEXT,
            laterality TEXT,
            severity INTEGER,
            first_noticed TEXT NOT NULL,
            resolved_at TEXT,
            frequency TEXT,
            triggers TEXT,
            llm_metadata TEXT,
            notes TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_symptoms_patient_date
            ON symptoms(patient_id, first_noticed DESC);
        ",
    )?;
    Ok(())
}

/// Health Vault ingestion pipeline: file manifest, ingestion jobs,
/// and document extraction results.
fn migrate_007_health_ingestion(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS ingestion_jobs (
            id TEXT PRIMARY KEY,
            patient_id TEXT REFERENCES patients(id),
            source_folder TEXT,
            started_at TEXT DEFAULT CURRENT_TIMESTAMP,
            completed_at TEXT,
            status TEXT DEFAULT 'running',
            total_files INTEGER DEFAULT 0,
            processed_files INTEGER DEFAULT 0,
            skipped_files INTEGER DEFAULT 0,
            failed_files INTEGER DEFAULT 0,
            model_used TEXT,
            error TEXT
        );

        CREATE TABLE IF NOT EXISTS file_manifest (
            id TEXT PRIMARY KEY,
            sha256 TEXT NOT NULL UNIQUE,
            original_path TEXT NOT NULL,
            stored_path TEXT,
            mime_type TEXT,
            size_bytes INTEGER,
            status TEXT DEFAULT 'pending',
            patient_id TEXT REFERENCES patients(id),
            job_id TEXT REFERENCES ingestion_jobs(id),
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            error TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_file_status ON file_manifest(status);
        CREATE INDEX IF NOT EXISTS idx_file_patient ON file_manifest(patient_id);

        CREATE TABLE IF NOT EXISTS document_extractions (
            id TEXT PRIMARY KEY,
            file_id TEXT NOT NULL REFERENCES file_manifest(id) ON DELETE CASCADE,
            document_type TEXT,
            classification_confidence REAL,
            raw_text TEXT,
            extracted_json TEXT,
            extraction_model TEXT,
            extraction_prompt_version TEXT,
            extracted_at TEXT DEFAULT CURRENT_TIMESTAMP,
            user_reviewed INTEGER DEFAULT 0,
            user_corrections TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_extraction_file ON document_extractions(file_id);
        CREATE INDEX IF NOT EXISTS idx_extraction_review ON document_extractions(user_reviewed);
        ",
    )?;
    Ok(())
}

/// LLM endpoint registry: user-configurable providers (Ollama, OpenAI,
/// Anthropic, Gemini, etc.) and per-feature bindings.
fn migrate_008_llm_endpoints(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS llm_endpoints (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            provider_type TEXT NOT NULL,
            base_url TEXT NOT NULL,
            api_key_encrypted TEXT,
            default_model TEXT,
            extra_headers TEXT,
            enabled INTEGER DEFAULT 1,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS llm_feature_bindings (
            feature TEXT PRIMARY KEY,
            endpoint_id TEXT REFERENCES llm_endpoints(id),
            model_override TEXT
        );
        ",
    )?;
    Ok(())
}

/// Health Vault week 4: episode foreign keys on per-event tables, drive
/// sync state placeholder for week 5, and a precomputed correlation cache.
fn migrate_009_health_timeline(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        ALTER TABLE lab_tests ADD COLUMN episode_id TEXT REFERENCES episodes(id);
        ALTER TABLE medications_v2 ADD COLUMN episode_id TEXT REFERENCES episodes(id);
        ALTER TABLE health_conditions ADD COLUMN episode_id TEXT REFERENCES episodes(id);
        ALTER TABLE vitals ADD COLUMN episode_id TEXT REFERENCES episodes(id);
        ALTER TABLE life_events ADD COLUMN episode_id TEXT REFERENCES episodes(id);
        ALTER TABLE symptoms ADD COLUMN episode_id TEXT REFERENCES episodes(id);

        CREATE TABLE IF NOT EXISTS drive_sync_state (
            id INTEGER PRIMARY KEY,
            enabled INTEGER DEFAULT 0,
            account_id TEXT,
            file_id_remote TEXT,
            last_synced_at TEXT,
            last_remote_etag TEXT,
            sync_cursor TEXT,
            error TEXT
        );

        CREATE TABLE IF NOT EXISTS health_correlations (
            id TEXT PRIMARY KEY,
            patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            source_kind TEXT NOT NULL,
            source_id TEXT NOT NULL,
            target_kind TEXT NOT NULL,
            target_id TEXT NOT NULL,
            relation TEXT NOT NULL,
            confidence REAL,
            delta_days INTEGER,
            notes TEXT,
            computed_at TEXT DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(patient_id, source_kind, source_id, target_kind, target_id)
        );
        CREATE INDEX IF NOT EXISTS idx_corr_patient_src
            ON health_correlations(patient_id, source_kind, source_id);
        CREATE INDEX IF NOT EXISTS idx_corr_patient_tgt
            ON health_correlations(patient_id, target_kind, target_id);
        ",
    )?;
    Ok(())
}

/// Health Vault week 5: AI analysis cache. Stores prompt+response so the
/// UI can show prior analyses without re-spending tokens.
fn migrate_010_health_analysis(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS health_analyses (
            id TEXT PRIMARY KEY,
            patient_id TEXT NOT NULL REFERENCES patients(id) ON DELETE CASCADE,
            mode TEXT NOT NULL,
            question TEXT,
            timeline_from TEXT,
            timeline_to TEXT,
            brief_text TEXT,
            response_text TEXT NOT NULL,
            response_json TEXT,
            model_used TEXT,
            endpoint_id TEXT,
            cloud_used INTEGER DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_analyses_patient_date
            ON health_analyses(patient_id, created_at DESC);
        ",
    )?;
    Ok(())
}

/// Health Vault week-5 polish: recreate ingestion_jobs + file_manifest
/// with proper ON DELETE CASCADE/SET NULL semantics so deleting a patient
/// or job doesn't fail with FK violations now that PRAGMA foreign_keys
/// is on. Uses the SQLite "12-step" pattern (rename → recreate → copy →
/// drop) wrapped in defer_foreign_keys.
fn migrate_011_health_fk_repair(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        PRAGMA defer_foreign_keys = ON;

        ALTER TABLE ingestion_jobs RENAME TO ingestion_jobs_old;
        CREATE TABLE ingestion_jobs (
            id TEXT PRIMARY KEY,
            patient_id TEXT REFERENCES patients(id) ON DELETE CASCADE,
            source_folder TEXT,
            started_at TEXT DEFAULT CURRENT_TIMESTAMP,
            completed_at TEXT,
            status TEXT DEFAULT 'running',
            total_files INTEGER DEFAULT 0,
            processed_files INTEGER DEFAULT 0,
            skipped_files INTEGER DEFAULT 0,
            failed_files INTEGER DEFAULT 0,
            model_used TEXT,
            error TEXT
        );
        INSERT INTO ingestion_jobs SELECT * FROM ingestion_jobs_old;
        DROP TABLE ingestion_jobs_old;

        ALTER TABLE file_manifest RENAME TO file_manifest_old;
        CREATE TABLE file_manifest (
            id TEXT PRIMARY KEY,
            sha256 TEXT NOT NULL UNIQUE,
            original_path TEXT NOT NULL,
            stored_path TEXT,
            mime_type TEXT,
            size_bytes INTEGER,
            status TEXT DEFAULT 'pending',
            patient_id TEXT REFERENCES patients(id) ON DELETE CASCADE,
            job_id TEXT REFERENCES ingestion_jobs(id) ON DELETE SET NULL,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            error TEXT
        );
        INSERT INTO file_manifest SELECT * FROM file_manifest_old;
        DROP TABLE file_manifest_old;

        CREATE INDEX IF NOT EXISTS idx_file_status ON file_manifest(status);
        CREATE INDEX IF NOT EXISTS idx_file_patient ON file_manifest(patient_id);
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
        assert_eq!(count, 11);

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
    fn test_patient_delete_cascades_health_data() {
        // Verifies BLOCKER 1 fix: PRAGMA foreign_keys is ON and the
        // ON DELETE CASCADE rules from migration 006/011 actually fire.
        let conn = setup_test_db();
        // Enable manually since this isolated Connection skips the pool init.
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        run(&conn).expect("Failed to run migrations");

        conn.execute(
            "INSERT INTO patients (id, phone_number, full_name, relationship, is_primary)
             VALUES ('p1', '+19', 'Jane', 'self', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lab_tests (id, patient_id, test_name, value, collected_at)
             VALUES ('l1', 'p1', 'HbA1c', 6.5, '2025-01-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ingestion_jobs (id, patient_id) VALUES ('j1', 'p1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO file_manifest (id, sha256, original_path, patient_id, job_id)
             VALUES ('f1', 'abc', '/tmp/x.pdf', 'p1', 'j1')",
            [],
        )
        .unwrap();

        // Delete the patient; everything pointing at them should disappear.
        conn.execute("DELETE FROM patients WHERE id = 'p1'", []).unwrap();

        let labs: i64 = conn
            .query_row("SELECT COUNT(*) FROM lab_tests", [], |r| r.get(0))
            .unwrap();
        let jobs: i64 = conn
            .query_row("SELECT COUNT(*) FROM ingestion_jobs", [], |r| r.get(0))
            .unwrap();
        let files: i64 = conn
            .query_row("SELECT COUNT(*) FROM file_manifest", [], |r| r.get(0))
            .unwrap();
        assert_eq!(labs, 0, "lab_tests should cascade on patient delete");
        assert_eq!(jobs, 0, "ingestion_jobs should cascade on patient delete");
        assert_eq!(files, 0, "file_manifest should cascade on patient delete");
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
