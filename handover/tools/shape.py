import re, sys
def shape(v):
    if v == '': return 'empty'
    tags = []
    if re.search(r'%u[0-9A-Fa-f]{4}', v): tags.append('%uXXXX')
    if re.search(r'%[0-9A-Fa-f]{2}', v): tags.append('%XX')
    if ' ' in v: tags.append('raw-space')
    if '@' in v: tags.append('raw-@')
    if re.fullmatch(r'-?\d+(\.\d+)?', v): tags.append('number' + ('.2' if re.fullmatch(r'-?\d+\.\d\d', v) else ''))
    if v in ('true', 'false'): tags = ['bool']
    return ','.join(tags) or 'plain'
for path in sys.argv[1:]:
    t = open(path, encoding='utf-8', errors='replace').read()
    divs = re.findall(r'<div>([^=<]+)=(.*?)\1=</div>', t, re.S)
    tail = t[t.rfind('</div>') + 6:]
    print(f'== {path}')
    print('  header:', repr(t[:t.find("<div>")]), ' sep after div:', repr(re.search(r'</div>(\s*)<div>', t).group(1)))
    print('  trailer:', repr(re.sub(r'\d{9,}', '<digits>', tail)))
    counts = {}
    for k, v in divs:
        counts.setdefault(shape(v), []).append(k.split(':')[-1])
    for s, ks in sorted(counts.items()):
        print(f'  {s:22} {len(ks):3}  ' + (' '.join(ks) if s not in ('bool', 'empty', 'number.2', 'number') else ''))
