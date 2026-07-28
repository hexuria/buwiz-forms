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
#
# Deliberately NOT listed: rsx's auto-injected source-location `.id(..)`. When
# an element carries a stateful attribute but rsx did not see an `id=`
# attribute, it injects its own id - which SILENTLY OVERRIDES an explicit
# `.id()` the original chain had, changing element identity and any state keyed
# to it. That must surface as a diff, not be normalised away.
BENIGN = [
    # the macro import the migration has to add; affects no element tree
    (re.compile(r'usegpui_rsx::rsx;'), ''),
    # rule 7: `let root = <expr>; root.m()` is the same value as `<expr>.m()`.
    # Chaining onto a brace-delimited macro does not parse, so the binding is
    # forced by syntax rather than chosen.
    # rule 7 binding under any name: `let X = <expr>; X.into_any_element()` is
    # the same value as `<expr>.into_any_element()`. Anchored on the terminal
    # method and using a backreference, so it only collapses a binding that is
    # consumed immediately by the call it was created for.
    (re.compile(r'let([a-z_][a-z0-9_]*)=(.*?);return\1\.into_any_element\(\)', re.S),
     r'return\2.into_any_element()'),
    (re.compile(r'let([a-z_][a-z0-9_]*)=(.*?);\1\.into_any_element\(\)', re.S),
     r'\2.into_any_element()'),
    # same binding, returned directly instead of chained
    (re.compile(r'let([a-z_][a-z0-9_]*)=(.*?);return\1;', re.S), r'return\2;'),
    (re.compile(r'let([a-z_][a-z0-9_]*)=(.*?);return\1\}', re.S), r'return\2}'),
    (re.compile(r'letroot='), ''),
    (re.compile(r';root\.'), '.'),
    (re.compile(r';root\}'), '}'),
    # tracing!/log! bake __FILE__ and __LINE__ into call-site metadata, so
    # adding an import line shifts every line number below it. Anchored to the
    # file-then-line pair so unrelated integer literals are never touched.
    (re.compile(r'(\.rs",\),::tracing_core::__macro_support::Option::Some\()\d+(u32\))'), r'\1<LINE>\2'),
    (re.compile(r'\.rs:\d+"'), '.rs:<LINE>"'),
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
