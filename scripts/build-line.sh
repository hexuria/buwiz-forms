#!/usr/bin/env bash
# What a build is made from, on one line to stderr, run from the repository's root before a recipe builds:
#   build: app <commit>, nsh <commit of ../nsh> beside it, tool <nsh --version>, <cargo --version>, folder <where it builds>
# The folder is CARGO_TARGET_DIR, unless this machine's cargo ignores it (a cargo built from huacnlee/cargo always builds a
# workspace in its own ./target and keeps registry crates in $CARGO_HOME/targets/shared-deps): then the line names those,
# so it never claims a folder the build did not use.
app="$(git rev-parse --short=8 HEAD 2>/dev/null || echo '?')"
git diff --quiet HEAD -- 2>/dev/null || app="$app (with uncommitted changes)"
kit="$(git -C ../nsh rev-parse --short=8 HEAD 2>/dev/null || echo '?')"
tool="$(nsh --version 2>/dev/null || echo missing)"
cargo_version="$(cargo --version 2>/dev/null || echo 'cargo missing')"
folder="${CARGO_TARGET_DIR:-unset}"
case "$cargo_version" in
    *huacnlee/cargo*)
        folder="$(pwd -P)/nsh/.nsh/rust/target and ${CARGO_HOME:-$HOME/.cargo}/targets/shared-deps (this cargo ignores CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-unset})" ;;
esac
echo "build: app $app, nsh $kit beside it, tool $tool, $cargo_version, folder $folder" >&2
