#!/usr/bin/env python3
"""Prove an rsx migration changed nothing.

rsx! expands to the same GPUI builder chain the hand-written code used, so a
correct migration is *token-identical* after expansion. Anything else - a
re-nested .child(), a dropped style, a reordered modifier - shows up here as a
diff. This is the only check that catches layout regressions, since nothing in
the crate renders an element tree in a test.

Usage:
  cargo expand -p bir-desktop <module> > baseline.rs   # BEFORE migrating
  # ... migrate ...
  cargo expand -p bir-desktop <module> > after.rs
  scripts/verify_rsx_expansion.py baseline.rs after.rs <module>
"""
import re, sys

# Differences that are expected and semantically inert.
BENIGN = [
    # the macro import the migration has to add; affects no element tree
    (re.compile(r'usegpui_rsx::rsx;'), ''),
    # rsx injects a source-location id on elements carrying stateful handlers
    (re.compile(r'\.id\(concat!\([^)]*\)\)'), '.id(<AUTO>)'),
    (re.compile(r'\.id\(format!\([^)]*\)\)'), '.id(<AUTO>)'),
]

def normalise(src: str) -> str:
    src = re.sub(r'//[^\n]*', '', src)            # line comments
    src = re.sub(r'/\*.*?\*/', '', src, flags=re.S)  # block comments
    src = re.sub(r'\s+', '', src)                 # all whitespace
    # BENIGN patterns are written against the whitespace-stripped form, so they
    # must be applied last. Applying them earlier silently matches nothing.
    for pat, rep in BENIGN:
        src = pat.sub(rep, src)
    return src

def main() -> int:
    base, new = open(sys.argv[1]).read(), open(sys.argv[2]).read()
    label = sys.argv[3] if len(sys.argv) > 3 else sys.argv[2]
    a, b = normalise(base), normalise(new)
    if a == b:
        print(f"  IDENTICAL  {label}  ({len(a)} tokens)")
        return 0
    # locate and show the first divergence with context
    i = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
    lo, hi = max(0, i - 90), i + 90
    print(f"  DIFFERS    {label}  (baseline {len(a)} vs migrated {len(b)} tokens, first at {i})")
    print(f"    baseline: ...{a[lo:hi]}...")
    print(f"    migrated: ...{b[lo:hi]}...")
    return 1

if __name__ == '__main__':
    sys.exit(main())
