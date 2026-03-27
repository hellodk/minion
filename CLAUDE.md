# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
# Rust backend
cargo build --workspace          # Build all crates
cargo test --workspace           # Run all tests
cargo test -p minion-core        # Test a single crate
cargo test -p minion-core -- test_name  # Run a single test
cargo clippy --workspace -- -D warnings # Lint (CI treats warnings as errors)
cargo fmt --all -- --check       # Check formatting
cargo fmt --all                  # Auto-format

# UI (from ui/ directory)
cd ui && pnpm install            # Install frontend deps
cd ui && pnpm dev                # Dev server (localhost:5173)
cd ui && pnpm build              # Production build
cd ui && pnpm lint               # ESLint
cd ui && pnpm typecheck          # TypeScript type check

# Full Tauri dev (builds UI + Rust, launches app)
cargo tauri dev

# Version management
./scripts/version.sh get|patch|minor|major
```

## System Dependencies (Linux)

```bash
sudo apt-get install -y libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
```

## Architecture

**Tauri 2 desktop app**: Rust backend (`src-tauri/`) + SolidJS frontend (`ui/`) communicating via Tauri commands.

**Rust workspace** with domain-specific crates under `crates/`:
- **minion-core** — Plugin trait, event bus, config, error types, task system. All other crates depend on this.
- **minion-db** — SQLite (rusqlite/libsql) with connection pooling (r2d2) and migrations.
- **minion-crypto** — AES-256-GCM encryption, Argon2 key derivation, credential vault.
- **minion-ai** — Ollama LLM integration, embeddings, RAG pipeline.
- **minion-search** — Full-text search via tantivy.
- **minion-plugins** — Plugin loader/registry built on minion-core's Plugin trait.
- **minion-files**, **minion-media**, **minion-blog**, **minion-finance**, **minion-fitness**, **minion-reader** — Feature modules implementing domain logic.
- **minion-app** — CLI application entry point, wires commands to crates.

**Tauri layer** (`src-tauri/`): `commands.rs` exposes `#[tauri::command]` functions invoked from the frontend. `state.rs` holds shared app state managed by Tauri.

**Frontend** (`ui/`): SolidJS + TypeScript + Tailwind CSS. Pages in `ui/src/pages/`, shared layout in `ui/src/components/`.

## Conventions

- **Rust edition 2021**, MSRV 1.75
- **Formatting**: `max_width = 100`, Unix newlines (see `.rustfmt.toml`)
- **Clippy**: configured in `.clippy.toml` — cognitive complexity ≤ 25, function args ≤ 7, function lines ≤ 100
- **Commits**: Conventional Commits format — `feat(scope): description`, `fix(scope):`, etc.
- **Error handling**: `thiserror` for library errors, `anyhow` for application-level errors
- Some crypto dependency versions are pinned to avoid Rust edition2024 incompatibilities (see `Cargo.toml` comments)
