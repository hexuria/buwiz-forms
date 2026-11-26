import sys,time,json,sqlite3
sys.path.insert(0,'/home/ubuntu/forms-validation/money-accept-proof')
import drive_accept as d
RES=d.OUT/'fgc-results.jsonl';AMT='fp-txtATCAmt1';I15='fp-frm2551Qv2018:txt15'
def w(x):open(RES,'a').write(json.dumps(x,ensure_ascii=False)+'\n');print(x['case'],x['ak'],x['al'],x['saved'],x['sentence'],flush=True)
def data():
 c=sqlite3.connect(str(d.DB));x=json.loads(c.execute('select data_json from form_drafts where id=1').fetchone()[0]);c.close();return x
def msgs():
 out=[]
 for n in d.nodes():
  s=n.get('name') or ''
  if n.get('id','').startswith('fp-boxmsg') or 'not an amount' in s or 'kept' in s or 'you typed' in s:out.append(s)
 return out
def toast():
 return [(n.get('id'),n.get('name')) for n in d.nodes() if 'toast' in (n.get('id') or '').lower() or 'toast' in (n.get('name') or '').lower()]
def rowsum():return [(n.get('id'),n.get('name')) for n in d.nodes() if n.get('id','').startswith('fp-row-drpATC1-')]
def open_sched():
 if d.maybe('fp-table-drpATC1-done'):d.click_node('fp-table-drpATC1-done',wait=.5)
 if d.maybe('fp-nav-4'):d.click_node('fp-nav-4',wait=.8)
 if not d.maybe(AMT) and d.maybe('fp-row-drpATC1-0'):d.click_node('fp-row-drpATC1-0',wait=.8)
 if not d.maybe(AMT):raise RuntimeError('schedule amount not open')
def open_item():
 if d.maybe('fp-table-drpATC1-done'):d.click_node('fp-table-drpATC1-done',wait=.5)
 if d.maybe('fp-nav-2'):d.click_node('fp-nav-2',wait=.8)
 if not d.maybe(I15):raise RuntimeError('item15 not found')
def reset_sched():
 open_sched();d.click_node(AMT,wait=.2);d.key(['ctrl+a']);d.type_text('1234.56',delay=120);d.key(['Tab']);time.sleep(1)
def reset_item():
 open_item();d.click_node(I15,wait=.2);d.key(['ctrl+a']);d.type_text('1234.56',delay=120);d.key(['Tab']);time.sleep(1)
def case(where,digits):
 if where=='schedule':
  reset_sched();fid=AMT;path='schedule';drawer=True
 else:
  reset_item();fid=I15;path='item15';drawer=False
 # test edit: select the held amount, type the digit string with real keys, then Tab away
 d.click_node(fid,wait=.2);d.key(['ctrl+a']);d.type_text(digits,delay=120);time.sleep(.35)
 ak=d.value(fid) if d.maybe(fid) else None;ks=d.shot('x-'+where+'-'+str(len(digits))+'digits-keys')
 row_before_leave=rowsum();toast_before=toast()
 d.key(['Tab']);time.sleep(1.3);al=d.value(fid) if d.maybe(fid) else None;dd=data();ls=d.shot('x-'+where+'-'+str(len(digits))+'digits-leave')
 saved=dd['schedule_1'][0]['taxable_amount'] if where=='schedule' else dd.get('creditable_tax_withheld')
 w({'case':'X '+where+' '+str(len(digits))+' digits','digits':digits,'editing_inside_row_drawer':drawer,'ak':ak,'al':al,'saved':saved,'sentence':msgs(),'toast_before_leave':toast_before,'row_lines_before_leave':row_before_leave,'row_lines_after_leave':rowsum(),'shotk':ks,'shotl':ls})
case('schedule','12345678901234567890')
case('item15','12345678901234567890')
case('schedule','1234567890123')
case('item15','1234567890123')
w({'case':'X digits complete','ak':None,'al':None,'saved':None})
