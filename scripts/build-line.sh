#!/usr/bin/env bash
# What a build is made from, on one line to stderr, run from the repository's root before a recipe builds:
#   build: app <commit>, nsh <commit of ../nsh> beside it, tool <nsh --version>, <cargo --version>, folder <where it builds>
#
# --strict (the recipes whose output is evidence or a package): refuse a cargo that ignores CARGO_TARGET_DIR. A cargo
# built from huacnlee/cargo puts every checkout of a repository into one folder under $CARGO_HOME/targets, so a build
# can link another worktree's crates, form definitions included; those recipes put the pinned toolchain's own cargo
# first on PATH (scripts/pinned-cargo-path.sh), and this stops them if it is still the fork.
# Without --strict the line names that shared folder instead, so it never claims a folder the build did not use.
strict=0
[ "${1:-}" = "--strict" ] && strict=1
app="$(git rev-parse --short=8 HEAD 2>/dev/null || echo '?')"
git diff --quiet HEAD -- 2>/dev/null || app="$app (with uncommitted changes)"
kit="$(git -C ../nsh rev-parse --short=8 HEAD 2>/dev/null || echo '?')"
tool="$(nsh --version 2>/dev/null || echo missing)"
cargo_version="$(cargo --version 2>/dev/null || echo 'cargo missing')"
folder="${CARGO_TARGET_DIR:-unset}"
case "$cargo_version" in
    *huacnlee/cargo*)
        if [ "$strict" = 1 ]; then
            echo "❌ $(command -v cargo) is $cargo_version, which ignores CARGO_TARGET_DIR and shares one build folder between checkouts; evidence and packages are built with the pinned toolchain's own cargo (rustup which cargo)." >&2
            exit 1
        fi
        folder="the folder this cargo keeps for the repository under ${CARGO_HOME:-$HOME/.cargo}/targets, shared by every checkout (it ignores CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-unset})" ;;
esac
echo "build: app $app, nsh $kit beside it, tool $tool, $cargo_version ($(command -v cargo)), folder $folder" >&2
