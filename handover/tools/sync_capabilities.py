"""Add (never remove) editor/print capability flags for generic-queue forms from
what exists, then set the freeze-gate EXPECTED rows to what the gate reads.
Run in repo root. Needs: source env.sh; CARGO_TARGET_DIR for codegen.
editor <- a view in FORM_VIEW_SPECS (or bespoke 2551Q/1601C)
render_contract, html_component, html_spec, pagination <- html-frozen/<slug>/writer-cells.json
"""
import os, re, subprocess
P = 'crates/bir-core/src/forms/support_level.rs'
def recs(t): return {m.group(1): m.group(0) for m in re.finditer(r'    FormCapabilityRecord \{\n        code: "([^"]+)",.*?\n    \},', t, re.S)}
cur = open(P).read()
specs = open('crates/bir-desktop/src/views/queueable_forms.rs').read()
views = set(re.findall(r'^\s*\w+, [\w:]+, "([^"]+)", "[^"]+";', specs, re.M)) | {'2551Q', '1601C'}
wc = {d for d in os.listdir('html-frozen') if os.path.isfile(f'html-frozen/{d}/writer-cells.json')}
SLUG = {'1601FQ': '1601-fq-2020', '1601EQ': '1601eq-2019', '1602Q': '1602q-2019', '2000': '2000-dst-2018', '2000OT': '2000-ot-2018',
        '1600VT': '1600-vt-2018', '1600PT': '1600-pt-2018', '2200T': '2200t-2022', '2200A': '2200a-2020', '1707': '1707-2021'}
def slug(code):
    if code in SLUG: return SLUG[code]
    c = sorted(d for d in os.listdir('html-frozen') if d.split('-')[0] == code.lower()); return c[0] if c else None
# Frozen sheet is a different revision from the return filed (2200T: 2022 sheet; 1601EQ/1602Q: 2019; 1601FQ: 2020).
NO_PREVIEW = set()  # mismatched-revision sheets still preview; pages must stay openable
added = []
for code, blk in recs(cur).items():
    if 'queue_submission: true' not in blk and not re.search(r'\beditor: true', blk): continue
    want = (['editor'] if code in views else []) + (['render_contract', 'html_component', 'html_spec', 'pagination'] if slug(code) in wc and code not in NO_PREVIEW else [])
    nb = blk
    for f in want:
        if re.search(rf'\b{f}: true', nb): continue
        if re.search(rf'\b{f}: false,', nb): nb = re.sub(rf'\b{f}: false,', f'{f}: true,', nb)
        else: nb = nb.replace('            ..SCAFFOLD', f'            {f}: true,\n            ..SCAFFOLD', 1)
    if nb != blk: cur = cur.replace(blk, nb); added.append(code)
open(P, 'w').write(cur); print('flags added for', added)
out = subprocess.run(['cargo', 'run', '-q', '-p', 'bir-rules-codegen', '--', 'status', '--boundaries-only'], capture_output=True, text=True)
m = re.search(r'found \[(.*)', out.stdout + out.stderr)
if m:
    F = re.findall(r'code: "([^"]+)", revision: "([^"]+)", form_id: "([^"]+)", capability_bits: (\d+), release_ready: (\w+)', m.group(1))
    p = 'crates/bir-rules-codegen/src/status.rs'; s = open(p).read()
    i = s.index('const EXPECTED: [(&str, &str, &str, u16, bool); '); j = s.index('];', i)
    s = s[:i] + f'const EXPECTED: [(&str, &str, &str, u16, bool); {len(F)}] = [\n' + ''.join(f'        ("{a}", "{b}", "{c}", {d}, {e}),\n' for a, b, c, d, e in F) + '    ' + s[j:]
    open(p, 'w').write(s); print(len(F), 'freeze rows from the gate')
else:
    print('freeze rows already match')
