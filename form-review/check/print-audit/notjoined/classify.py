import json, csv, re, os
from collections import Counter
ROOT='/home/ubuntu/repos/bf-auditmain'
nj=json.load(open('/tmp/printaudit/notjoined.json'))
rows=list(csv.DictReader(open('/tmp/printaudit/per-key.tsv'),delimiter='\t'))
printed={}
for r in rows:
    printed.setdefault((r['form'],r['sample']),[]).append(r)
SLUG2PKG={'1801-2018':'1801-v2018','1701a-2018':'1701a-v2018','2200p-2020':'2200p-v2020','1707a-2021':'1707a-v2021','2200m-2018':'2200m-v2018','1700-2018':'1700-v2018','1702ex-2018':'1702ex-v2018','1702mx-2018c':'1702mx-v2018c','1707-2021':'1707-v2021','1701ms-2024':'1701ms-v2024'}
def fields_for(pkg):
    p=f'{ROOT}/rules/forms/{pkg}/fields.json'
    if not os.path.exists(p): return {}
    return {f['field_key']:f for f in json.load(open(p)).get('fields',[])}
def nrm(x):
    x=(x or '').strip().replace(',','').replace(' ','')
    try:
        f=float(x); return str(int(f)) if f==int(f) else ('%.2f'%f).rstrip('0').rstrip('.')
    except ValueError: return re.sub(r'\s+',' ',x).upper()
out=[]; counts={}
for form,pkg in SLUG2PKG.items():
    if form not in nj: continue
    F=fields_for(pkg)
    c=Counter()
    for stem,key,offv in nj[form]:
        fld=F.get(key)
        kind=(fld or {}).get('control_kind','')
        kl=key.lower()
        ov=nrm(offv)
        cls=None; note=''
        if any(x in kl for x in ('currentpage','maxpage','txtversion','addedrowcount','rowcount','disabledinputs','enabledlinks','hid','hdn')) or kind in ('hidden','button'):
            cls='c'; note='control metadata / hidden / pagination-state, not a printed box'
        elif kind in ('radio','checkbox') or kl.startswith(('frm'+pkg.split('-')[0])) and ('rdo' in kl or 'chk' in kl.split(':')[-1][:3]):
            cls='c'; note='radio/checkbox option input; the sheet marks the choice with X'
        elif 'modal' in kl:
            cls='c'; note='row-entry modal input, not itself a sheet cell'
        elif not ov:
            cls='c'; note='empty after normalization'
        else:
            # search printed cells of this sample for the value
            pr=printed.get((form,stem),[])
            hit=None; exact=False
            variants={ov}
            try:
                fv=float((offv or '').replace(',',''))
                variants.add(str(int(round(fv))))
            except (ValueError,TypeError): pass
            for r in pr:
                pv=nrm(r['printed'])
                if not pv: continue
                if pv in variants:
                    hit=r['key']; exact=True; break
            if not hit and ov.isdigit() and 3<=len(ov)<5:
                for r in pr:
                    if nrm(r['printed'])==ov:
                        hit=r['key']; exact=True; break
            if not hit and len(ov)>=5 and not ov.isdigit():
                for r in pr:
                    if ov in nrm(r['printed']):
                        hit=r['key']; break
            if not hit and ov.replace('.','').isdigit():
                # concatenate ALL printed digits in order (comb-per-digit printing)
                stream=''.join((r['printed'] or '').replace(',','').replace('.','').replace(' ','') for r in pr)
                stream=re.sub(r'\D','',stream)
                cands={ov.replace('.','')}
                try:
                    fv=float((offv or '').replace(',',''))
                    cands.add(str(int(round(fv))))
                except (ValueError,TypeError): pass
                if any(len(x)>=5 and x in stream for x in cands):
                    hit='comb digits';
            if hit:
                cls='b'; note=f'carried by {hit}' + ('' if exact else ' (text substring)')
            else:
                item=(fld or {}).get('item_number')
                cls='a'; note=f'visible input, item {item}, nowhere in the printed output'
        if cls=='a' and not fld:
            cls='d'; note='key absent from fields.json - likely a legacy/duplicate serialization key, unconfirmed'
        c[cls]+=1
        out.append([form,stem,key,offv,cls,str((fld or {}).get('item_number') or ''),note])
    counts[form]=dict(c)
with open('/tmp/njfinal.tsv','w') as f:
    f.write('form\tsample\tkey\tofficial\tclass\titem\tnote\n')
    for r in out: f.write('\t'.join(x.replace('\t',' ') for x in r)+'\n')
for form in SLUG2PKG:
    if form in counts: print(form, counts[form])
