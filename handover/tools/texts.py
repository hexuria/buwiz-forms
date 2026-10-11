"""Print text runs (page, top, left, text) sorted by position. Usage: texts.py <index.html> [page-id]"""
import html, re, sys
s = open(sys.argv[1], encoding='utf-8').read()
only = sys.argv[2] if len(sys.argv) > 2 else None
rows = []
for tid, pid, l, t, x in re.findall(r'class="t" id="((p\d+)t\d+)" style="left:([\d.]+)pt;top:([\d.]+)pt;[^"]*">([^<]*)', s):
    if only and pid != only: continue
    x = ' '.join(html.unescape(x).split())
    if x: rows.append((float(t), float(l), pid, x))
for t, l, pid, x in sorted(rows):
    print(f'{pid} t{t:.0f} l{l:.0f} {x}')
