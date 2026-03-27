# MINION

**Modular Intelligence Network for Integrated Operations Natively**

A unified personal intelligence platform that runs natively on Linux, Windows, macOS, Mini PCs, and high-performance workstations.

## Philosophy

- **No Docker Required** - Native execution, zero containerization dependency
- **Offline-First** - Full functionality without internet
- **Privacy-First** - Zero telemetry unless explicitly enabled
- **Local AI** - Ollama-compatible local LLM integration
- **Modular** - Enable only what you need

## Modules

| Module | Description | Status |
|--------|-------------|--------|
| Media Intelligence | Video processing, YouTube automation, thumbnail generation | Planned |
| File Intelligence | Duplicate detection, storage analytics, media comparison | Planned |
| Blog AI Engine | Multi-platform publishing, SEO optimization | Planned |
| Finance Intelligence | Portfolio tracking, expense analysis, tax planning | Planned |
| Fitness & Wellness | Workout planning, habit tracking, health metrics | Planned |
| Book Reader | Premium reading experience with AI-powered insights | Planned |

## Tech Stack

- **Backend**: Rust (Tokio async runtime)
- **Frontend**: Tauri + SolidJS
- **Database**: SQLite (libsql) + tantivy (search index)
- **AI Layer**: Rust bindings + optional Python micro-layer
- **Encryption**: AES-256-GCM via RustCrypto

## Requirements

- **RAM**: < 300MB baseline
- **Storage**: ~100MB application + user data
- **OS**: Linux (Ubuntu 20.04+), Windows 10+, macOS 11+

## Quick Start

```bash
# Clone and build
git clone https://github.com/yourusername/minion.git
cd minion
cargo build --release

# Run
./target/release/minion
```

## Project Structure

```
minion/
├── src/
│   ├── core/           # Core engine, plugin system, encryption
│   ├── modules/        # Feature modules
│   │   ├── media/      # Media Intelligence
│   │   ├── files/      # File Intelligence
│   │   ├── blog/       # Blog AI Engine
│   │   ├── finance/    # Finance Intelligence
│   │   ├── fitness/    # Fitness & Wellness
│   │   └── reader/     # Book Reader
│   └── ui/             # Tauri frontend
├── plugins/            # External plugins
├── docs/               # Documentation
└── tests/              # Integration tests
```

## Security

- AES-256-GCM encryption for sensitive data
- Credential vault with hardware key derivation
- OAuth token isolation per module
- No background telemetry
- Audit logging (local only)

## License

MIT License - See [LICENSE](LICENSE)

## Contributing

See [CONTRIBUTING.md](docs/CONTRIBUTING.md)
