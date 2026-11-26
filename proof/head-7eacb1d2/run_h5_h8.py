#!/usr/bin/env python3
import json, os, sqlite3, subprocess, sys, time
from pathlib import Path
sys.path.insert(0,'/home/ubuntu/forms-validation/money-focus-proof')
import drive_focus as d, run_focus_cases as r
OUT=Path('/home/ubuntu/forms-validation/money-focus-proof')
RES=OUT/'h5-h8-results.jsonl'
TIN='00012345300000'
AMT='fp-txtATCAmt1'
PCT='fp-frm1702RT:Pg2Pt4I40IncomeTaxRate'
INT='fp-frm2551Qv2018:txtSheets'

def write(row):
 row={'t':time.time(),**row}
 with RES.open('a') as f:f.write(json.dumps(row,ensure_ascii=False)+'\n')
 print(json.dumps(row,ensure_ascii=False),flush=True)

def latest(form):
 con=sqlite3.connect(str(d.DB),timeout=2)
 row=con.execute("select id,taxable_year,quarter,data_json from form_drafts where tin=? and form_code=? order by id desc limit 1",(TIN,form)).fetchone()
 con.close()
 if not row:return None
 return {'id':row[0],'year':row[1],'quarter':row[2],'data':json.loads(row[3])}

def path(obj,path):
 cur=obj
 for part in path.split('.'):
  if '[' in part:
   name,idx=part[:-1].split('[');
   if not isinstance(cur,dict):return None
   cur=cur.get(name)
   try:cur=cur[int(idx)]
   except Exception:return None
  else:
   if not isinstance(cur,dict):return None
   cur=cur.get(part)
 return cur

def saved(form,pathstr):
 r=latest(form)
 return path(r['data'],pathstr) if r else None

def msgs():
 return [n.get('name') for n in d.nodes() if n.get('name') and ('box shows' in n.get('name') or 'draft stored' in n.get('name') or 'type an amount' in n.get('name') or 'valid amount' in n.get('name').lower())]

def finish(case,keys,fid,form,pathstr,prefix):
 time.sleep(.15)
 ak=d.value(fid) if d.maybe(fid) else None
 sk=d.shot(prefix+'-after-keys')
 d.tab();time.sleep(1.1)
 al=d.value(fid) if d.maybe(fid) else None
 sv=saved(form,pathstr)
 sl=d.shot(prefix+'-after-leaving')
 row={'case':case,'keys_action':keys,'visible_after_keys':ak,'visible_after_leaving':al,'saved_draft_value':sv,'messages':msgs(),'shot_after_keys':sk,'shot_after_leaving':sl,'raw_node':d.maybe(fid)}
 write(row);return row

def click_type(fid,text,delay=120):
 d.click_node(fid,wait=.25);d.type_text(text,delay=delay)

def reset_field(fid,text='0'):
 d.click_node(fid,wait=.22);d.key(['ctrl+a']);d.type_text(text,delay=120);d.tab();time.sleep(1.0)

def tab_into(fid):
 # click the field, move one stop forward/back then return by keyboard; initial click is only setup for keyboard entry.
 d.click_node(fid,wait=.2);d.key(['shift+Tab']);time.sleep(.2);d.key(['Tab']);time.sleep(.15)

def paste(text):
 subprocess.run(['xclip','-selection','clipboard'],input=text.encode(),env={**os.environ,'DISPLAY':':0'},check=True)
 time.sleep(.1);d.key(['ctrl+v'])

def money_zero():
 # Assumes 2551Q schedule row drawer is open.
 reset_field(AMT,'0')

def open_recent(form):
 try:d.click_node('nav-home',wait=1)
 except Exception:pass
 rid=f'recent-{TIN}-{form}-2026-' + ('4' if form in ('2551Q','1601FQ') else '11')
 if d.maybe(rid):d.click_node(rid,wait=1.3);return True
 return False
