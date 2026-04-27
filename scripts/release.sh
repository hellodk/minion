#!/usr/bin/env bash
# MINION release script — bump version, test, build .deb/.AppImage, install, clean.
#
# Usage:
#   ./scripts/release.sh patch          # bump patch (x.x.X), build, install, clean
#   ./scripts/release.sh minor          # bump minor (x.X.0)
#   ./scripts/release.sh major          # bump major (X.0.0)
#   ./scripts/release.sh patch --no-install   # skip dpkg install
#   ./scripts/release.sh patch --no-clean     # skip artifact cleanup
#   ./scripts/release.sh patch --skip-tests   # skip cargo test + clippy

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$ROOT_DIR"

# ── Parse args ────────────────────────────────────────────────────────────────

BUMP_TYPE="${1:-patch}"
DO_INSTALL=true
DO_CLEAN=true
SKIP_TESTS=false

for arg in "${@:2}"; do
    case "$arg" in
        --no-install)  DO_INSTALL=false ;;
        --no-clean)    DO_CLEAN=false ;;
        --skip-tests)  SKIP_TESTS=true ;;
        *) echo "Unknown option: $arg"; exit 1 ;;
    esac
done

if [[ ! "$BUMP_TYPE" =~ ^(major|minor|patch)$ ]]; then
    echo "Usage: $0 [major|minor|patch] [--no-install] [--no-clean] [--skip-tests]"
    exit 1
fi

# ── Pre-flight ────────────────────────────────────────────────────────────────

echo ""
echo "╔═══════════════════════════════════════╗"
echo "║        MINION Release Script          ║"
echo "╚═══════════════════════════════════════╝"

# Uncommitted changes check
if ! git diff-index --quiet HEAD --; then
    echo "✗ Uncommitted changes detected. Commit or stash first."
    exit 1
fi

# Branch check
BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [[ "$BRANCH" != "main" && "$BRANCH" != "master" ]]; then
    echo "⚠ Not on main/master (current: $BRANCH)"
    read -rp "  Continue anyway? [y/N] " reply
    [[ "$reply" =~ ^[Yy]$ ]] || exit 1
fi

OLD_VERSION=$("$SCRIPT_DIR/version.sh" get)

# Compute new version without modifying files yet
IFS='.' read -r ma mi pa <<< "$OLD_VERSION"
case "$BUMP_TYPE" in
    patch) NEW_VERSION="$ma.$mi.$((pa+1))" ;;
    minor) NEW_VERSION="$ma.$((mi+1)).0" ;;
    major) NEW_VERSION="$((ma+1)).0.0" ;;
esac

echo ""
echo "  Current : v$OLD_VERSION"
echo "  New     : v$NEW_VERSION  ($BUMP_TYPE bump)"
echo "  Install : $DO_INSTALL"
echo "  Clean   : $DO_CLEAN"
echo "  Tests   : $(! $SKIP_TESTS && echo yes || echo skipped)"
echo ""
read -rp "Proceed? [y/N] " reply
[[ "$reply" =~ ^[Yy]$ ]] || { echo "Aborted."; exit 0; }

# ── Tests & lint ──────────────────────────────────────────────────────────────

if ! $SKIP_TESTS; then
    echo ""
    echo "── Running tests ──"
    cargo test --workspace

    echo ""
    echo "── Clippy ──"
    cargo clippy --workspace -- -D warnings

    echo ""
    echo "── Format check ──"
    cargo fmt --all -- --check
fi

# ── Version bump ──────────────────────────────────────────────────────────────

echo ""
echo "── Bumping version to $NEW_VERSION ──"
"$SCRIPT_DIR/version.sh" "$BUMP_TYPE"
git add Cargo.toml ui/package.json src-tauri/tauri.conf.json
git commit -m "chore: bump version to $NEW_VERSION"

# ── Restore frontend deps if node_modules wiped ───────────────────────────────

if [[ ! -d ui/node_modules ]]; then
    echo ""
    echo "── Restoring frontend deps ──"
    pnpm install --prefix ui --frozen-lockfile
fi

# ── Tauri bundle (.deb + .AppImage + .rpm) ────────────────────────────────────

echo ""
echo "── Building Tauri release bundles ──"
cargo tauri build

DEB=$(find target/release/bundle/deb -name "*.deb" | sort -V | tail -1)
echo ""
echo "  Bundles:"
echo "    $(ls -lh target/release/bundle/deb/*.deb  2>/dev/null | awk '{print $NF, $5}')"
echo "    $(ls -lh target/release/bundle/rpm/*.rpm  2>/dev/null | awk '{print $NF, $5}' || true)"
echo "    $(ls -lh target/release/bundle/appimage/*.AppImage 2>/dev/null | awk '{print $NF, $5}' || true)"

# ── Git tag ───────────────────────────────────────────────────────────────────

git tag -a "v$NEW_VERSION" -m "Release v$NEW_VERSION"
echo ""
echo "  Tagged: v$NEW_VERSION"

# ── Install ───────────────────────────────────────────────────────────────────

if $DO_INSTALL; then
    echo ""
    echo "── Installing $DEB ──"
    sudo dpkg -i "$DEB"
fi

# ── Cleanup ───────────────────────────────────────────────────────────────────

if $DO_CLEAN; then
    echo ""
    echo "── Cleaning build artifacts ──"
    BEFORE=$(df / | awk 'NR==2{print $3}')
    rm -rf target/debug ui/node_modules ~/.cargo/registry/src
    AFTER=$(df / | awk 'NR==2{print $3}')
    FREED_MB=$(( (BEFORE - AFTER) / 1024 ))
    echo "  Freed ~${FREED_MB} MB"
    df -h / | awk 'NR==2{print "  Disk: " $3 " used / " $2 " total (" $5 " full)"}'
fi

# ── Done ─────────────────────────────────────────────────────────────────────

echo ""
echo "╔═══════════════════════════════════════╗"
printf  "║  ✓ MINION v%-27s║\n" "$NEW_VERSION released"
$DO_INSTALL && echo "║  ✓ Installed via dpkg                 ║"
$DO_CLEAN   && echo "║  ✓ Build artifacts cleaned            ║"
echo    "║                                       ║"
echo    "║  To publish:                          ║"
echo    "║    git push && git push --tags        ║"
echo    "╚═══════════════════════════════════════╝"
echo ""
