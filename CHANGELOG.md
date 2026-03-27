# Changelog

All notable changes to MINION will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project structure with 13 Rust crates
- Core engine with event bus, plugin system, and task scheduler
- Cryptographic utilities (AES-256-GCM, Argon2id, credential vault)
- Database layer with SQLite and migrations
- File Intelligence module with scanning, hashing, duplicate detection
- Book Reader module with EPUB parsing, annotations, knowledge base
- Tauri desktop application shell
- SolidJS UI with Dashboard, Files, Reader, Finance, Fitness, Settings pages
- Dark/Light theme support
- Comprehensive documentation (Architecture, API, Security, Roadmap)

### Security
- AES-256-GCM encryption for sensitive data
- Argon2id key derivation with secure memory handling
- Zero telemetry by default

## [0.1.0] - 2026-02-15

### Added
- Initial release
- Project scaffolding and architecture
- Core module stubs for all 6 planned modules
- Basic UI framework

[Unreleased]: https://github.com/minion-app/minion/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/minion-app/minion/releases/tag/v0.1.0
