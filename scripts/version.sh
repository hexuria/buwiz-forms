#!/usr/bin/env bash
set -e

# Always run from the project root
cd "$(dirname "$0")/.."

CARGO_FILE="Cargo.toml"

get_version() {
    grep -E '^version = ".*"' "$CARGO_FILE" | head -n 1 | awk -F '"' '{print $2}'
}

set_version() {
    local new_version="$1"
    # Update Cargo.toml (works on both GNU and BSD sed)
    if sed --version >/dev/null 2>&1; then
        sed -i "s/^version = \".*\"/version = \"$new_version\"/" "$CARGO_FILE"
    else
        sed -i '' "s/^version = \".*\"/version = \"$new_version\"/" "$CARGO_FILE"
    fi
    # Also update Cargo.lock
    cargo check > /dev/null 2>&1
}

bump_version() {
    local current_version=$(get_version)
    local major=$(echo "$current_version" | cut -d. -f1)
    local minor=$(echo "$current_version" | cut -d. -f2)
    local patch=$(echo "$current_version" | cut -d. -f3)
    local new_version="${major}.${minor}.$((patch + 1))"
    set_version "$new_version"
}

tag_release() {
    local current_version=$(get_version)
    git add "$CARGO_FILE"
    # Only commit if there are changes
    git diff --cached --quiet || git commit -m "chore: bump version to v${current_version}"
    git tag -a "v${current_version}" -m "Release v${current_version}"
    git push origin HEAD
    git push origin "v${current_version}"
}

case "$1" in
    get)
        get_version
        ;;
    set)
        if [ -z "$2" ]; then
            echo "Error: Must provide a version number (e.g. 1.0.0)"
            exit 1
        fi
        set_version "$2"
        ;;
    bump)
        bump_version
        ;;
    tag)
        tag_release
        ;;
    *)
        # Default behavior: just print version
        get_version
        ;;
esac
