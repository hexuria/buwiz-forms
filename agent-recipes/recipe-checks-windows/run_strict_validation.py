import json, os, sqlite3, subprocess, time
from pathlib import Path

ROOT=Path(r'C:\Users\Administrator\recipe-checks')
REPO=ROOT/'buwiz-forms'
OUT=REPO/'agent-recipes'/'recipe-checks-windows'/'runs'
CLI=ROOT.parent/'Administrator'/'.cargo'/'bin'/'gpui-agent.exe'
# Path above is wrong on Windows pathlib in some contexts; use exact string below.
CLI=r'C:\Users\Administrator\.cargo\bin\gpui-agent.exe'
SCHEMA=REPO/'agent-recipes'/'bir.schema.json'
DB=ROOT/'scratch.db'
TOKEN=(ROOT/'agent-token').read_text().strip()
SURF='127.0.0.1:17421'
DATA='127.0.0.1:17422'
TIN='00012345300000'
OUT.mkdir(parents=True,exist_ok=True)

def run_recipe(path, addr, receipt, err, schema=False, sets=None):
    cmd=[CLI,'--addr',addr,'recipe','run',str(path)]
    if schema: cmd += ['--schema',str(SCHEMA)]
    for k,v in (sets or {}).items(): cmd += ['--set',f'{k}={v}']
    cmd += ['--receipt-out',str(receipt)]
    env=os.environ.copy(); env['GPUI_AGENT_TOKEN']=TOKEN
    p=subprocess.run(cmd,cwd=str(ROOT),env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=60)
    Path(err).write_text(p.stderr+p.stdout)
    try: rec=json.loads(Path(receipt).read_text())
    except Exception: rec={'ok':False,'parse_error':'missing/invalid receipt'}
    return p.returncode,rec,(p.stderr+p.stdout).strip()

def snapshot(addr,path):
    env=os.environ.copy(); env['GPUI_AGENT_TOKEN']=TOKEN
    p=subprocess.run([CLI,'--addr',addr,'snapshot'],cwd=str(ROOT),env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=20)
    Path(path).write_text(p.stdout)
    try: return json.loads(p.stdout)
    except Exception: return {}

def walk(n):
    yield n
    for c in n.get('children',[]): yield from walk(c)

def nodes(s):
    for root in s.get('tree',{}).get('nodes',[]):
        yield from walk(root)

def node(s,node_id):
    for n in nodes(s):
        if n.get('id')==node_id: return n

def draft(year,quarter):
    con=sqlite3.connect(DB)
    row=con.execute("select id,status,data_json,updated_at from form_drafts where tin=? and form_code='2551Q' and taxable_year=? and quarter=? order by id desc limit 1",(TIN,year,quarter)).fetchone()
    con.close()
    if not row: return None
    try: data=json.loads(row[2])
    except Exception: data={}
    return {'id':row[0],'status':row[1],'updated_at':row[3],'data':data}

def write_report(dirpath,obj):
    dirpath.mkdir(parents=True,exist_ok=True)
    (dirpath/'report.json').write_text(json.dumps(obj,indent=2))
    lines=[f"# {obj['case']} {obj['run']}",'',f"verdict: `{obj['verdict']}`",f"known_gap: `{obj.get('known_gap')}`",'']
    for k in ['exact_sentence','row_exists_in_draft','box_text','saved_value','replace_vs_append','failure']:
        if k in obj: lines.append(f"- **{k}**: `{obj[k]}`")
    lines += ['','## steps']
    for st in obj.get('steps',[]): lines.append(f"- {st}")
    (dirpath/'report.md').write_text('\n'.join(lines)+'\n')

def make(path,text):
    path=Path(path); path.write_text(text); return path

def close_surface_form(work):
    reset=REPO/'agent-recipes'/'recipe-checks-windows'/'reset-form-ui.wants'
    return run_recipe(reset,SURF,work/'reset-receipt.json',work/'reset-err.txt')

def dismiss_data(work):
    p=make(work/'data-dismiss.wants','invoke form.dismiss discard=true\n')
    return run_recipe(p,DATA,work/'data-dismiss-receipt.json',work/'data-dismiss-err.txt',schema=True)

