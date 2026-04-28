#!/bin/sh
# scripts/version.sh — Manage workspace version
#
# Usage:
#   ./scripts/version.sh           # print current version
#   ./scripts/version.sh bump      # auto-increment patch (0.1.0 → 0.1.1)
#   ./scripts/version.sh set 0.2.0 # set explicit version
#   ./scripts/version.sh tag       # create + push git tag for current version

set -e

ROOT_CARGO="Cargo.toml"

current_version() {
    grep '^version' "$ROOT_CARGO" | head -1 | sed 's/.*"\(.*\)"/\1/'
}

bump_patch() {
    local ver="$1"
    local major minor patch
    major=$(echo "$ver" | cut -d. -f1)
    minor=$(echo "$ver" | cut -d. -f2)
    patch=$(echo "$ver" | cut -d. -f3)
    patch=$((patch + 1))
    echo "$major.$minor.$patch"
}

set_version() {
    local new_ver="$1"
    local old_ver
    old_ver=$(current_version)

    # Update workspace version
    sed -i.bak "s/^version = \"$old_ver\"/version = \"$new_ver\"/" "$ROOT_CARGO"
    rm -f "$ROOT_CARGO.bak"

    # Update bir-print which has its own version (not workspace)
    local print_toml="crates/bir-print/Cargo.toml"
    if [ -f "$print_toml" ]; then
        sed -i.bak "s/^version = \"$old_ver\"/version = \"$new_ver\"/" "$print_toml"
        rm -f "$print_toml.bak"
    fi

    echo "Version: $old_ver → $new_ver"
}

create_tag() {
    local ver
    ver=$(current_version)
    git add -A
    git commit -m "release: v$ver" --allow-empty
    git tag -a "v$ver" -m "Release v$ver"
    echo "Created tag v$ver"
}

push_tag() {
    local ver
    ver=$(current_version)
    git push origin main
    git push origin "v$ver"
    echo "Pushed tag v$ver — GitHub Actions release workflow will trigger"
}

# ── CLI ──────────────────────────────────────────────────────────────────────

case "${1:-}" in
    "")
        echo "$(current_version)"
        ;;
    bump)
        old=$(current_version)
        new=$(bump_patch "$old")
        set_version "$new"
        ;;
    set)
        if [ -z "${2:-}" ]; then
            echo "Usage: $0 set <version>" >&2
            exit 1
        fi
        set_version "$2"
        ;;
    tag)
        create_tag
        push_tag
        ;;
    *)
        echo "Usage: $0 [bump|set <version>|tag]" >&2
        exit 1
        ;;
esac
