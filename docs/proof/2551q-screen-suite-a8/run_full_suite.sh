#!/usr/bin/env bash
set -u
base=/home/ubuntu/urgent2551/a8aeaca8
out=/home/ubuntu/urgent2551/full-suite-a8
mkdir -p "$out"
cd "$base/buwiz-forms/nsh"
find . -mindepth 1 \( -path ./verify -o -name node_modules -o -name target -o -name '.*' \) -prune -o \( -name '*.test.ts' -o -name '*.test.tsx' \) -print | LC_ALL=C sort > "$out/files.txt"
run_shard() {
  local shard=$1
  mkdir -p "$out/shard$shard"
  : > "$out/shard$shard/summary.txt"
  awk -v n=3 -v i="$shard" '(NR - 1) % n == i' "$out/files.txt" | while IFS= read -r file; do
    safe=$(printf '%s' "$file" | sed 's#^\./##; s#[/.]#_#g')
    (cd "$base/buwiz-forms/nsh" && HOME="$base/home" "$base/nsh2404.sh" env XDG_RUNTIME_DIR="$base/home/xdg-run" RUSTC_WRAPPER= RUSTUP_TOOLCHAIN=1.99.0 CARGO_PROFILE_DEV_DEBUG=0 just test "$file" < /dev/null) > "$out/shard$shard/$safe.log" 2>&1
    rc=$?
    printf '%s rc=%s\n' "$file" "$rc" >> "$out/shard$shard/summary.txt"
  done
}
run_shard 0 & p0=$!
run_shard 1 & p1=$!
run_shard 2 & p2=$!
wait "$p0" "$p1" "$p2"
cat "$out"/shard*/summary.txt | sort > "$out/summary.txt"
echo total:$(wc -l < "$out/summary.txt") failed:$(grep -vc 'rc=0$' "$out/summary.txt" || true)
grep -v 'rc=0$' "$out/summary.txt" || true
