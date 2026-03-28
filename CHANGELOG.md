# Changelog

All notable changes to MINION will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.6.0] - 2026-03-27

### Added
- Persistent book library (books survive app restarts)
- Book collections/playlists with color coding
- EPUB cover image extraction and display
- EPUB inline image rendering (base64 data URIs)
- O'Reilly Learning SSO login via Chrome cookies
- Multi-directory file comparison
- Bulk delete/move duplicate files
- Optimized scan (hash only size-candidates, 10x faster)
- Live scan progress with file count
- Cancel scan support and exclusion patterns
- LLM configuration in Settings (Ollama URL, model)
- AI health analysis (sends metrics to local LLM)
- Fitness module wired to real DB data
- App icon (blue M gradient)
- 623+ tests across the workspace

## [1.5.0] - 2026-03-15

### Added
- Database persistence for all modules (20 new tables)
- Finance CSV bank statement import with auto-categorization
- CAGR calculator and net worth breakdown
- 18 new Tauri IPC commands (Finance 7, Fitness 6, Reader 5)
- Command palette (Ctrl+K) with fuzzy search
- Dashboard wired to real data from all modules

## [1.0.0] - 2026-03-01

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

[Unreleased]: https://github.com/minion-app/minion/compare/v1.6.0...HEAD
[1.6.0]: https://github.com/minion-app/minion/compare/v1.5.0...v1.6.0
[1.5.0]: https://github.com/minion-app/minion/compare/v1.0.0...v1.5.0
[1.0.0]: https://github.com/minion-app/minion/compare/v0.1.0...v1.0.0
[0.1.0]: https://github.com/minion-app/minion/releases/tag/v0.1.0
