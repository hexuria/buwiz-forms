"""Rebuild FORM_CAPABILITY_REGISTRY from whole records found in git refs.

Usage: python3 fix_registry.py REF [REF...]   (run in repo root)
Order: records in the first ref's order, then new codes in later refs' order.
The working file's registry block is replaced.
"""
import re, subprocess, sys
P = 'crates/bir-core/src/forms/support_level.rs'
START = 'pub const FORM_CAPABILITY_REGISTRY: &[FormCapabilityRecord] = &['
def records(text):
    i = text.index(START) + len(START); j = text.index('\n];', i)
    body = text[i:j]
    out = []
    for m in re.finditer(r'(?:\n(?:    //[^\n]*\n)*)?    FormCapabilityRecord \{.*?\n    \},', body, re.S):
        blk = m.group(0).lstrip('\n')
        code = re.search(r'code: "([^"]+)"', blk).group(1)
        out.append((code, blk))
    return out
order, recs = [], {}
for ref in sys.argv[1:]:
    t = subprocess.run(['git', 'show', f'{ref}:{P}'], capture_output=True, text=True, check=True).stdout
    for code, blk in records(t):
        if code not in recs: order.append(code); recs[code] = blk; continue
        # Keep the record that claims more capabilities (a stale branch copy
        # never wins over a newer, richer one).
        if blk.count(': true') > recs[code].count(': true'): recs[code] = blk
s = open(P).read()
i = s.index(START) + len(START); j = s.index('\n];', i)
s = s[:i] + '\n' + '\n'.join(recs[c] for c in order) + s[j:]
open(P, 'w').write(s)
print(len(order), 'records:', ' '.join(order))
