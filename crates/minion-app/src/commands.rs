//! CLI command definitions and argument parsing.

use crate::{Error, Result};

/// Available CLI commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Scan a directory for files.
    Scan { path: String, recursive: bool },
    /// Search indexed content.
    Search { query: String, limit: usize },
    /// Get or set a configuration value.
    Config {
        key: Option<String>,
        value: Option<String>,
    },
    /// Show application information.
    Info,
    /// Show the application version.
    Version,
    /// Show current application status.
    Status,
}

impl Command {
    /// Parse a command from CLI argument strings.
    ///
    /// Expected formats:
    /// - `["scan", "<path>"]` or `["scan", "<path>", "--recursive"]`
    /// - `["search", "<query>"]` or `["search", "<query>", "--limit", "<n>"]`
    /// - `["config"]`, `["config", "<key>"]`, or `["config", "<key>", "<value>"]`
    /// - `["info"]`
    /// - `["version"]`
    /// - `["status"]`
    pub fn parse(args: &[String]) -> Result<Self> {
        let first = args
            .first()
            .map(|s| s.as_str())
            .ok_or_else(|| Error::App("No command provided".to_string()))?;

        match first {
            "scan" => Self::parse_scan(&args[1..]),
            "search" => Self::parse_search(&args[1..]),
            "config" => Self::parse_config(&args[1..]),
            "info" => Ok(Command::Info),
            "version" => Ok(Command::Version),
            "status" => Ok(Command::Status),
            other => Err(Error::App(format!("Unknown command: {other}"))),
        }
    }

    /// Return the name of this command.
    pub fn name(&self) -> &str {
        match self {
            Command::Scan { .. } => "scan",
            Command::Search { .. } => "search",
            Command::Config { .. } => "config",
            Command::Info => "info",
            Command::Version => "version",
            Command::Status => "status",
        }
    }

    /// Return a human-readable description of this command.
    pub fn description(&self) -> &str {
        match self {
            Command::Scan { .. } => "Scan a directory for files",
            Command::Search { .. } => "Search indexed content",
            Command::Config { .. } => "Get or set configuration values",
            Command::Info => "Show application information",
            Command::Version => "Show the application version",
            Command::Status => "Show current application status",
        }
    }

    fn parse_scan(args: &[String]) -> Result<Self> {
        if args.is_empty() {
            return Err(Error::App(
                "scan command requires a path argument".to_string(),
            ));
        }

        let path = args[0].clone();
        let recursive = args[1..].iter().any(|a| a == "--recursive" || a == "-r");

        Ok(Command::Scan { path, recursive })
    }

    fn parse_search(args: &[String]) -> Result<Self> {
        if args.is_empty() {
            return Err(Error::App(
                "search command requires a query argument".to_string(),
            ));
        }

        let query = args[0].clone();
        let mut limit: usize = 10;

        let mut i = 1;
        while i < args.len() {
            if (args[i] == "--limit" || args[i] == "-l") && i + 1 < args.len() {
                limit = args[i + 1]
                    .parse::<usize>()
                    .map_err(|e| Error::App(format!("Invalid limit value: {e}")))?;
                i += 2;
            } else {
                i += 1;
            }
        }

        Ok(Command::Search { query, limit })
    }

    fn parse_config(args: &[String]) -> Result<Self> {
        let key = args.first().cloned();
        let value = args.get(1).cloned();
        Ok(Command::Config { key, value })
    }
}

