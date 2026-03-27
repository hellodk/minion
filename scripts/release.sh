#!/bin/bash
# Release script for MINION
# Usage: ./scripts/release.sh [major|minor|patch]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$ROOT_DIR"

# Check for uncommitted changes
if ! git diff-index --quiet HEAD --; then
    echo "Error: You have uncommitted changes. Please commit or stash them first."
    exit 1
fi

# Ensure we're on main branch
BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" != "main" ] && [ "$BRANCH" != "master" ]; then
    echo "Warning: You're not on main/master branch (current: $BRANCH)"
    read -p "Continue anyway? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Get bump type
BUMP_TYPE=${1:-patch}
if [[ ! "$BUMP_TYPE" =~ ^(major|minor|patch)$ ]]; then
    echo "Usage: $0 [major|minor|patch]"
    exit 1
fi

# Get current and new version
OLD_VERSION=$("$SCRIPT_DIR/version.sh" get)
NEW_VERSION=$("$SCRIPT_DIR/version.sh" "$BUMP_TYPE" 2>/dev/null | tail -1 | sed 's/Version updated to //')

# Actually bump the version
"$SCRIPT_DIR/version.sh" "$BUMP_TYPE"
NEW_VERSION=$("$SCRIPT_DIR/version.sh" get)

echo ""
echo "=== Release v$NEW_VERSION ==="
echo ""

# Run tests
echo "Running tests..."
cargo test --workspace

# Run clippy
echo "Running clippy..."
cargo clippy --workspace -- -D warnings

# Format check
echo "Checking formatting..."
cargo fmt --all -- --check

# Build release
echo "Building release..."
cargo build --release

# Build UI
echo "Building UI..."
cd ui && pnpm build && cd ..

# Update changelog
echo ""
echo "Don't forget to update CHANGELOG.md with release notes!"
echo ""

# Create git commit and tag
read -p "Create git commit and tag for v$NEW_VERSION? [y/N] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    git add -A
    git commit -m "chore: release v$NEW_VERSION"
    git tag -a "v$NEW_VERSION" -m "Release v$NEW_VERSION"
    
    echo ""
    echo "Created commit and tag for v$NEW_VERSION"
    echo "Run 'git push && git push --tags' to publish"
fi

echo ""
echo "Release v$NEW_VERSION complete!"
