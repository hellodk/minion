#!/bin/bash
# Version management script for MINION
# Usage: ./scripts/version.sh [major|minor|patch|get]

set -e

CARGO_TOML="Cargo.toml"
PACKAGE_JSON="ui/package.json"
TAURI_CONF="src-tauri/tauri.conf.json"

# Get current version from workspace Cargo.toml
get_version() {
    grep -m1 'version = "' "$CARGO_TOML" | sed 's/.*version = "\([^"]*\)".*/\1/'
}

# Parse version into components
parse_version() {
    local version=$1
    MAJOR=$(echo "$version" | cut -d. -f1)
    MINOR=$(echo "$version" | cut -d. -f2)
    PATCH=$(echo "$version" | cut -d. -f3)
}

# Bump version
bump_version() {
    local bump_type=$1
    local current=$(get_version)
    parse_version "$current"
    
    case $bump_type in
        major)
            MAJOR=$((MAJOR + 1))
            MINOR=0
            PATCH=0
            ;;
        minor)
            MINOR=$((MINOR + 1))
            PATCH=0
            ;;
        patch)
            PATCH=$((PATCH + 1))
            ;;
        *)
            echo "Unknown bump type: $bump_type"
            exit 1
            ;;
    esac
    
    echo "$MAJOR.$MINOR.$PATCH"
}

# Update version in all files
update_version() {
    local new_version=$1
    local old_version=$(get_version)
    
    echo "Updating version: $old_version -> $new_version"
    
    # Update workspace Cargo.toml
    sed -i "s/^version = \"$old_version\"/version = \"$new_version\"/" "$CARGO_TOML"
    
    # Update package.json
    if [ -f "$PACKAGE_JSON" ]; then
        sed -i "s/\"version\": \"$old_version\"/\"version\": \"$new_version\"/" "$PACKAGE_JSON"
    fi
    
    # Update tauri.conf.json
    if [ -f "$TAURI_CONF" ]; then
        sed -i "s/\"version\": \"$old_version\"/\"version\": \"$new_version\"/" "$TAURI_CONF"
    fi
    
    echo "Version updated to $new_version"
}

# Main
case "${1:-get}" in
    get)
        get_version
        ;;
    major|minor|patch)
        new_version=$(bump_version "$1")
        update_version "$new_version"
        ;;
    set)
        if [ -z "$2" ]; then
            echo "Usage: $0 set <version>"
            exit 1
        fi
        update_version "$2"
        ;;
    *)
        echo "Usage: $0 [get|major|minor|patch|set <version>]"
        echo ""
        echo "Commands:"
        echo "  get           Show current version"
        echo "  major         Bump major version (X.0.0)"
        echo "  minor         Bump minor version (x.X.0)"
        echo "  patch         Bump patch version (x.x.X)"
        echo "  set <ver>     Set specific version"
        exit 1
        ;;
esac
