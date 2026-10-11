"""List a frozen print sheet's fillable cells with the printed captions next to them.

Usage: python3 dump_cells.py html-frozen/<slug>/index.html [page]
One line per fillable cell:  id kind(slots) @left,top wXX | L: caption to the left | A: caption above
"""
import html, re, sys
s = open(sys.argv[1], encoding='utf-8').read()
only = sys.argv[2] if len(sys.argv) > 2 else None
texts = {}
for m in re.finditer(r'class="t" id="(p\d+)t\d+" style="([^"]*)">([^<]*)', s):
    pid, style, x = m.groups()
    tm = re.search(r'top:([\d.]+)pt', style)
    if not tm: continue
    # Some runs carry left:0 (or none) and put their x in transform: translate(Xpt, ...).
    lm = re.search(r'left:([\d.]+)pt', style)
    l = float(lm.group(1)) if lm else 0.0
    tx = re.search(r'translate\(([-\d.]+)pt', style)
    if tx and l == 0.0: l = float(tx.group(1))
    x = ' '.join(html.unescape(x).split())
    if x: texts.setdefault(pid, []).append((l, float(tm.group(1)), x))
for m in re.finditer(r'<div id="((p\d+)c\d+)" class="c f"([^>]*)>', s):
    cid, pid, attrs = m.groups()
    if only and pid != only: continue
    if 'data-cell-kind="field"' not in attrs: continue
    st = re.search(r'left:([\d.]+)pt;top:([\d.]+)pt;width:([\d.]+)pt;height:([\d.]+)pt', attrs)
    if not st: continue
    l, t, w, h = map(float, st.groups())
    fk = re.search(r'data-field-kind="([^"]*)"', attrs); cap = re.search(r'data-comb-capacity="(\d+)"', attrs)
    kind = (fk.group(1) if fk else '?') + (f'({cap.group(1)})' if cap else '')
    tx = texts.get(pid, [])
    left = sorted([x for x in tx if abs(x[1] - t) < 9 and x[0] < l], key=lambda x: x[0])[-2:]
    above = sorted([x for x in tx if 0 < t - x[1] < 16 and l - 60 < x[0] < l + w], key=lambda x: -x[1])[:2]
    print(f'{cid} {kind} @{l:.0f},{t:.0f} w{w:.0f} | L: ' + ' / '.join(x[2] for x in left)[:70] + ' | A: ' + ' / '.join(x[2] for x in above)[:60])
