#!/usr/bin/env python3
import json, os, re, sqlite3, subprocess, sys, time
from pathlib import Path
sys.path.insert(0,'/home/ubuntu/forms-validation/money-accept-proof')
import drive_accept as d
OUT=Path('/home/ubuntu/forms-validation/money-accept-proof'); RESULTS=OUT/'accept-acd-results.jsonl'
TIN='00012345300000'; YEAR=2026; QUARTER=4
AMT='fp-txtATCAmt1'; SUR='fp-frm2551Qv2018:txt20'

def env(): return {**os.environ,'GPUI_AGENT_TOKEN':'acceptlocal','DISPLAY':':0'}
def write(row):
 row=dict(row);row['t']=time.time()
 with RESULTS.open('a') as f:f.write(json.dumps(row,ensure_ascii=False)+'\n')
 print(json.dumps(row,ensure_ascii=False),flush=True)
def keychars(text):return [d.keyname(c) for c in text]
def xchain(*args):return d.run(['xdotool']+list(args),timeout=20)
def click_type_burst(node_id,text):
 n=d.find(node_id);sx,sy,wid,logical=d.screen_point(n);d.run(['xdotool','windowactivate',str(wid)],check=False)
 xchain('mousemove',str(sx),str(sy),'click','1','key','--delay','0',*keychars(text))
def value(node):
 try:return d.value(node)
 except Exception:return None
def nodeinfo(node):
 try:return d.find(node)
 except Exception:return {}
def draft(path):return d.draft_path('2551Q',path,YEAR,QUARTER)
def box_messages():
 return [(n.get('id'),n.get('name')) for n in d.nodes() if n.get('id','').startswith('fp-boxmsg') or 'is not an amount' in (n.get('name') or '') or 'digits' in (n.get('name') or '').lower()]
def amt_msgs():return [x for x in box_messages() if 'txtatcamt1' in (x[0] or '').lower() or 'amount' in (x[1] or '').lower() or 'digits' in (x[1] or '').lower()]
def finish(case,keys,fid=AMT,path='schedule_1[0].taxable_amount',prefix='x',expect=None,extra=None):
 time.sleep(.15);ak=value(fid);sk=d.shot(prefix+'-after-keys')
 d.key(['Tab']);time.sleep(1.2);al=value(fid);saved=draft(path);sl=d.shot(prefix+'-after-leaving')
 row={'case':case,'keys_action':keys,'visible_after_keys':ak,'visible_after_leaving':al,'saved_draft_value':saved,'sentence_text':amt_msgs(),'shot_after_keys':sk,'shot_after_leaving':sl,'raw_node':nodeinfo(fid)}
 if expect:row['result']='RIGHT' if ak==expect.get('ak') and al==expect.get('al') and (expect.get('saved') is None or saved==expect.get('saved')) else 'FAULT'
 if extra:row.update(extra)
 write(row);return row
def scroll(step):
 subprocess.run(['gpui-agent','--addr',d.ADDR,'--reply-timeout-ms','10000','scroll','form-scroll','--dy',str(step)],capture_output=True,env=env(),timeout=5);time.sleep(.25)
def ensure(fid,rounds=35):
 for _ in range(rounds):
  if d.maybe(fid):return True
  scroll(500)
 for _ in range(10):
  if d.maybe(fid):return True
  scroll(-500)
 return bool(d.maybe(fid))
def expand_all():
 n=d.maybe('fp-parts-toggle')
 if n and 'Collapse' not in (n.get('name') or ''):
  try:d.click_node('fp-parts-toggle',wait=.8)
  except Exception:pass
def focus_prev_tab():
 try:
  d.click_node('fp-drpATC1-trigger',wait=.3);d.key(['Escape']);time.sleep(.2)
 except Exception:
  d.click_node(AMT,wait=.2);d.key(['shift+Tab']);time.sleep(.2)
 d.key(['Tab']);time.sleep(.18)
def focus_next_shift():
 d.click_node(AMT,wait=.2);d.key(['Tab']);time.sleep(.22);d.key(['shift+Tab']);time.sleep(.18)
def set_amount_tab(text):
 ensure(AMT);focus_prev_tab();d.type_text(str(text),delay=120);d.key(['Tab']);time.sleep(1.15)
def set123():set_amount_tab('1234.56')
def set0():set_amount_tab('0')
def open_existing():
 # open the existing 2551Q draft through the visible drafts list unless already open
 if d.maybe('fp-parts-toggle'):
  pass
 elif d.maybe('home-tile-drafts'):d.click_node('home-tile-drafts',wait=1)
 if d.maybe('return-1'):d.click_node('return-1',wait=1.8)
 else:
  rid='recent-00012345300000-2551Q-2026-4'
  if d.maybe(rid):d.click_node(rid,wait=1.8)
  else:
   for id in ['sb-new','new-return-2551Q','new-return-open']:
    if d.maybe(id):d.click_node(id,wait=.8)
 time.sleep(1);expand_all()
 # open Schedule 1 row 1 drawer when the table is collapsed
 if not d.maybe(AMT) and d.maybe('fp-row-drpATC1-0'):d.click_node('fp-row-drpATC1-0',wait=.9)
 ensure(AMT)
def paste_text(text):
 subprocess.run(['xclip','-selection','clipboard'],input=text.encode(),env=env(),check=True)
 time.sleep(.1);d.key(['ctrl+v'])