/// Return a list of all available commands with their descriptions.
pub fn available_commands() -> Vec<(&'static str, &'static str)> {
    vec![
        ("scan", "Scan a directory for files"),
        ("search", "Search indexed content"),
        ("config", "Get or set configuration values"),
        ("info", "Show application information"),
        ("version", "Show the application version"),
        ("status", "Show current application status"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    // ---- parse tests ----

    #[test]
    fn test_parse_no_args() {
        let result = Command::parse(&[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No command"));
    }

    #[test]
    fn test_parse_unknown_command() {
        let result = Command::parse(&args(&["foobar"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown command"));
    }

    // ---- scan ----

    #[test]
    fn test_parse_scan_basic() {
        let cmd = Command::parse(&args(&["scan", "/home/user"])).unwrap();
        assert_eq!(
            cmd,
            Command::Scan {
                path: "/home/user".to_string(),
                recursive: false,
            }
        );
    }

    #[test]
    fn test_parse_scan_recursive_long() {
        let cmd = Command::parse(&args(&["scan", "/tmp", "--recursive"])).unwrap();
        assert_eq!(
            cmd,
            Command::Scan {
                path: "/tmp".to_string(),
                recursive: true,
            }
        );
    }

    #[test]
    fn test_parse_scan_recursive_short() {
        let cmd = Command::parse(&args(&["scan", "/tmp", "-r"])).unwrap();
        assert_eq!(
            cmd,
            Command::Scan {
                path: "/tmp".to_string(),
                recursive: true,
            }
        );
    }

    #[test]
    fn test_parse_scan_missing_path() {
        let result = Command::parse(&args(&["scan"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("path"));
    }

    // ---- search ----

    #[test]
    fn test_parse_search_basic() {
        let cmd = Command::parse(&args(&["search", "hello"])).unwrap();
        assert_eq!(
            cmd,
            Command::Search {
                query: "hello".to_string(),
                limit: 10,
            }
        );
    }

    #[test]
    fn test_parse_search_with_limit_long() {
        let cmd = Command::parse(&args(&["search", "rust", "--limit", "25"])).unwrap();
        assert_eq!(
            cmd,
            Command::Search {
                query: "rust".to_string(),
                limit: 25,
            }
        );
    }

    #[test]
    fn test_parse_search_with_limit_short() {
        let cmd = Command::parse(&args(&["search", "rust", "-l", "5"])).unwrap();
        assert_eq!(
            cmd,
            Command::Search {
                query: "rust".to_string(),
                limit: 5,
            }
        );
    }

    #[test]
    fn test_parse_search_invalid_limit() {
        let result = Command::parse(&args(&["search", "q", "--limit", "abc"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid limit"));
    }

    #[test]
    fn test_parse_search_missing_query() {
        let result = Command::parse(&args(&["search"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("query"));
    }

    // ---- config ----

    #[test]
    fn test_parse_config_no_args() {
        let cmd = Command::parse(&args(&["config"])).unwrap();
        assert_eq!(
            cmd,
            Command::Config {
                key: None,
                value: None,
            }
        );
    }

    #[test]
    fn test_parse_config_key_only() {
        let cmd = Command::parse(&args(&["config", "theme"])).unwrap();
        assert_eq!(
            cmd,
            Command::Config {
                key: Some("theme".to_string()),
                value: None,
            }
        );
    }

    #[test]
    fn test_parse_config_key_and_value() {
        let cmd = Command::parse(&args(&["config", "theme", "dark"])).unwrap();
        assert_eq!(
            cmd,
            Command::Config {
                key: Some("theme".to_string()),
                value: Some("dark".to_string()),
            }
        );
    }

    // ---- simple commands ----

    #[test]
    fn test_parse_info() {
        assert_eq!(Command::parse(&args(&["info"])).unwrap(), Command::Info);
    }

    #[test]
    fn test_parse_version() {
        assert_eq!(
            Command::parse(&args(&["version"])).unwrap(),
            Command::Version
        );
    }

    #[test]
    fn test_parse_status() {
        assert_eq!(Command::parse(&args(&["status"])).unwrap(), Command::Status);
    }

    // ---- name / description ----

    #[test]
    fn test_command_names() {
        assert_eq!(
            Command::Scan {
                path: String::new(),
                recursive: false,
            }
            .name(),
            "scan"
        );
        assert_eq!(
            Command::Search {
                query: String::new(),
                limit: 0,
            }
            .name(),
            "search"
        );
        assert_eq!(
            Command::Config {
                key: None,
                value: None,
            }
            .name(),
            "config"
        );
        assert_eq!(Command::Info.name(), "info");
        assert_eq!(Command::Version.name(), "version");
        assert_eq!(Command::Status.name(), "status");
    }

    #[test]
    fn test_command_descriptions_non_empty() {
        let variants: Vec<Command> = vec![
            Command::Scan {
                path: String::new(),
                recursive: false,
            },
            Command::Search {
                query: String::new(),
                limit: 0,
            },
            Command::Config {
                key: None,
                value: None,
            },
            Command::Info,
            Command::Version,
            Command::Status,
        ];
        for cmd in &variants {
            assert!(
                !cmd.description().is_empty(),
                "{} should have a description",
                cmd.name()
            );
        }
    }

    // ---- available_commands ----

    #[test]
    fn test_available_commands_count() {
        let cmds = available_commands();
        assert_eq!(cmds.len(), 6);
    }

    #[test]
    fn test_available_commands_contains_all() {
        let cmds = available_commands();
        let names: Vec<&str> = cmds.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"scan"));
        assert!(names.contains(&"search"));
        assert!(names.contains(&"config"));
        assert!(names.contains(&"info"));
        assert!(names.contains(&"version"));
        assert!(names.contains(&"status"));
    }

    #[test]
    fn test_available_commands_descriptions_non_empty() {
        for (name, desc) in available_commands() {
            assert!(!desc.is_empty(), "{name} should have a description");
        }
    }

    // ---- clone / debug ----

    #[test]
    fn test_command_clone() {
        let cmd = Command::Search {
            query: "test".to_string(),
            limit: 5,
        };
        let cloned = cmd.clone();
        assert_eq!(cmd, cloned);
    }

    #[test]
    fn test_command_debug() {
        let cmd = Command::Info;
        let debug = format!("{:?}", cmd);
        assert!(debug.contains("Info"));
    }
}
