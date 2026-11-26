import sys,time,json,sqlite3,subprocess,os
sys.path.insert(0,'/home/ubuntu/forms-validation/money-accept-proof')
import drive_accept as d
OUT=d.OUT;RES=OUT/'fgc-results.jsonl';A='fp-frm2550m:txtTax13A';B='fp-frm2550m:txtTax13B';env={**os.environ,'DISPLAY':':0'}
def w(x):open(RES,'a').write(json.dumps(x,ensure_ascii=False)+'\n');print(x['case'],x['ak'],x['al'],x['saved'],flush=True)
def data():
 c=sqlite3.connect(str(d.DB));z=json.loads(c.execute('select data_json from form_drafts where id=2').fetchone()[0]);c.close();return z
def msgs():return [n.get('name') for n in d.nodes() if n.get('id','').startswith('fp-boxmsg') or 'not an amount' in (n.get('name') or '') or 'you typed' in (n.get('name') or '')]
def ensure():
 if not d.maybe(B):
  if d.maybe('fp-nav-2'):d.click_node('fp-nav-2',wait=.8)
 return d.maybe(A) and d.maybe(B)
def run(mode,idx,base):
 assert ensure()
 # Setup only: put a fresh base in 13A with explicit select-all. The tested input starts at the real Tab key below.
 d.click_node(A,wait=.2);d.key(['ctrl+a']);d.type_text(str(base),delay=120)
 before=d.value(B) if d.maybe(B) else None
 delay='120' if mode=='120ms' else '0'
 subprocess.run(['xdotool','key','--delay',delay,'Tab','9','9','9'],env=env)
 time.sleep(.35)
 ak=d.value(B) if d.maybe(B) else None;fb=d.find(B).get('focused') if d.maybe(B) else None;ks=d.shot('f-'+mode+'-'+str(idx)+'-keys')
 d.key(['Tab']);time.sleep(1.25)
 al=d.value(B) if d.maybe(B) else None;dd=data();sv=dd.get('output_tax_government');base_saved=dd.get('sales_to_government');ls=d.shot('f-'+mode+'-'+str(idx)+'-leave')
 w({'case':'F 2550M 13A->13B '+mode+' run'+str(idx),'base':base,'before_recomputed':before,'ak':ak,'al':al,'saved':sv,'base_saved':base_saved,'sentence':msgs(),'focus_after_keys':fb,'shotk':ks,'shotl':ls})
open(RES,'w').close()
for i,b in enumerate([1000,2000,3000,4000,5000],1):run('120ms',i,b)
for i,b in enumerate([1100,2200,3300,4400,5500],1):run('nodelay',i,b)
w({'case':'F complete','ak':None,'al':None,'saved':None})