# ---- R1: one proving run, then stop at the hard sheet-press gap as instructed.
r1dir=OUT/'r1-2551q-schedule1-invalid-run1'
r1dir.mkdir(parents=True,exist_ok=True)
# close anything open and drop the data-plane session before starting clean.
close_surface_form(r1dir)
dismiss_data(r1dir)
r1recipe=make(r1dir/'r1-attempt.wants','''wait_until context-pill --timeout-ms 5000
click context-pill
wait_until tp-years-$year --timeout-ms 5000
click tp-years-$year
wait_until tp-new-return --timeout-ms 5000
click tp-new-return
wait_until new-return-2551Q --timeout-ms 5000
click new-return-2551Q
click new-return-period-trigger
wait_until new-return-period-opt-$period --timeout-ms 5000
click new-return-period-opt-$period
wait_until new-return-period-trigger value="Q$period $year" --timeout-ms 5000
click new-return-open
wait_until fp-pill-next --timeout-ms 20000
click fp-pill-open
wait_until fp-section-4 --timeout-ms 5000
click fp-section-4
wait_until fp-table-drpATC1 --timeout-ms 5000
click fp-table-drpATC1-add
wait_until fp-table-drpATC1-drawer --timeout-ms 5000
click fp-drpATC2-trigger
wait_until fp-drpATC2-opt-4 --timeout-ms 5000
click fp-drpATC2-opt-4
wait_until fp-txtATCAmt2 --timeout-ms 5000
type fp-txtATCAmt2 abc --key-delay-ms 120
key fp-txtATCAmt2 Tab
wait_until fp-table-drpATC1-unfinished --timeout-ms 5000
assert fp-table-drpATC1-unfinished
snapshot
''')
rc,rec,err=run_recipe(r1recipe,SURF,r1dir/'receipt.json',r1dir/'stderr-stdout.txt',sets={'year':2021,'period':1})
snap=snapshot(SURF,r1dir/'snapshot-after-recipe.json')
d=draft(2021,1)
sched=(d or {}).get('data',{}).get('schedule_1',[])
row2=sched[1] if len(sched)>1 else None
failure=(err or '').splitlines()[0] if err else ''
known=None
if 'offscreen' in failure or any('offscreen' in str(s.get('error','')) for s in rec.get('steps',[])):
    known='hexuria/nsh-gpui-agent#13 sheet press'
obj={
 'case':'r1-2551q-schedule1-invalid','run':1,'year':2021,'quarter':1,
 'verdict':'known_gap' if known else ('failed' if rc else 'passed'),
 'known_gap':known,'failure':failure,
 'exact_sentence':None,'row_exists_in_draft':bool(row2),
 'draft_row2':row2,'draft_id':(d or {}).get('id'),'draft_status':(d or {}).get('status'),
 'steps':[f"recipe rc={rc}; stopped at first hard failure", f"draft rows={len(sched)}"]
}
write_report(r1dir,obj)
# cleanup drawer if possible: full-screen empty button inside dialog is the sheet backdrop.
backdrop=None
for n in nodes(snap):
    b=n.get('bounds') or {}
    if n.get('role')=='button' and not n.get('name') and b.get('x')==0.0 and b.get('y')==0.0 and (b.get('w') or 0)>1000:
        backdrop=n.get('id'); break
if backdrop:
    p=make(r1dir/'close-sheet.wants',f'click {backdrop}\n')
    run_recipe(p,SURF,r1dir/'close-sheet-receipt.json',r1dir/'close-sheet-err.txt')
close_surface_form(r1dir)

