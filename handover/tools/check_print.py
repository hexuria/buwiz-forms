"""Mirror bir-print's validate_writer_cells name= checks; --fix moves bad joins to left_out."""
import json, re, sys, glob, os
fix = '--fix' in sys.argv
for wc in sorted(glob.glob('html-frozen/*/writer-cells.json')):
    d = os.path.dirname(wc); html = open(d + '/index.html', encoding='utf-8').read()
    names = set(re.findall(r'<input[^>]*\bname="([^"]+)"', html))
    j = json.load(open(wc)); bad = []
    for key, ids in (('joins', ('html_id',)), ('money_joins', ('peso_html_id', 'cent_html_id')), ('xbox_joins', ('html_id',)), ('peso_joins', ('html_id',))):
        keep = []
        for m in j.get(key, []):
            if any(m[i] not in names for i in ids) or (key in ('joins', 'peso_joins') and m['writer_key'] in names):
                bad.append((m['writer_key'], m[ids[0]]))
            else: keep.append(m)
        j[key] = keep
    if bad:
        print(d, bad)
        if fix:
            lo = j.setdefault('left_out', [])
            for k, v in bad:
                if isinstance(lo, list): lo.append({'html_id': v, 'reason': f'{k}: no fillable input name= for this cell (mixed/label/stamped)'})
            open(wc, 'w').write(json.dumps(j, indent=1, ensure_ascii=False) + '\n')
