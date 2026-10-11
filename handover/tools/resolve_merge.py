"""Resolve the recurring integration conflicts (run in the repo root).

- official_xml.rs LAYOUTS: union of ids, sorted.
- bir-rules-codegen status.rs EXPECTED: union of rows (ours wins on a code
  present on both sides), ordered like FORM_CAPABILITY_REGISTRY.
Other conflicted files are reported, not touched.
"""
import re, subprocess, sys
CONFLICT = re.compile(r'<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n', re.S)

def resolve(path, fn):
    s = open(path).read()
    if '<<<<<<<' not in s:
        return
    def pick(m):
        if 'LAYOUTS' in m.group(1) + m.group(2) or 'layouts![' in m.group(1) + m.group(2):
            return fn(m.group(1), m.group(2))
        if 'All Rights Reserved' in m.group(1) + m.group(2):
            return m.group(1)  # ours: the trailer test accepts any year
        print('  ! non-LAYOUTS hunk in', path, '- kept both sides for review')
        return m.group(0)
    s = CONFLICT.sub(pick, s)
    if '<<<<<<<' in s:
        open(path, 'w').write(s); return
    open(path, 'w').write(s)
    subprocess.run(['git', 'add', path], check=True)
    print('resolved', path)

def layouts(a, b):
    return a  # keep ours; fix_layouts.py rebuilds the block as a union
    ids = sorted(set(re.findall(r'"([^"]+)"', a)) | set(re.findall(r'"([^"]+)"', b)))
    return 'const LAYOUTS: &[(&str, &str)] = layouts![\n' + ''.join(f'    "{i}",\n' for i in ids) + '];\n'

ROW = re.compile(r'\("([^"]+)", "([^"]+)", "([^"]+)", (\d+), (true|false)\),')
def expected(a, b):
    rows = {}
    for side in (b, a):  # ours (a) last so it wins
        for m in ROW.finditer(side):
            rows[m.group(1)] = m.group(0)
    reg = open('crates/bir-core/src/forms/support_level.rs').read()
    order = re.findall(r'FormCapabilityRecord \{\s*code: "([^"]+)"', reg)
    missing = [c for c in order if c not in rows]
    extra = [c for c in rows if c not in order]
    if missing or extra:
        print('WARNING EXPECTED vs registry: missing', missing, 'extra', extra)
    codes = [c for c in order if c in rows] + extra
    body = ''.join(f'        {rows[c]}\n' for c in codes)
    total = len(codes) + len(re.findall(ROW, '')); 
    return f'    const EXPECTED: [(&str, &str, &str, u16, bool); __N__] = [\n' + body

resolve('crates/bir-core/src/official_xml.rs', layouts)
subprocess.run(['python3', __file__.rsplit('/', 1)[0] + '/fix_layouts.py'], check=True)
subprocess.run(['git', 'add', 'crates/bir-core/src/official_xml.rs'], check=True)
p = 'crates/bir-rules-codegen/src/status.rs'
s = open(p).read()
if '<<<<<<<' in s:
    s = CONFLICT.sub(lambda m: expected(m.group(1), m.group(2)), s)
    # fix array length: count rows in the EXPECTED block
    i = s.index('const EXPECTED: [(&str, &str, &str, u16, bool); ')
    j = s.index('];', i)
    n = len(ROW.findall(s[i:j]))
    s = re.sub(r'(const EXPECTED: \[\(&str, &str, &str, u16, bool\); )(__N__|\d+)\]', lambda m: f'{m.group(1)}{n}]', s, count=1)
    open(p, 'w').write(s)
    subprocess.run(['git', 'add', p], check=True)
    print('resolved', p, n, 'rows')
left = subprocess.run(['git', 'diff', '--name-only', '--diff-filter=U'], capture_output=True, text=True).stdout.split()
print('still conflicted:', left)
