import sys,time,json,sqlite3,subprocess,os
sys.path.insert(0,'/home/ubuntu/forms-validation/money-accept-proof')
import drive_accept as d, run_accept_acd as r
OUT=d.OUT;RES=OUT/'be-results.jsonl';TIN='00012345300000';AMT='fp-txtATCAmt1';INT='fp-frm2551Qv2018:txtSheets';RATE='fp-txtRate0'
def w(x):
 x['t']=time.time();open(RES,'a').write(json.dumps(x,ensure_ascii=False)+'\n');print(x['case'],x.get('result'),x.get('ak'),x.get('al'),x.get('saved'),flush=True)
def rowf(form):
 c=sqlite3.connect(str(d.DB));z=c.execute('select data_json from form_drafts where tin=? and form_code=? order by id desc limit 1',(TIN,form)).fetchone();c.close();return json.loads(z[0]) if z else {}
def getp(o,p):
 for q in p.split('.'):
  if '[' in q:
   a,i=q[:-1].split('[');o=o.get(a,{});o=o[int(i)] if isinstance(o,list) else None
  else:o=o.get(q) if isinstance(o,dict) else None
 return o
def msgs():return [n.get('name') for n in d.nodes() if n.get('id','').startswith('fp-boxmsg') or 'is not an amount' in (n.get('name') or '')]
def fin(case,fid,form,path,prefix):
 time.sleep(.15);ak=d.value(fid) if d.maybe(fid) else None;ks=d.shot(prefix+'-keys');d.key(['Tab']);time.sleep(1.1);al=d.value(fid) if d.maybe(fid) else None;sv=getp(rowf(form),path);ls=d.shot(prefix+'-leave');w({'case':case,'ak':ak,'al':al,'saved':sv,'sentence':msgs(),'shotk':ks,'shotl':ls})
def openret(n):
 if d.maybe('nav-home'):d.click_node('nav-home',wait=.8)
 if d.maybe('home-tile-drafts'):d.click_node('home-tile-drafts',wait=.8)
 if d.maybe('return-'+str(n)):d.click_node('return-'+str(n),wait=1.5)
 if d.maybe('fp-parts-toggle'):d.click_node('fp-parts-toggle',wait=.6)
def typecase(text,prefix,delay=120,paste=False):
 r.set0();d.click_node(AMT,wait=.2)
 if paste:
  subprocess.run(['xclip','-selection','clipboard'],input=text.encode(),env={**os.environ,'DISPLAY':':0'});time.sleep(.1);d.key(['ctrl+v'])
 elif delay==0:subprocess.run(['xdotool','key','--delay','0']+[d.keyname(c) for c in text],env={**os.environ,'DISPLAY':':0'})
 else:d.type_text(text,delay=delay)
 fin('B '+prefix+' '+text,AMT,'2551Q','schedule_1[0].taxable_amount','b-'+prefix)
# B
openret(1)
if not d.maybe(AMT) and d.maybe('fp-row-drpATC1-0'):d.click_node('fp-row-drpATC1-0',wait=.8)
r.ensure(AMT)
typecase('99999.99','99999')
typecase('1000000.005','million005')
# edit inversion
r.set0();d.click_node(AMT,wait=.2);d.type_text('1234567890',delay=120);d.key(['Left','Left','BackSpace'],delay=120);fin('B edit-inversion 1234567890 LL BS',AMT,'2551Q','schedule_1[0].taxable_amount','b-inversion')
for i in range(1,4):typecase('99999999.99','big'+str(i))
typecase('99999999.99','big-nodelay',0)
typecase('99999999.99','big-paste',paste=True)
# E int in same form
r.ensure(INT)
for mode in ['click','tab']:
 d.click_node(INT,wait=.2);d.key(['ctrl+a']);d.type_text('0',delay=120);d.key(['Tab']);time.sleep(.8)
 if mode=='click':d.click_node(INT,wait=.2)
 else:d.click_node(INT,wait=.2);d.key(['shift+Tab']);time.sleep(.15);d.key(['Tab']);time.sleep(.15)
 d.type_text('42',delay=120);fin('E sheets '+mode+' 42',INT,'2551Q','number_of_attached_sheets','e-sheets-'+mode)
# E rate 1601FQ return 3
openret(3);r.ensure(RATE)
for mode in ['click','tab']:
 d.click_node(RATE,wait=.2);d.key(['ctrl+a']);d.type_text('0',delay=120);d.key(['Tab']);time.sleep(.8)
 if mode=='click':d.click_node(RATE,wait=.2)
 else:d.click_node(RATE,wait=.2);d.key(['shift+Tab']);time.sleep(.15);d.key(['Tab']);time.sleep(.15)
 d.type_text('42',delay=120);fin('E 1601FQ rate '+mode+' 42',RATE,'1601FQ','schedule_1[0].tax_rate','e-rate-'+mode)
w({'case':'complete'})
