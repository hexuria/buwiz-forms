import sys,time,json,sqlite3
sys.path.insert(0,'/home/ubuntu/forms-validation/money-accept-proof')
import drive_accept as d
RES=d.OUT/'fgc-results.jsonl';F='fp-frm1604c:txtSched1Adj1'
def w(x):open(RES,'a').write(json.dumps(x,ensure_ascii=False)+'\n');print(x['case'],x['ak'],x['al'],x['saved'],flush=True)
def data():
 c=sqlite3.connect(str(d.DB));x=json.loads(c.execute('select data_json from form_drafts where id=5').fetchone()[0]);c.close();return x
def msgs():return [n.get('name') for n in d.nodes() if n.get('id','').startswith('fp-boxmsg') or 'not an amount' in (n.get('name') or '') or 'you typed' in (n.get('name') or '')]
def openrow():
 if d.maybe('return-5'):d.click_node('return-5',wait=1.4)
 if d.maybe('fp-nav-2'):d.click_node('fp-nav-2',wait=.8)
 if not d.maybe(F):
  row='fp-row-frm1604c:txtSched1Date1-0'
  if d.maybe(row):d.click_node(row,wait=.8)
  elif d.maybe('fp-table-frm1604c:txtSched1Date1-add'):d.click_node('fp-table-frm1604c:txtSched1Date1-add',wait=.8)
 if not d.maybe(F):raise RuntimeError('adjustment field not open')
def reset0():
 d.click_node(F,wait=.2);d.key(['ctrl+a']);d.type_text('0',delay=120);d.key(['Tab']);time.sleep(1.1)
 return data()['months'][0]['adjustment']
def case(name,keys):
 z=reset0()
 d.click_node(F,wait=.2);d.key(['ctrl+a']);d.key(keys,delay=120);time.sleep(.3)
 ak=d.value(F) if d.maybe(F) else None;ks=d.shot('c-neg-'+name+'-keys')
 d.key(['Tab']);time.sleep(1.3);al=d.value(F) if d.maybe(F) else None;dd=data();row=dd['months'][0];ls=d.shot('c-neg-'+name+'-leave')
 w({'case':'C negative 1604C adjustment '+name,'setup_saved_zero':z,'ak':ak,'al':al,'saved':row['adjustment'],'total_adjustment':dd['total_adjustment'],'row_total_remitted':row['total_remitted'],'sentence':msgs(),'shotk':ks,'shotl':ls})
openrow()
case('parentheses',['parenleft','1','comma','0','0','0','period','0','0','parenright'])
case('minus',['minus','1','5','0','0','period','5','0'])
w({'case':'C negative complete','ak':None,'al':None,'saved':None})
