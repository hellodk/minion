# MINION

**Modular Intelligence Network for Integrated Operations Natively**

A unified personal intelligence platform that runs natively on Linux, Windows, macOS, Mini PCs, and high-performance workstations. Built with Rust and Tauri 2 for native performance and a small footprint.

## Philosophy

- **No Docker Required** - Native execution, zero containerization dependency
- **Offline-First** - Full functionality without internet
- **Privacy-First** - Zero telemetry unless explicitly enabled
- **Local AI** - Ollama-compatible local LLM integration
- **Modular** - Enable only what you need

## Features

### File Intelligence
- Multi-directory scanning with duplicate detection (SHA-256 hashing)
- Multi-directory file comparison
- Bulk delete/move duplicate files
- Optimized scan: hashes only size-candidates (10x faster than naive)
- Live scan progress with file count and cancel support
- Exclusion patterns for skipping directories
- Storage analytics and breakdown

### Book Reader
- EPUB parsing with full chapter navigation (arrow keys)
- EPUB cover image extraction and display
- EPUB inline image rendering (base64 data URIs)
- Persistent book library (books survive app restarts)
- Book collections/playlists with color coding
- Annotations and knowledge base
- O'Reilly Learning SSO login via Chrome cookies

### Finance Intelligence
- CSV bank statement import with auto-categorization
- Portfolio tracking and expense analysis
- CAGR calculator and net worth breakdown
- Account management and transaction history

### Fitness & Wellness
- Workout tracking and habit logging
- Nutrition tracking with calorie/macro breakdown
- AI health analysis (sends metrics to local Ollama LLM)
- Historical charts and trend visualization

### Media Intelligence
- Video processing and metadata extraction
- YouTube automation and thumbnail generation

### Blog AI Engine
- Multi-platform publishing
- SEO optimization

### General
- Command palette (Ctrl+K) with fuzzy search
- Dashboard with real data from all modules
- Dark/Light theme support
- App icon (blue M gradient)
- 623+ tests across the workspace

## Screenshots

<!-- TODO: Add screenshots -->

## Tech Stack

- **Backend**: Rust (Tokio async runtime), 14 workspace crates
- **Frontend**: Tauri 2 + SolidJS + TypeScript + Tailwind CSS
- **Database**: SQLite (rusqlite) with r2d2 connection pooling, 20+ tables
- **Search**: tantivy full-text search index
- **AI**: Ollama LLM integration, embeddings, RAG pipeline
- **Encryption**: AES-256-GCM via RustCrypto, Argon2id key derivation

## Requirements

- **RAM**: < 300MB baseline
- **Storage**: ~100MB application + user data
- **OS**: Linux (Ubuntu 20.04+), Windows 10+, macOS 11+
- **Optional**: [Ollama](https://ollama.ai/) for local AI features

## Installation

### System Dependencies (Linux)

```bash
sudo apt-get install -y libgtk-3-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

### Build from Source

```bash
# Clone
git clone https://github.com/yourusername/minion.git
cd minion

# Install frontend dependencies
cd ui && pnpm install && cd ..

# Run in development mode (builds UI + Rust, launches app)
cargo tauri dev

# Build for production (.deb and .AppImage on Linux)
cargo tauri build
```

Production artifacts are placed in `src-tauri/target/release/bundle/`.

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+K` | Open command palette (fuzzy search across all actions) |
| `Left` / `Right` | Previous / next chapter in Reader |
| `Ctrl+,` | Open Settings |

## Architecture

```
minion/
├── crates/
│   ├── minion-core/       # Plugin trait, event bus, config, error types, task system
│   ├── minion-db/         # SQLite with connection pooling (r2d2) and migrations
│   ├── minion-crypto/     # AES-256-GCM encryption, Argon2 key derivation, vault
│   ├── minion-ai/         # Ollama LLM integration, embeddings, RAG pipeline
│   ├── minion-search/     # Full-text search via tantivy
│   ├── minion-plugins/    # Plugin loader/registry
│   ├── minion-files/      # File Intelligence module
│   ├── minion-media/      # Media Intelligence module
│   ├── minion-blog/       # Blog AI Engine module
│   ├── minion-finance/    # Finance Intelligence module
│   ├── minion-fitness/    # Fitness & Wellness module
│   ├── minion-reader/     # Book Reader module
│   ├── minion-app/        # CLI entry point
│   └── minion-integration-tests/
├── src-tauri/             # Tauri 2 backend (commands, state)
├── ui/                    # SolidJS + TypeScript frontend
│   └── src/
│       ├── components/    # Shared layout, command palette
│       └── pages/         # Dashboard, Files, Reader, Finance, Fitness, Settings
├── docs/                  # Architecture, API, security, roadmap docs
└── scripts/               # Release and version management
```

The Tauri layer (`src-tauri/commands.rs`) exposes `#[tauri::command]` IPC functions that the SolidJS frontend invokes. Shared app state lives in `src-tauri/state.rs`.

## Configuration

### Data Directory

MINION stores its SQLite database and configuration in a platform-specific data directory managed by Tauri (typically `~/.local/share/minion/` on Linux).

### LLM Setup

1. Install [Ollama](https://ollama.ai/) and pull a model (e.g., `ollama pull llama3.2`)
2. Open Settings in the app
3. Set the Ollama URL (default: `http://localhost:11434`) and select your model
4. AI features (health analysis, RAG, embeddings) will use the configured model

## Security

- AES-256-GCM encryption for sensitive data
- Credential vault with Argon2id key derivation
- Secure memory handling for cryptographic operations
- No background telemetry
- All data stored locally

## License

MIT License - See [LICENSE](LICENSE)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md)
