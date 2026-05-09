#!/usr/bin/env bash
# MINION quick build + deploy + clean.
#
# Usage:
#   ./scripts/build.sh                  # build, install, partial clean
#   ./scripts/build.sh --no-install     # build only (no dpkg)
#   ./scripts/build.sh --no-clean       # build + install, keep all artifacts
#   ./scripts/build.sh --deep-clean     # also remove target/debug/deps (~7 GB)
#   SKIP_DEPLOY=1 git push              # bypass post-push hook

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$ROOT_DIR"

# ── Parse args ────────────────────────────────────────────────────────────────

DO_INSTALL=true
DO_CLEAN=true
DEEP_CLEAN=false

for arg in "$@"; do
    case "$arg" in
        --no-install) DO_INSTALL=false ;;
        --no-clean)   DO_CLEAN=false ;;
        --deep-clean) DEEP_CLEAN=true ;;
        *) echo "Unknown option: $arg"; exit 1 ;;
    esac
done

# ── Build ─────────────────────────────────────────────────────────────────────

echo ""
echo "── Building MINION (release) ──"
T0=$(date +%s)

# Restore frontend deps from pnpm global store if wiped (typically <30 s)
if [[ ! -d ui/node_modules ]]; then
    echo "  restoring ui/node_modules from pnpm store…"
    pnpm install --prefix ui --frozen-lockfile
fi

cargo tauri build

T1=$(date +%s)
echo "  build: $((T1 - T0))s"

# ── Install ───────────────────────────────────────────────────────────────────

if $DO_INSTALL; then
    DEB=$(find target/release/bundle/deb -name "*.deb" 2>/dev/null | sort -V | tail -1)
    if [[ -z "$DEB" ]]; then
        echo "✗ No .deb found — did cargo tauri build finish successfully?"
        exit 1
    fi
    echo "── Installing $(basename "$DEB") ──"
    sudo dpkg -i "$DEB"
fi

# ── Clean ─────────────────────────────────────────────────────────────────────
# Strategy:
#   Always remove:  target/debug/{incremental,build}   — regenerated cheaply
#   --deep-clean:   also target/debug/deps              — ~7 GB; next build slower
#   Never remove:   ~/.cargo/registry, ui/node_modules  — expensive to restore

if $DO_CLEAN; then
    echo "── Cleaning debug artifacts ──"
    BEFORE=$(df / | awk 'NR==2{print $3}')

    rm -rf target/debug/incremental target/debug/build
    if $DEEP_CLEAN; then
        rm -rf target/debug/deps
        echo "  (deep clean — target/debug/deps removed)"
    fi

    AFTER=$(df / | awk 'NR==2{print $3}')
    FREED=$(( (BEFORE - AFTER) / 1024 ))
    echo "  freed ~${FREED} MB"
    df -h / | awk 'NR==2{print "  disk: " $3 " used / " $2 " total (" $5 " full)"}'
fi

echo ""
echo "── Done (total: $(($(date +%s) - T0))s) ──"
