import json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from controls import controls
S = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SER = {'text', 'select-one', 'radio', 'checkbox'}
for hta, prefix, sample, rules in [
    ('BIR-Form1601Cv2018.hta', 'frm1601c', '1601C-062025', '1601c-v2018'),
    ('BIR-Form2551Qv2018.hta', 'frm2551Qv2018', '2551Q-122025Q1', '2551q-v2018'),
]:
    ctl = [(t, i.replace('@RDO', prefix + ':txtRDOCode')) for t, i, n in controls(f'{S}/validation/package/forms/{hta}')]
    official = [i for t, i in ctl if t in SER]
    noid = [x for x in ctl if x[0] in SER and not x[1]]
    dup = sorted({i for i in official if official.count(i) > 1})
    xml = open(f'{S}/forms/crates/bir-core/tests/sample-payloads/{sample}.plain.xml').read()
    ours = re.findall(r'<div>([^=<]+)=', xml)
    o, u = set(official), set(ours)
    print(f'===== {hta}: official {len(official)} serialized controls ({len(o)} unique), ours {len(ours)}')
    if noid: print('  controls without id (serialized with empty key):', noid)
    if dup: print('  duplicate official ids:', dup)
    print('  MISSING from ours (official writes, we do not):')
    for i in official:
        if i not in u: print('    ', dict(ctl)[i] if i in dict(ctl) else '', i)
    print('  EXTRA in ours (we write, official does not):')
    for i in ours:
        if i not in o: print('    ', i)
    inv = f'{S}/forms/rules/forms/{rules}/fixtures/runtime-control-inventory-v796.json'
    if os.path.exists(inv):
        txt = open(inv).read()
        ids = set(re.findall(r'"(?:id|control_id|element_id|field_key)"\s*:\s*"([^"]+)"', txt))
        print(f'  rule inventory ids: {len(ids)}; official-not-in-inventory:', sorted(o - ids)[:10], ' inventory-not-official:', sorted(i for i in ids - o if ':' in i)[:10])
