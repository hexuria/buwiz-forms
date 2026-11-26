import drive_rdo as d, subprocess, os, time, json, sqlite3
OUT=d.OUT; res={}
def ag(*a,timeout=20):
 return subprocess.run(['gpui-agent','--addr',d.ADDR,'--reply-timeout-ms','10000',*a],env=os.environ,capture_output=True,text=True,timeout=timeout)
def invoke(name,**kw):
 args=[]
 for k,v in kw.items(): args += ['--arg',f'{k}={json.dumps(v)}']
 p=ag('invoke',name,*args,timeout=30)
 try: out=json.loads(p.stdout)
 except: out={'raw':p.stdout,'err':p.stderr,'rc':p.returncode}
 return out,p
def click(id,w=.55):
 # real coordinate clicks: semantic clicks can report success without activating these rows
 d.click_node(id,wait=w)
def openform(code,period):
 for id in ['sb-new',f'new-return-{code}','new-return-open']: click(id,.65)
 time.sleep(1.5); return {'ui_dialog':True}
def expand():
 for n in list(d.nodes()):
  id=n.get('id','')
  if id.startswith('fp-part-') and id.endswith('-toggle'):
   base=id[:-7]
   container=d.maybe(base)
   if container and (container.get('bounds') or {}).get('h',999)<=60: d.click_node(id,wait=.3)
 time.sleep(.7)
def scrollfind(id,rounds=30):
 for _ in range(rounds):
  if d.maybe(id): return True
  ag('scroll','form-scroll','--dy','500');time.sleep(.25)
 return bool(d.maybe(id))
def vals(msgname):
 click('fp-check',1.2)
 return {'messages':[(n.get('id'),n.get('name')) for n in d.nodes() if n.get('id','').startswith('fp-boxmsg-') or 'items need attention' in (n.get('name') or '')],'shot':d.shot(msgname)}
def catalog(key,name,pick=False):
 trig=key+'-trigger'; ok=scrollfind(trig); click(trig,.9)
 ns=d.nodes(); codes=sorted(set((n.get('id','')[len(key)+5:] for n in ns if n.get('id','').startswith(key+'-opt-'))))
 texts=[n.get('name') or '' for n in ns]
 r={'trigger_found':ok,'semantic_visible_count':len(codes),'codes_visible_first':codes[:5],'codes_visible_last':codes[-5:],'has000':'000' in codes,'template_text':any('rdo.description' in x for x in texts),'shot_open':d.shot(name+'-open')}
 if d.maybe(key+'-search'):
  d.click_node(key+'-search',wait=.2);d.key(['ctrl+a']);d.type_text('000',delay=30);time.sleep(.5)
  r['search000_none']=any('Nothing matches' in (n.get('name') or '') for n in d.nodes());r['shot000']=d.shot(name+'-search000')
  d.key(['ctrl+a']);d.type_text('132',delay=30);time.sleep(.5);r['search132_found']=bool(d.maybe(key+'-opt-132'))
  d.key(['ctrl+a']);d.type_text('001',delay=30);time.sleep(.5);r['search001_found']=bool(d.maybe(key+'-opt-001'))
  d.key(['ctrl+a']);d.type_text('040',delay=30);time.sleep(.5);r['search040_found']=bool(d.maybe(key+'-opt-040'))
  if pick and d.maybe(key+'-opt-040'): click(key+'-opt-040',.8);r['picked_value']=d.value(trig) if d.maybe(trig) else None
 else: r['no_search']=True
 return r
# main 1601C
res['open1601']=openform('1601C',11);expand();key='fp-frm1601c:txtRDOCode';scrollfind(key+'-trigger')
res['1601_blank_validation']=vals('1601c-blank-validation')
res['1601_catalog']=catalog(key,'1601c-catalog',pick=True)
res['1601_040_validation']=vals('1601c-040-validation')
res['1601_picked_shot']=d.shot('1601c-040-picked')
click('nav-home',.8);rid='recent-00011124500000-1601C-2026-11'
if d.maybe(rid):
 click(rid,1.5);expand();scrollfind(key+'-trigger');res['1601_reopen_value']=d.value(key+'-trigger') if d.maybe(key+'-trigger') else None;res['1601_reopen_shot']=d.shot('1601c-040-reopen')
# profile db proof
try:
 db=sqlite3.connect(str(OUT/'rdo-deef-validation.db')); row=db.execute("select json_extract(data_json,'$.rdo_code') from profiles where tin='00011124500000'").fetchone();res['profile_rdo_after']=row[0] if row else 'missing'
except Exception as e: res['profile_rdo_after_error']=str(e)
res['profile_shot']=d.shot('profile-after-1601c')
# 1701 filer
res['open1701']=openform('1701Q',4);expand();key2='fp-frm1701q:txtRDOCode';res['1701_filer_catalog']=catalog(key2,'1701q-filer-catalog',pick=False)
# spouse scan, no pick
key3='fp-frm1701q:txtSpouseRDOCode';res['1701_spouse_found']=scrollfind(key3+'-trigger',35)
if res['1701_spouse_found']: res['1701_spouse_catalog']=catalog(key3,'1701q-spouse-catalog',pick=False)
print(json.dumps(res,indent=2));open(OUT/'results.json','w').write(json.dumps(res,indent=2))
