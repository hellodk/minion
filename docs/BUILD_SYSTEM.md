# MINION Build System

## Overview

MINION uses a multi-stage build system combining Cargo (Rust) and pnpm (frontend).

---

## Build Requirements

### Development

```
Rust: 1.75+ (stable)
Node.js: 20 LTS+
pnpm: 8+
```

### Platform-Specific

**Linux (Ubuntu/Debian)**:
```bash
# Build essentials
sudo apt install build-essential pkg-config

# WebKit for Tauri
sudo apt install libwebkit2gtk-4.1-dev

# SSL
sudo apt install libssl-dev

# Additional libraries
sudo apt install libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

**macOS**:
```bash
xcode-select --install
```

**Windows**:
- Visual Studio Build Tools 2022
- WebView2 (usually pre-installed on Windows 10+)

---

## Build Commands

### Development Build

```bash
# Full build
cargo build

# Run in development mode
cargo tauri dev
```

### Release Build

```bash
# Optimized release build
cargo build --release

# Build Tauri application
cargo tauri build
```

### Individual Crates

```bash
# Build specific crate
cargo build -p minion-core

# Test specific crate
cargo test -p minion-core

# Run clippy on specific crate
cargo clippy -p minion-core
```

---

## Frontend Build

### Setup

```bash
cd ui
pnpm install
```

### Development

```bash
pnpm dev
```

### Production

```bash
pnpm build
```

---

## CI/CD Pipeline

### GitHub Actions

```yaml
# .github/workflows/build.yml
name: Build

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  build:
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      
      - name: Install Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
      
      - name: Install pnpm
        uses: pnpm/action-setup@v2
        with:
          version: 8
      
      - name: Install Linux dependencies
        if: runner.os == 'Linux'
        run: |
          sudo apt update
          sudo apt install -y libwebkit2gtk-4.1-dev libssl-dev libgtk-3-dev
      
      - name: Install frontend dependencies
        working-directory: ui
        run: pnpm install
      
      - name: Build frontend
        working-directory: ui
        run: pnpm build
      
      - name: Build Rust
        run: cargo build --release
      
      - name: Run tests
        run: cargo test --release
      
      - name: Build Tauri app
        run: cargo tauri build
```

### Release Workflow

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: windows-latest
            target: x86_64-pc-windows-msvc
          - os: macos-latest
            target: x86_64-apple-darwin
          - os: macos-latest
            target: aarch64-apple-darwin
    
    runs-on: ${{ matrix.os }}
    
    steps:
      # ... build steps ...
      
      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: minion-${{ matrix.target }}
          path: target/release/bundle/
```

---

## Cross-Compilation

### Linux ARM64

```bash
# Install target
rustup target add aarch64-unknown-linux-gnu

# Install linker
sudo apt install gcc-aarch64-linux-gnu

# Build
cargo build --release --target aarch64-unknown-linux-gnu
```

### macOS Universal Binary

```bash
# Install targets
rustup target add x86_64-apple-darwin aarch64-apple-darwin

# Build both architectures
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# Create universal binary
lipo -create \
  target/x86_64-apple-darwin/release/minion \
  target/aarch64-apple-darwin/release/minion \
  -output target/release/minion-universal
```

---

## Dependency Management

### Cargo Dependencies

```bash
# Update all dependencies
cargo update

# Check for outdated dependencies
cargo outdated

# Audit for security vulnerabilities
cargo audit
```

### Frontend Dependencies

```bash
cd ui

# Update dependencies
pnpm update

# Check for vulnerabilities
pnpm audit
```

---

## Build Optimization

### Release Profile

```toml
# Cargo.toml
[profile.release]
lto = true           # Link-time optimization
codegen-units = 1    # Better optimization
panic = "abort"      # Smaller binary
strip = true         # Remove symbols
opt-level = 3        # Maximum optimization
```

### Build Caching

```bash
# Use sccache for faster rebuilds
cargo install sccache
export RUSTC_WRAPPER=sccache
```

---

## Testing

### Unit Tests

```bash
cargo test
```

### Integration Tests

```bash
cargo test --test '*'
```

### Benchmarks

```bash
cargo bench
```

### Coverage

```bash
# Install coverage tool
cargo install cargo-llvm-cov

# Run with coverage
cargo llvm-cov --html
```

---

## Documentation

### Generate Docs

```bash
cargo doc --no-deps --open
```

### Documentation with Private Items

```bash
cargo doc --no-deps --document-private-items
```
