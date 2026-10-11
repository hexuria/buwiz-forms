"""Finish an in-progress `git merge` of a forms branch (run in repo root).

Usage: python3 integrate.py   (after `git merge --no-edit <branch>` stopped)
"""
import re, subprocess, sys
sys.path.insert(0, __file__.rsplit('/', 1)[0])
CONFLICT = re.compile(r'<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n', re.S)
def sh(*a): return subprocess.run(a, capture_output=True, text=True).stdout
def theirs(path): return sh('git', 'show', f'MERGE_HEAD:{path}')
conflicted = sh('git', 'diff', '--name-only', '--diff-filter=U').split()

QK = 'crates/bir-core/src/forms/queueable.rs'
VS = 'crates/bir-desktop/src/views/queueable_forms.rs'
ARM = re.compile(r'QueueableKind::(\w+) => \{\s*type \$ty = (\$crate::[\w:]+);')
SPEC_OLD = re.compile(r'form_view_spec!\(\s*(\w+),\s*([\w:]+),\s*"([^"]+)",\s*"([^"]+)"')
SPEC_NEW = re.compile(r'^\s*(\w+), ([\w:]+), "([^"]+)", "([^"]+)";', re.M)

ARM_BLOCK = re.compile(r'            \$crate::forms::queueable::QueueableKind::(\w+) => \{\n                type \$ty = (\$crate::[\w:]+);\n                \$body\n            \}\n')
def one_line_kinds(s):
    return ARM_BLOCK.sub(lambda m: f'            $crate::forms::queueable::QueueableKind::{m.group(1)} => {{ type $ty = {m.group(2)}; $body }}\n', s)

def one_line_specs(s):
    if 'form_view_specs! {' in s: return s
    i = s.index('pub const FORM_VIEW_SPECS'); j = s.index('];', i) + 3
    specs = SPEC_OLD.findall(s[i:j])
    new = ('''/// One line per form: `Variant, view type, "CODE", "slug";`.
macro_rules! form_view_specs {
    ($($variant:ident, $view:ty, $code:literal, $slug:literal;)*) => {
        pub const FORM_VIEW_SPECS: &[FormViewSpec] = &[$(form_view_spec!($variant, $view, $code, $slug),)*];
    };
}

form_view_specs! {
''' + ''.join(f'    {v}, {t}, "{c}", "{g}";\n' for v, t, c, g in specs) + '}\n')
    return s[:i] + new + s[j:]

def merge_kinds():
    subprocess.run(['git', 'checkout', '--ours', QK], check=True)
    s = one_line_kinds(open(QK).read())
    have = dict(ARM.findall(s))
    for variant, draft in ARM.findall(theirs(QK)):
        if variant in have: continue
        s = s.replace('    #[cfg(test)]\n    Test,\n}', f'    {variant},\n    #[cfg(test)]\n    Test,\n}}', 1)
        i = s.index('pub const ALL'); j = s.index('];', i)
        s = s[:j] + f'    QueueableKind::{variant},\n    ' + s[j:]
        k = s.index('            #[cfg(test)]\n            $crate::forms::queueable::QueueableKind::Test')
        s = s[:k] + f'            $crate::forms::queueable::QueueableKind::{variant} => {{ type $ty = {draft}; $body }}\n' + s[k:]
        print('  + kind', variant)
    open(QK, 'w').write(s)

def merge_specs():
    subprocess.run(['git', 'checkout', '--ours', VS], check=True)
    s = one_line_specs(open(VS).read())
    i = s.index('form_view_specs! {')
    j = s.index('\n}\n', i)
    have = {m[0] for m in SPEC_NEW.findall(s[i:j])}
    t = theirs(VS)
    add = [m for m in SPEC_OLD.findall(t) + SPEC_NEW.findall(t) if m[0] not in have]
    seen = set()
    lines = ''
    for v, ty, c, g in add:
        if v in seen or v == 'Variant': continue
        seen.add(v); lines += f'    {v}, {ty}, "{c}", "{g}";\n'; print('  + view', v)
    s = s[:j + 1] + lines + s[j + 1:]
    open(VS, 'w').write(s)

def union(path):
    s = open(path).read()
    def pick(m):
        ours, theirs_ = m.group(1), m.group(2)
        extra = ''.join(l for l in theirs_.splitlines(True) if l not in ours.splitlines(True))
        return ours + extra
    s = CONFLICT.sub(pick, s)
    open(path, 'w').write(s)

for path in conflicted:
    if path == QK: merge_kinds()
    elif path == VS: merge_specs()
    elif path.endswith(('official_xml.rs', 'status.rs')): continue
    elif path.endswith(('forms/mod.rs', 'views/mod.rs', 'sample_payloads/mod.rs', 'support_level.rs', 'build.rs', 'frozen_html.rs')): union(path)
    else:
        print('  ! manual:', path); continue
    subprocess.run(['git', 'add', path], check=True)
# pinned lists
import importlib.util
spec = importlib.util.spec_from_file_location('rm', __file__.rsplit('/', 1)[0] + '/resolve_merge.py')
rm = importlib.util.module_from_spec(spec); spec.loader.exec_module(rm)
