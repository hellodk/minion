# Contributing to MINION

Thank you for your interest in contributing to MINION! This document provides guidelines and information for contributors.

## Code of Conduct

Be respectful and constructive. We're all here to build something great together.

## Development Setup

### Prerequisites

- Rust 1.75+ (install via [rustup](https://rustup.rs/))
- Node.js 20+ and pnpm
- System dependencies for Tauri:
  - **Linux**: `libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev`
  - **macOS**: Xcode Command Line Tools
  - **Windows**: Visual Studio Build Tools

### Getting Started

```bash
# Clone the repository
git clone https://github.com/minion-app/minion.git
cd minion

# Install UI dependencies
cd ui && pnpm install && cd ..

# Build and run in development mode
cargo tauri dev
```

## Coding Standards

### Rust

- Format code with `cargo fmt`
- Run `cargo clippy` and fix all warnings
- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Write doc comments for public APIs
- Add tests for new functionality

### TypeScript/JavaScript

- Use TypeScript for all new code
- Follow the ESLint configuration
- Use single quotes for strings
- Always use semicolons

### Git Commits

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Code style (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvement
- `test`: Adding tests
- `chore`: Maintenance tasks

Examples:
```
feat(reader): add EPUB table of contents extraction
fix(crypto): handle empty password in vault unlock
docs: update API documentation for file scanner
```

## Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/amazing-feature`)
3. Make your changes
4. Run tests (`cargo test --workspace`)
5. Run lints (`cargo clippy && cargo fmt --check`)
6. Commit your changes using conventional commits
7. Push to your fork
8. Open a Pull Request

### PR Checklist

- [ ] Code follows project style guidelines
- [ ] Tests added/updated for changes
- [ ] Documentation updated if needed
- [ ] Changelog updated for notable changes
- [ ] All CI checks pass

## Versioning

We use [Semantic Versioning](https://semver.org/):

- **MAJOR**: Breaking changes
- **MINOR**: New features (backward compatible)
- **PATCH**: Bug fixes (backward compatible)

Use `./scripts/version.sh` to manage versions:
```bash
./scripts/version.sh get      # Show current version
./scripts/version.sh patch    # Bump patch version
./scripts/version.sh minor    # Bump minor version
./scripts/version.sh major    # Bump major version
```

## Architecture Decisions

Major architectural decisions should be discussed in issues before implementation. Reference the `docs/ARCHITECTURE.md` for current architecture.

## Security

If you discover a security vulnerability, please do NOT open a public issue. Instead, email security@minion.dev (or the maintainers directly).

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