def test_A():
 # initial DB row starts 0; use Tab entry to establish 1,234.56, then test Tab itself on next pass
 set123()
 focus_prev_tab();d.type_text('15000.50',delay=120)
 finish('A entry Tab into 1,234.56','Tab then type 15000.50 @120ms',prefix='a-tab',expect={'ak':'15000.50','al':'15,000.50','saved':15000.5})
 set123();focus_next_shift();d.type_text('15000.50',delay=120)
 finish('A entry Shift+Tab into 1,234.56','Shift+Tab then type 15000.50 @120ms',prefix='a-shifttab',expect={'ak':'15000.50','al':'15,000.50','saved':15000.5})
 set123();d.click_node(AMT,wait=.25);d.key(['ctrl+a']);d.type_text('15000.50',delay=120)
 finish('A click Ctrl+A type','click Ctrl+A type 15000.50 @120ms',prefix='a-click-ctrla',expect={'ak':'15000.50','al':'15,000.50','saved':15000.5})
 set123();d.click_node(AMT,wait=.25);d.key(['ctrl+a']);d.key(['BackSpace']);d.type_text('15000.50',delay=120)
 finish('A click Ctrl+A BackSpace type','click Ctrl+A BackSpace type 15000.50 @120ms',prefix='a-click-backspace',expect={'ak':'15000.50','al':'15,000.50','saved':15000.5})
 set123();d.click_node(AMT,wait=.25);d.type_text('15000.50',delay=120)
 r=finish('A one click then type','one click then type 15000.50 @120ms',prefix='a-oneclick')
 r['caret_inference']='replaced/all selected' if r['visible_after_keys']=='15000.50' else 'not selected or caret edit; exact visible text shows outcome';write({'case':'A one click inference','inference':r['caret_inference']})
 # zero box ordinary click and Tab
 set0();d.click_node(AMT,wait=.25);d.type_text('15000.50',delay=120)
 finish('A zero click type','zero box click then type 15000.50 @120ms',prefix='a-zero-click',expect={'ak':'15000.50','al':'15,000.50','saved':15000.5})
 set0();focus_prev_tab();d.type_text('15000.50',delay=120)
 finish('A zero Tab type','zero box Tab then type 15000.50 @120ms',prefix='a-zero-tab',expect={'ak':'15000.50','al':'15,000.50','saved':15000.5})
 for i in range(1,11):
  set0();click_type_burst(AMT,'15000.50')
  finish(f'A zero click first-key-at-once run {i}','click and type 15000.50 in one xdotool process delay0',prefix=f'a-zero-burst-{i}',expect={'ak':'15000.50','al':'15,000.50','saved':15000.5})

def unreadable(text,prefix,delay=120):
 set123();d.click_node(AMT,wait=.2);d.key(['ctrl+a'])
 if delay==0:
  # no-delay key burst after focus/caret setup
  xchain('key','--delay','0',*keychars(text))
 else:d.type_text(text,delay=delay)
 r=finish('C unreadable '+repr(text),f'Ctrl+A type {text!r} delay {delay}, Tab away',prefix=prefix)
 # click again and see whether sentence disappears
 d.click_node(AMT,wait=.5);time.sleep(.4)
 write({'case':'C click again clears sentence '+repr(text),'sentence_text_after_click':amt_msgs(),'shot':d.shot(prefix+'-click-again')})

def test_C():
 for text,delay,prefix in [('1500,50',120,'c-comma-decimal'),('15000..50',0,'c-two-dots'),('150a00.50',0,'c-letterburst'),('12a3',120,'c-12a3'),('1.234,56',120,'c-euro'),('1,2,3',120,'c-commas'),(',',120,'c-lone-comma')]:unreadable(text,prefix,delay)
 # paste cases into zero
 for text,prefix,expected in [('₱ 15,000.50','c-paste-peso','15,000.50'),(' 15000.50 ','c-paste-spaces','15,000.50'),('PHP 15000.50','c-paste-php','15,000.50'),('15000.50.25','c-paste-twodots',None)]:
  set0();d.click_node(AMT,wait=.2);paste_text(text)
  finish('C paste '+repr(text),f'real Ctrl+V paste {text!r}',prefix=prefix,expect=({'ak':expected,'al':expected,'saved':15000.5} if expected else None))
 # twenty digits
 set123();d.click_node(AMT,wait=.2);d.key(['ctrl+a']);d.type_text('12345678901234567890',delay=120)
 finish('C twenty digits','Ctrl+A type 20 digits @120ms, Tab away',prefix='c-twenty-digits')

def test_D():
 ensure(SUR)
 # surcharge was reset to exactly 1500 in scratch DB before launch
 # visible Check then to-do next until surcharge focused
 try:d.click_node('fp-check',wait=1.5)
 except Exception:pass
 steps=[]
 for i in range(5):
  try:d.click_node('fp-pill-next',wait=.9)
  except Exception as e:steps.append({'error':str(e)})
  fs=[]
  for n in d.nodes():
   if n.get('focused') or 'focused' in str(n.get('states')):fs.append({'id':n.get('id'),'name':n.get('name'),'value':n.get('value')})
  steps.append({'step':i+1,'focused':fs,'surcharge':nodeinfo(SUR)})
  if any(x.get('id')==SUR for x in fs):break
 d.type_text('2500',delay=120)
 r=finish('D form focus surcharge type 2500','Check/to-do focus into filled 1,500.00 then type 2500 @120ms',fid=SUR,path='surcharge',prefix='d-form-focus',expect={'ak':'2500','al':'2,500.00','saved':2500.0},extra={'focus_steps':steps})

if __name__=='__main__':
 open_existing();write({'case':'setup','drafts':d.latest_drafts(),'shot':d.shot('accept-setup')})
 test_A();test_C();test_D();write({'case':'complete'})
