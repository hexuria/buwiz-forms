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
t0=time.monotonic();before=rowsummary(data())
# ensure row2 drawer open and type a replacement amount in it
if not d.maybe('fp-txtATCAmt2'):
 if d.maybe('fp-row-drpATC1-1'):d.click_node('fp-row-drpATC1-1',wait=.7)
rawclick('fp-txtATCAmt2');d.key(['ctrl+a']);d.type_text('7777',delay=120)
typed_at=time.monotonic();ak=d.value('fp-txtATCAmt2') if d.maybe('fp-txtATCAmt2') else None;shotk=d.shot('g-fast-row2-7777-keys')
# move to row1 and press its Delete as soon as row1's drawer is really present
click1=rawclick('fp-row-drpATC1-0');click1_at=time.monotonic()
# first click closes row2's sheet; as soon as it is gone, the same row1 row is clicked again to open row1
closed_at=None
for _ in range(200):
 if not d.maybe('fp-txtATCAmt2'):
  closed_at=time.monotonic();break
 time.sleep(.003)
click2=rawclick('fp-row-drpATC1-0');click2_at=time.monotonic()
ready_at=None
for _ in range(300):
 if d.maybe('fp-txtATCAmt1'):
  ready_at=time.monotonic();break
 time.sleep(.003)
if ready_at is None:raise RuntimeError('row1 drawer did not appear')
delclick=rawclick('fp-table-drpATC1-delete');delete_at=time.monotonic()
time.sleep(1.8)
dd=data();rows=rowsummary(dd);shotl=d.shot('g-fast-after-delete')
# visible row summary after delete
vis=[(n.get('id'),n.get('name')) for n in d.nodes() if n.get('id','').startswith('fp-row-drpATC1-')]
w({'case':'G fast row2 pending then row1 delete','before':before,'typed_box_after_keys':ak,'rows_after':rows,'saved_total_tax_due':dd.get('total_tax_due'),'visible_rows':vis,'timings_ms':{'type_end':round((typed_at-t0)*1000,1),'row1_click':round((click1_at-t0)*1000,1),'row1_ready':round((ready_at-t0)*1000,1),'delete_click':round((delete_at-t0)*1000,1),'close_click_to_delete':round((delete_at-click1_at)*1000,1),'open_click_to_delete':round((delete_at-click2_at)*1000,1),'sheet_close_detect':round((closed_at-click1_at)*1000,1) if closed_at else None},'clicks':{'close_row2_sheet':click1,'open_row1':click2,'delete':delclick},'shotk':shotk,'shotl':shotl})
