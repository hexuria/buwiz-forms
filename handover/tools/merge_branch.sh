#!/usr/bin/env bash
# Merge one forms branch into the current branch with the integration resolvers.
S=/tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad
b=$1
pre=$(git rev-parse HEAD)
git merge --no-edit "$b" >/dev/null 2>&1
if git rev-parse -q --verify MERGE_HEAD >/dev/null; then
  # tooling: keep ours (the oracle is being rewritten centrally)
  for f in $(git diff --name-only --diff-filter=U | grep '^tools/official-xml/'); do git checkout --ours "$f"; git add "$f"; done
  python3 $S/tools/integrate.py
  python3 $S/tools/resolve_merge.py
  python3 $S/tools/fix_registry.py $pre "$b"
  python3 $S/tools/order_expected.py "$b"
  for f in $(git diff --name-only --diff-filter=U | grep '^crates/bir-print/'); do git checkout --ours "$f"; done
  python3 $S/tools/fix_print.py
  git add -A crates
  left=$(grep -rln '^<<<<<<< \|^>>>>>>> ' crates tools html-frozen 2>/dev/null)
  if [ -n "$left" ]; then echo "UNRESOLVED: $left"; exit 1; fi
  git commit -q --no-edit && echo "merged $b (resolved)"
else
  echo "merged $b cleanly"
  python3 $S/tools/fix_registry.py $pre "$b" >/dev/null; python3 $S/tools/order_expected.py "$b" >/dev/null
  git commit -qam "fix(integration): registry and freeze rows after merging $b" 2>/dev/null
fi
