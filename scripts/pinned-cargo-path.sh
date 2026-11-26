#!/usr/bin/env bash
# The folder of the pinned toolchain's own cargo (rust-toolchain.toml, through rustup), for a recipe to put first on
# PATH: a cargo installed ahead of rustup's proxy (as on a Mac with a huacnlee/cargo build) is then bypassed, and the
# nsh tool, which runs `cargo`, uses the same one. Prints nothing without rustup, leaving PATH as it is.
command -v rustup >/dev/null 2>&1 || exit 0
cargo="$(rustup which cargo 2>/dev/null)" || exit 0
dirname "$cargo"
