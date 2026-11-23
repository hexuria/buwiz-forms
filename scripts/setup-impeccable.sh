#!/usr/bin/env bash
#
# setup-impeccable.sh — the impeccable verification CLI and the impeccable-rust
# agent skill, at the versions scripts/impeccable.pin holds.
#
# setup-linux.sh and setup-macos.sh source this file for its functions (it
# uses their log/warn/die). Run directly, it installs both and exits; the
# Devin blueprint's maintenance does that when the pin moves:
#
#   bash scripts/setup-impeccable.sh
#
# The skill lands in .agents/skills/impeccable-rust/ at the repository root,
# where agents look for skills. .agents/ is git-ignored (agent artifacts are
# never committed), so the files are downloaded at the pinned commit instead.

IMPECCABLE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMPECCABLE_PIN_FILE="$IMPECCABLE_ROOT/scripts/impeccable.pin"
IMPECCABLE_SKILL_DIR="$IMPECCABLE_ROOT/.agents/skills/impeccable-rust"
IMPECCABLE_SKILL_FILES="SKILL.md performance.md"

impeccable_pin() {  # impeccable_pin <KEY> → its value in scripts/impeccable.pin
    sed -n "s/^$1=\([^[:space:]]*\).*/\1/p" "$IMPECCABLE_PIN_FILE" 2>/dev/null | head -1 || true
}

impeccable_installed_version() {  # empty when impeccable is not on PATH
    command -v impeccable >/dev/null 2>&1 || return 0
    impeccable --version 2>/dev/null | sed -n 's/^impeccable \([0-9][0-9.]*\).*/\1/p' | head -1 || true
}

impeccable_skill_at() {  # impeccable_skill_at <commit> → true when the skill is there at it
    local f
    [ "$(cat "$IMPECCABLE_SKILL_DIR/.commit" 2>/dev/null)" = "$1" ] || return 1
    for f in $IMPECCABLE_SKILL_FILES; do
        [ -s "$IMPECCABLE_SKILL_DIR/$f" ] || return 1
    done
}

install_impeccable() {
    local version installed
    version="$(impeccable_pin IMPECCABLE_VERSION)"
    [ -n "$version" ] || die "IMPECCABLE_VERSION not found in $IMPECCABLE_PIN_FILE"
    installed="$(impeccable_installed_version)"
    if [ "$installed" = "$version" ]; then
        log "impeccable already installed at pinned version $version"
        return 0
    fi
    [ -z "$installed" ] || warn "installed impeccable version $installed does not match $version; reinstalling"
    log "Installing impeccable CLI $version (pinned in scripts/impeccable.pin)"
    cargo install --locked impeccable --version "$version"
}

# Each file goes to a temp name and is moved into place only once complete, and
# the .commit marker is written last, so a failed download never passes as done.
install_impeccable_skill() {
    local commit base f tmp
    commit="$(impeccable_pin IMPECCABLE_SKILL_COMMIT)"
    [ -n "$commit" ] || die "IMPECCABLE_SKILL_COMMIT not found in $IMPECCABLE_PIN_FILE"
    if impeccable_skill_at "$commit"; then
        log "impeccable-rust skill already at ${commit:0:7}"
        return 0
    fi
    log "Downloading the impeccable-rust skill at ${commit:0:7} into .agents/skills/impeccable-rust"
    mkdir -p "$IMPECCABLE_SKILL_DIR"
    rm -f "$IMPECCABLE_SKILL_DIR/.commit"
    base="https://raw.githubusercontent.com/hexuria/impeccable-rust/$commit/skill"
    for f in $IMPECCABLE_SKILL_FILES; do
        tmp="$IMPECCABLE_SKILL_DIR/.$f.tmp"
        if ! curl -fsSL -o "$tmp" "$base/$f"; then
            rm -f "$tmp"
            die "could not download $base/$f"
        fi
        mv -f "$tmp" "$IMPECCABLE_SKILL_DIR/$f"
    done
    printf '%s\n' "$commit" > "$IMPECCABLE_SKILL_DIR/.commit"
}

check_impeccable() {  # check_impeccable <label width> → prints two rows; false if either is off-pin
    local w="$1" ok=0 version installed commit
    version="$(impeccable_pin IMPECCABLE_VERSION)"
    commit="$(impeccable_pin IMPECCABLE_SKILL_COMMIT)"
    installed="$(impeccable_installed_version)"
    if [ -z "$installed" ]; then
        printf "  %-${w}s MISSING (pinned %s in scripts/impeccable.pin)\n" "impeccable" "$version"
        ok=1
    elif [ "$installed" != "$version" ]; then
        printf "  %-${w}s %s, but scripts/impeccable.pin pins %s\n" "impeccable" "$installed" "$version"
        ok=1
    else
        printf "  %-${w}s %s\n" "impeccable" "impeccable $installed"
    fi
    if impeccable_skill_at "$commit"; then
        printf "  %-${w}s %s at %s\n" "impeccable-skill" ".agents/skills/impeccable-rust" "${commit:0:7}"
    else
        printf "  %-${w}s MISSING (.agents/skills/impeccable-rust at %s)\n" "impeccable-skill" "${commit:0:7}"
        ok=1
    fi
    return "$ok"
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    set -euo pipefail
    log()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
    warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*" >&2; }
    die()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }
    install_impeccable
    install_impeccable_skill
fi
