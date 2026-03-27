//! Tests for Tauri commands and state management

#[cfg(test)]
mod tests {
    use crate::commands::*;
    use crate::state::AppState;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::RwLock;

    /// Helper to create a test app state
    #[allow(dead_code)]
    fn create_test_state() -> Result<Arc<RwLock<AppState>>, Box<dyn std::error::Error>> {
        let dir = tempdir()?;

        let mut config = minion_core::Config::default();
        config.data_dir = dir.path().join("data");
        config.config_dir = dir.path().join("config");
        config.cache_dir = dir.path().join("cache");

        std::fs::create_dir_all(&config.data_dir)?;
        std::fs::create_dir_all(&config.config_dir)?;
        std::fs::create_dir_all(&config.cache_dir)?;

        let db_path = config.data_dir.join(&config.database.path);
        let db = minion_db::Database::new(&db_path, config.database.pool_size)?;
        db.migrate()?;

        let event_bus = Arc::new(minion_core::EventBus::new());
        let task_scheduler = minion_core::TaskScheduler::new(config.workers.background_workers);
        let data_dir = config.data_dir.clone();

        Ok(Arc::new(RwLock::new(AppState {
            config,
            db,
            event_bus,
            task_scheduler,
            data_dir,
            watched_dirs: HashMap::new(),
            scan_tasks: HashMap::new(),
            scan_cache: None,
        })))
    }

    // ========== Response Types Tests ==========

    #[test]
    fn test_system_info_serialization() {
        let info = SystemInfo {
            version: "0.1.0".to_string(),
            platform: "linux".to_string(),
            arch: "x86_64".to_string(),
            data_dir: "/home/user/.minion/data".to_string(),
        };

        let json = serde_json::to_string(&info).expect("Serialization failed");
        assert!(json.contains("\"version\":\"0.1.0\""));
        assert!(json.contains("\"platform\":\"linux\""));
    }

    #[test]
    fn test_module_info_serialization() {
        let info = ModuleInfo {
            id: "files".to_string(),
            name: "File Intelligence".to_string(),
            enabled: true,
            status: "active".to_string(),
        };

        let json = serde_json::to_string(&info).expect("Serialization failed");
        assert!(json.contains("\"id\":\"files\""));
        assert!(json.contains("\"enabled\":true"));
    }

    #[test]
    fn test_scan_progress_serialization() {
        let progress = ScanProgress {
            task_id: "task-123".to_string(),
            status: "running".to_string(),
            files_scanned: 100,
            total_files: Some(500),
            progress_percent: 20.0,
        };

        let json = serde_json::to_string(&progress).expect("Serialization failed");
        assert!(json.contains("\"task_id\":\"task-123\""));
        assert!(json.contains("\"files_scanned\":100"));
        assert!(json.contains("\"progress_percent\":20.0"));
    }

    #[test]
    fn test_duplicate_group_serialization() {
        let group = DuplicateGroupResponse {
            id: "group-1".to_string(),
            match_type: "exact".to_string(),
            match_label: "Identical content (SHA-256 hash match)".to_string(),
            file_count: 3,
            total_size: 1024,
            wasted_space: 768,
            files: vec![],
            hash: Some("abc123".to_string()),
        };

        let json = serde_json::to_string(&group).expect("Serialization failed");
        assert!(json.contains("\"match_type\":\"exact\""));
        assert!(json.contains("\"file_count\":3"));
    }

    #[test]
    fn test_file_info_serialization() {
        let info = FileInfoResponse {
            path: "/home/user/file.txt".to_string(),
            name: "file.txt".to_string(),
            size: 1024,
            modified: "2024-01-01T00:00:00Z".to_string(),
            extension: Some("txt".to_string()),
        };

        let json = serde_json::to_string(&info).expect("Serialization failed");
        assert!(json.contains("\"name\":\"file.txt\""));
        assert!(json.contains("\"size\":1024"));
    }

    #[test]
    fn test_storage_analytics_serialization() {
        let analytics = StorageAnalytics {
            total_files: 1000,
            total_size: 1073741824,
            by_extension: vec![ExtensionStats {
                extension: "txt".to_string(),
                count: 100,
                size: 51200,
            }],
            duplicates_found: 50,
            duplicate_size: 10240,
        };

        let json = serde_json::to_string(&analytics).expect("Serialization failed");
        assert!(json.contains("\"total_files\":1000"));
        assert!(json.contains("\"extension\":\"txt\""));
    }

    #[test]
    fn test_extension_stats_serialization() {
        let stats = ExtensionStats {
            extension: "pdf".to_string(),
            count: 25,
            size: 52428800,
        };

        let json = serde_json::to_string(&stats).expect("Serialization failed");
        assert!(json.contains("\"extension\":\"pdf\""));
        assert!(json.contains("\"count\":25"));
    }

    // ========== Request Types Tests ==========

    #[test]
    fn test_add_directory_request_deserialization() {
        let json = r#"{
            "path": "/home/user/documents",
            "recursive": true
        }"#;

        let request: AddDirectoryRequest =
            serde_json::from_str(json).expect("Deserialization failed");

        assert_eq!(request.path, "/home/user/documents");
        assert_eq!(request.recursive, Some(true));
    }

    #[test]
    fn test_add_directory_request_minimal() {
        let json = r#"{
            "path": "/home/user/documents"
        }"#;

        let request: AddDirectoryRequest =
            serde_json::from_str(json).expect("Deserialization failed");

        assert_eq!(request.path, "/home/user/documents");
        assert!(request.recursive.is_none());
    }

    #[test]
    fn test_duplicate_filter_deserialization() {
        let json = r#"{
            "match_type": "exact",
            "min_size": 1024
        }"#;

        let filter: DuplicateFilter = serde_json::from_str(json).expect("Deserialization failed");

        assert_eq!(filter.match_type, Some("exact".to_string()));
        assert_eq!(filter.min_size, Some(1024));
    }

    #[test]
    fn test_duplicate_filter_empty() {
        let json = "{}";

        let filter: DuplicateFilter = serde_json::from_str(json).expect("Deserialization failed");

        assert!(filter.match_type.is_none());
        assert!(filter.min_size.is_none());
    }
}
