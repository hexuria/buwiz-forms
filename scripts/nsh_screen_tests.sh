#!/usr/bin/env bash
# The headless TSX tests of nsh/, one `just test <file>` run per file, as `just nsh-test` does (nsh/verify holds
# real-window verifiers that need the owner's data copy, so it is left out). Run from nsh/. The macOS and Linux jobs of
# .github/workflows/nsh.yml run it; the Windows job has the same rules in PowerShell (docs/CI.md).
#
# SHARD and SHARDS: line k of the sorted list (from 0) belongs to shard k % SHARDS, so every file is in exactly one
# shard and the shards together run the whole list. LC_ALL=C makes the order the same on every runner.
# SCREENS_FILTER (optional, applied before sharding): a glob such as `screens/form/**`, where * also crosses folders,
# or a plain file or folder path such as `screens/form`.
set -euo pipefail

shard="${SHARD:-0}"
shards="${SHARDS:-1}"
filter="${SCREENS_FILTER:-}"

# The unquoted $filter below is a pattern on purpose.
# shellcheck disable=SC2053
wanted() {
  local path="${1#./}"
  case "$filter" in
    '') return 0 ;;
    *[*?[]*) [[ $path == $filter ]] ;;
    *) [[ $path == "${filter%/}" || $path == "${filter%/}"/* ]] ;;
  esac
}

files=()
while IFS= read -r file; do
  if wanted "$file"; then files+=("$file"); fi
done < <(find . -mindepth 1 \( -path ./verify -o -name node_modules -o -name target -o -name '.*' \) -prune -o \( -name '*.test.ts' -o -name '*.test.tsx' \) -print | LC_ALL=C sort)

if [ "${#files[@]}" -eq 0 ]; then
  echo "::error::no test files${filter:+ match $filter}"
  exit 1
fi

failed=()
ran=0
k=0
while [ "$k" -lt "${#files[@]}" ]; do
  if [ $((k % shards)) -eq "$shard" ]; then
    ran=$((ran + 1))
    just test "${files[$k]}" || failed+=("${files[$k]}")
  fi
  k=$((k + 1))
done
echo "shard $shard of $shards ran $ran of ${#files[@]} test files${filter:+ matching $filter}"
if [ "$ran" -eq 0 ]; then
  # Without a filter every shard has files; with one, a short list can leave a shard empty.
  [ -n "$filter" ] || { echo "::error::this shard found no test files"; exit 1; }
fi
if [ "${#failed[@]}" -gt 0 ]; then
  echo "failed in:"
  printf '  %s\n' "${failed[@]}"
  exit 1
fi
