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
# shellcheck source=scripts/lib/prebuilt.sh
. "$IMPECCABLE_ROOT/scripts/lib/prebuilt.sh"

impeccable_pin() {  # impeccable_pin <KEY> → its value in scripts/impeccable.pin
    sed -n "s/^$1=\([^[:space:]]*\).*/\1/p" "$IMPECCABLE_PIN_FILE" 2>/dev/null | head -1 || true
}

# cargo install writes to $CARGO_INSTALL_ROOT, else $CARGO_HOME, else ~/.cargo.
# A distro Cargo in /usr/bin does not put that bin directory on PATH, so add it
# for this run (and say so) before installing or checking the CLI.
impeccable_use_cargo_bin() {
    local bin="${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}/bin"
    case ":$PATH:" in *":$bin:"*) return 0 ;; esac
    export PATH="$bin:$PATH"
    warn "$bin is not on PATH; using it for this run. Add it to your shell profile so impeccable is found."
}

impeccable_skill_at() {  # impeccable_skill_at <commit> → true when the skill is there at it
    local f
    [ "$(cat "$IMPECCABLE_SKILL_DIR/.commit" 2>/dev/null)" = "$1" ] || return 1
    for f in $IMPECCABLE_SKILL_FILES; do
        [ -s "$IMPECCABLE_SKILL_DIR/$f" ] || return 1
    done
}

install_impeccable() {
    local version
    version="$(impeccable_pin IMPECCABLE_VERSION)"
    [ -n "$version" ] || die "IMPECCABLE_VERSION not found in $IMPECCABLE_PIN_FILE"
    impeccable_use_cargo_bin
    log "Installing impeccable CLI $version (pinned in scripts/impeccable.pin)"
    install_pinned_tool impeccable "$version" --locked
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
    local w="$1" ok=0 version commit
    version="$(impeccable_pin IMPECCABLE_VERSION)"
    commit="$(impeccable_pin IMPECCABLE_SKILL_COMMIT)"
    impeccable_use_cargo_bin
    if ! prebuilt_report_pinned impeccable impeccable impeccable "$version"; then
        ok=1
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
