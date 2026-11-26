import sys,time,json,sqlite3,os
sys.path.insert(0,'/home/ubuntu/forms-validation/money-accept-proof')
import drive_accept as d
RES=d.OUT/'e-results.jsonl';TIN='00012345300000';INT='fp-frm2551Qv2018:txtSheets';RATE='fp-txtRate0'
def w(x):open(RES,'a').write(json.dumps(x,ensure_ascii=False)+'\n');print(x['case'],x['ak'],x['al'],x['saved'],flush=True)
def data(form):
 c=sqlite3.connect(str(d.DB));z=c.execute('select data_json from form_drafts where tin=? and form_code=? order by id desc limit 1',(TIN,form)).fetchone();c.close();return json.loads(z[0])
def getp(o,p):
 for q in p.split('.'):
  if '[' in q:
   a,i=q[:-1].split('[');o=o.get(a,[]);o=o[int(i)] if isinstance(o,list) and len(o)>int(i) else None
  else:o=o.get(q) if isinstance(o,dict) else None
 return o
def msgs():return [n.get('name') for n in d.nodes() if n.get('id','').startswith('fp-boxmsg') or 'not an amount' in (n.get('name') or '')]
def fin(case,fid,form,path,prefix):
 time.sleep(.15);ak=d.value(fid) if d.maybe(fid) else None;ks=d.shot(prefix+'-keys');d.key(['Tab']);time.sleep(1.1);al=d.value(fid) if d.maybe(fid) else None;sv=getp(data(form),path);ls=d.shot(prefix+'-leave');w({'case':case,'ak':ak,'al':al,'saved':sv,'sentence':msgs(),'shotk':ks,'shotl':ls})
def reset(fid):
 d.click_node(fid,wait=.2);d.key(['ctrl+a']);d.type_text('0',delay=120);d.key(['Tab']);time.sleep(.9)
def sheetcase(mode):
 reset(INT)
 if mode=='click':d.click_node(INT,wait=.2)
 else:
  d.click_node(INT,wait=.2);d.key(['shift+Tab']);time.sleep(.2);d.key(['Tab']);time.sleep(.15)
 d.type_text('42',delay=120);fin('E sheets '+mode+' type42',INT,'2551Q','number_of_attached_sheets','e-sheets-'+mode)
def scroll(fid):
 import subprocess
 env={**os.environ,'GPUI_AGENT_TOKEN':'acceptlocal','DISPLAY':':0'}
 for z in range(35):
  if d.maybe(fid):return True
  subprocess.run(['gpui-agent','--addr',d.ADDR,'--reply-timeout-ms','10000','scroll','form-scroll','--dy','500'],capture_output=True,env=env);time.sleep(.25)
 return bool(d.maybe(fid))
def open1601():
 if d.maybe('nav-home'):d.click_node('nav-home',wait=.9)
 if d.maybe('home-tile-drafts'):d.click_node('home-tile-drafts',wait=.9)
 if d.maybe('return-3'):d.click_node('return-3',wait=1.6)
 if d.maybe('fp-parts-toggle'):d.click_node('fp-parts-toggle',wait=.6)
def ratecase(mode):
 scroll(RATE)
 if not d.maybe(RATE):w({'case':'E rate setup error','ak':None,'al':None,'saved':None});return
 reset(RATE)
 if mode=='click':d.click_node(RATE,wait=.2)
 else:
  d.click_node(RATE,wait=.2);d.key(['shift+Tab']);time.sleep(.2);d.key(['Tab']);time.sleep(.15)
 d.type_text('42',delay=120);fin('E 1601FQ rate '+mode+' type42',RATE,'1601FQ','schedule_1[0].tax_rate','e-rate-'+mode)
# sheets cases, current 2551Q return details already visible
sheetcase('click');sheetcase('tab')
open1601();ratecase('click');ratecase('tab')
w({'case':'complete','ak':None,'al':None,'saved':None})
