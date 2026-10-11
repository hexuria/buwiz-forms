"""Rebuild bir-print's writer_cells_json() arms and build.rs SLUGS from the
html-frozen/<slug>/writer-cells.json files present (run in repo root)."""
import os, re
F = 'crates/bir-print/src/frozen_html.rs'; B = 'crates/bir-print/build.rs'
slugs = sorted(d for d in os.listdir('html-frozen') if os.path.isfile(f'html-frozen/{d}/writer-cells.json'))
s = open(F).read()
i = s.index('fn writer_cells_json(slug: &str) -> Option<&\'static str> {')
j = s.index('\n}\n', i) + 3
arms = ''.join(f'        "{x}" => Some(include_str!(\n            "../../../html-frozen/{x}/writer-cells.json"\n        )),\n' for x in slugs)
s = s[:i] + "fn writer_cells_json(slug: &str) -> Option<&'static str> {\n    match slug {\n" + arms + '        _ => None,\n    }\n}\n' + s[j:]
open(F, 'w').write(s)
b = open(B).read()
i = b.index('const SLUGS: &[&str] = &['); j = b.index('];', i)
have = re.findall(r'"([^"]+)"', b[i:j])
allslugs = have + [x for x in slugs if x not in have]
b = b[:i] + 'const SLUGS: &[&str] = &[\n' + ''.join(f'    "{x}",\n' for x in dict.fromkeys(allslugs)) + b[j:]
open(B, 'w').write(b)
print('writer-cells:', slugs)
