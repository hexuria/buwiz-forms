import sys,time,json,sqlite3,os
sys.path.insert(0,'/home/ubuntu/forms-validation/money-accept-proof')
import drive_accept as d
RES=d.OUT/'e-results.jsonl';TIN='00012345300000';RATE='fp-txtRate0'
def w(x):open(RES,'a').write(json.dumps(x,ensure_ascii=False)+'\n');print(x['case'],x.get('a4'),x.get('ak'),x.get('al'),x.get('saved'),flush=True)
def draft():
 c=sqlite3.connect(str(d.DB));z=c.execute("select data_json from form_drafts where tin=? and form_code='1601FQ' order by id desc limit 1",(TIN,)).fetchone();c.close();return json.loads(z[0])
def sv():return draft()['schedule1'][0].get('tax_rate')
def msgs():return [n.get('name') for n in d.nodes() if n.get('id','').startswith('fp-boxmsg') or 'not an amount' in (n.get('name') or '')]
def reset0():
 d.click_node(RATE,wait=.25);d.key(['ctrl+a']);d.type_text('0',delay=120);d.key(['Tab']);time.sleep(1)
 return sv()
def focus_mode(mode):
 if mode=='click':d.click_node(RATE,wait=.25)
 else:
  d.click_node(RATE,wait=.25);d.key(['shift+Tab']);time.sleep(.2);d.key(['Tab']);time.sleep(.2)
def case(mode):
 before=reset0()
 focus_mode(mode)
 d.type_text('4',delay=120);time.sleep(.15);a4=d.value(RATE) if d.maybe(RATE) else None;s4=d.shot('e2-rate-'+mode+'-after4')
 d.type_text('2',delay=120);time.sleep(.15);ak=d.value(RATE) if d.maybe(RATE) else None;ks=d.shot('e2-rate-'+mode+'-keys')
 d.key(['Tab']);time.sleep(1.2);al=d.value(RATE) if d.maybe(RATE) else None;save=sv();ls=d.shot('e2-rate-'+mode+'-leave')
 w({'case':'E 1601FQ rate '+mode+' type42 corrected','setup_saved_zero':before,'a4':a4,'ak':ak,'al':al,'saved':save,'sentence':msgs(),'shot4':s4,'shotk':ks,'shotl':ls})
case('click');case('tab')