# ---- R2 runs.
r2results=[]
# Ensure no stale data-plane form is open before the loop.
(OUT/'r2-preclean').mkdir(parents=True,exist_ok=True)
dismiss_data(OUT/'r2-preclean')
for i,year in enumerate(range(2021,2026),1):
    rd=OUT/f'r2-2551q-recomputed-money-type-run{i}'
    rd.mkdir(parents=True,exist_ok=True)
    # Any prior surface form should have been closed at the end of prior iteration; ignore cleanup failures here.
    data=make(rd/'r2-data-setup.wants',f'''invoke form.open code=2551Q year={year} period=2 tin={TIN}
invoke form.fill fields='{{"drpATC1":"1","txtATCAmt1":"1000"}}' source=document
invoke form.fill fields='{{"txtATCAmt1":"2000"}}' source=document
''')
    rc0,rec0,err0=run_recipe(data,DATA,rd/'data-receipt.json',rd/'data-stderr-stdout.txt',schema=True)
    preerr=''
    if rc0==0:
        # Open penalties. Separate recipe so the snapshot can find a dynamic EOPT Micro button.
        openpen=make(rd/'r2-open-penalties.wants','''wait_until fp-pill-next --timeout-ms 10000
click fp-pill-open
wait_until fp-section-2 --timeout-ms 5000
click fp-section-2
wait_until fp-frm2551Qv2018:txt20 --timeout-ms 5000
click fp-penalty-2:6
wait_until fp-frm2551Qv2018:txt20 --timeout-ms 5000
''')
        rc1,rec1,err1=run_recipe(openpen,SURF,rd/'open-penalties-receipt.json',rd/'open-penalties-err.txt')
        preerr=err1
        psnap=snapshot(SURF,rd/'snapshot-before-type.json')
        # If the year's EOPT tier card is shown, answer it through the recipe with the invented/demo tier Micro.
        micro=next((n.get('id') for n in nodes(psnap) if n.get('role') in ('button','radio') and n.get('name')=='Micro'),None)
        if micro:
            esc=micro.replace('"','\\"')
            mp=make(rd/'r2-answer-eopt.wants',f'click "{esc}"\n')
            run_recipe(mp,SURF,rd/'answer-eopt-receipt.json',rd/'answer-eopt-err.txt')
            time.sleep(0.7)
        typ=make(rd/'r2-type.wants','''click fp-frm2551Qv2018:txt20
type fp-frm2551Qv2018:txt20 9 --key-delay-ms 120
key fp-frm2551Qv2018:txt20 Tab
assert fp-frm2551Qv2018:txt20
snapshot
''')
        rc2,rec2,err2=run_recipe(typ,SURF,rd/'receipt.json',rd/'stderr-stdout.txt')
        # Poll because the app's autosave is asynchronous after blur.
        d=None
        for _ in range(12):
            d=draft(year,2)
            if d and d.get('data',{}).get('surcharge')==9.0: break
            time.sleep(0.5)
        fsnap=snapshot(SURF,rd/'snapshot-after-recipe.json')
        n=node(fsnap,'fp-frm2551Qv2018:txt20') or {}
        box=n.get('value')
        saved=(d or {}).get('data',{}).get('surcharge') if d else None
        replace = 'replaces_selection' if box=='9.00' else ('appended_or_other' if box else 'untested')
        verdict='passed' if rc2==0 and box=='9.00' and saved==9.0 else 'failed'
        obj={'case':'r2-2551q-recomputed-money-type','run':i,'year':year,'quarter':2,
             'verdict':verdict,'known_gap':None,'box_text':box,'saved_value':saved,
             'replace_vs_append':replace,'data_setup_rc':rc0,'surface_rc':rc2,
             'draft':{k:(d or {}).get(k) for k in ['id','status','updated_at']},
             'computed':{k:(d or {}).get('data',{}).get(k) for k in ['total_tax_due','tax_payable','interest','compromise','total_penalties','total_amount_payable']},
             'failure':err2 if rc2 else preerr,
             'steps':[f'data setup rc={rc0}',f'open penalties rc={rc1}',f'type rc={rc2}',f'eopt_micro_answered={bool(micro)}']}
    else:
        obj={'case':'r2-2551q-recomputed-money-type','run':i,'year':year,'quarter':2,'verdict':'failed','known_gap':None,'failure':err0,'steps':[f'data setup rc={rc0}']}
    write_report(rd,obj)
    r2results.append(obj)
    close_surface_form(rd)
    dismiss_data(rd)

summary={'r1':json.loads((OUT/'r1-2551q-schedule1-invalid-run1'/'report.json').read_text()),'r2':r2results}
(OUT/'strict-validation-summary.json').write_text(json.dumps(summary,indent=2))
print(json.dumps(summary,indent=2))
