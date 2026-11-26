import sys,time,json,sqlite3,subprocess,os
sys.path.insert(0,'/home/ubuntu/forms-validation/money-accept-proof')
import drive_accept as d
RES=d.OUT/'fgc-results.jsonl';env={**os.environ,'DISPLAY':':0'}
def w(x):open(RES,'a').write(json.dumps(x,ensure_ascii=False)+'\n');print(json.dumps(x,ensure_ascii=False),flush=True)
def data():
 c=sqlite3.connect(str(d.DB));z=json.loads(c.execute('select data_json from form_drafts where id=1').fetchone()[0]);c.close();return z
def rawclick(node):
 n=d.find(node);sx,sy,wid,logical=d.screen_point(n);subprocess.run(['xdotool','windowactivate',str(wid)],env=env,capture_output=True);subprocess.run(['xdotool','mousemove',str(sx),str(sy),'click','1'],env=env);return {'id':node,'screen':[sx,sy],'logical':logical}
def rowsummary(dd):return [{'atc':r.get('atc'),'taxable_amount':r.get('taxable_amount'),'tax_due':r.get('tax_due')} for r in dd.get('schedule_1',[])]
case='G corrected one-second row2 typing then row1 delete';t0=time.monotonic();before=rowsummary(data())
if not d.maybe('fp-txtATCAmt2'):
 if d.maybe('fp-row-drpATC1-1'):d.click_node('fp-row-drpATC1-1',wait=.7)
rawclick('fp-txtATCAmt2');d.key(['ctrl+a']);d.type_text('7777',delay=120);typed_at=time.monotonic();time.sleep(1.0)
click1=rawclick('fp-row-drpATC1-0');click1_at=time.monotonic();closed_at=None
for _ in range(250):
 if not d.maybe('fp-txtATCAmt2'):closed_at=time.monotonic();break
 time.sleep(.002)
click2=rawclick('fp-row-drpATC1-0');click2_at=time.monotonic();ready_at=None
for _ in range(350):
 if d.maybe('fp-txtATCAmt1'):ready_at=time.monotonic();break
 time.sleep(.002)
if ready_at is None:raise RuntimeError('row1 drawer did not appear')
delclick=rawclick('fp-table-drpATC1-delete');delete_at=time.monotonic()
time.sleep(1.7);shotk=d.shot('g-delay-corrected-after-delete');dd=data();rows=rowsummary(dd);vis=[(n.get('id'),n.get('name')) for n in d.nodes() if n.get('id','').startswith('fp-row-drpATC1-')]
w({'case':case,'before':before,'typed_box_after_keys':'7777 (not pre-read to preserve exact one-second timing)','rows_after':rows,'saved_total_tax_due':dd.get('total_tax_due'),'visible_rows':vis,'timings_ms':{'type_end':round((typed_at-t0)*1000,1),'type_end_to_delete':round((delete_at-typed_at)*1000,1),'row1_click':round((click1_at-typed_at)*1000,1),'row1_ready_after_typing':round((ready_at-typed_at)*1000,1),'close_click_to_delete':round((delete_at-click1_at)*1000,1),'open_click_to_delete':round((delete_at-click2_at)*1000,1),'sheet_close_detect':round((closed_at-click1_at)*1000,1) if closed_at else None},'clicks':{'close_row2_sheet':click1,'open_row1':click2,'delete':delclick},'shotk':shotk,'shotl':shotk})
