#!/usr/bin/env python3
import sys,time,json,subprocess,os,re
from pathlib import Path
sys.path.insert(0,'/home/ubuntu/forms-validation/money-93c-proof')
import drive_fault2 as d,bisect_driver
OUT=Path('/home/ubuntu/forms-validation/money-30ada-proof')
RES=OUT/'nothing-else-results.jsonl';RES.write_text('')
A='fp-frm2550m:txtTax13A';B='fp-frm2550m:txtTax13B'
def write(x):
 with RES.open('a') as f:f.write(json.dumps(x)+'\n')
 print(json.dumps(x),flush=True)
def val(x):
 try:return d.value(x)
 except Exception:return None
def shot(name):return d.shot(str(OUT/name))
def read():
 try:
  z=bisect_driver.read2550();return z.get('sales_to_government'),z.get('output_tax_government')
 except Exception:return None,None
def set_a(text):
 d.click_node(A,wait=.15);d.key(['ctrl+a']);d.type_text(text,delay=80);d.key(['Tab']);time.sleep(1.0)
def tab_into_a():
 # Focus A once, move one focus step back, then enter A by forward Tab.
 d.click_node(A,wait=.12);d.key(['shift+Tab']);time.sleep(.12);t0=time.time();d.key(['Tab']);return t0,time.time()
def xchain(*args):
 e=os.environ.copy();e['DISPLAY']=':0'
 return subprocess.run(['xdotool']+list(args),env=e,capture_output=True,text=True,timeout=10)
if not bisect_driver.ensure2550():raise RuntimeError('not reachable')
# a: filled not-recomputed 13A, wait 1 s after Tab, End then 9.
set_a('1234')
t0,t1=tab_into_a();time.sleep(1.0);d.key(['End']);time.sleep(.08);d.type_text('9',delay=80);time.sleep(.25)
ak=val(A);sa,sb=read();sh=shot('check-a-after-end-9')
write({'check':'a filled not recomputed wait1s End then 9','after_keys':ak,'saved_13a':sa,'expected_append':'12349','pass':ak=='12349','shot':sh})
# leave to settle
d.key(['Tab']);time.sleep(.7)
# b: same entry but End and 9 chained immediately after Tab.
set_a('1234')
d.click_node(A,wait=.12);d.key(['shift+Tab']);time.sleep(.12)
t0=time.time();xchain('key','Tab','key','End','key','9');t1=time.time();time.sleep(.3)
ak=val(A);sa,sb=read();sh=shot('check-b-fast-end-9')
write({'check':'b filled not recomputed End within 100ms then 9','chain_ms':round((t1-t0)*1000,1),'after_keys':ak,'saved_13a':sa,'expected_append':'12349','pass':ak=='12349','shot':sh})
d.key(['Tab']);time.sleep(.7)
# c: click 1234 between digits 2 and 3, type 9.
set_a('1234');d.key(['Tab']);time.sleep(.5)
n=d.find(A);x,y,w,h,wid=d.window_geom();b=n['bounds']
# text is right aligned; this x is approximately between 2 and 3 in a four digit amount.
sx=round(x+b['x']+b['w']-24);sy=round(y+b['y']+b['h']/2)
d.run(['xdotool','windowactivate',str(wid),'mousemove',str(sx),str(sy),'click','1'],check=False);time.sleep(.25)
d.type_text('9',delay=80);time.sleep(.25)
ak=val(A);sa,sb=read();sh=shot('check-c-click-middle-9')
write({'check':'c click filled amount between digits then 9','click_x':sx,'after_keys':ak,'saved_13a':sa,'expected_if_between_2_and_3':'12934','pass':ak=='12934','shot':sh})
d.key(['Tab']);time.sleep(.7)
# d: recompute 13B, click its right end within 100 ms after outside-write trace, type 9.
set_a('200000')
# Arm: type a changed base and leave by Tab, then watch trace file for the recomputed outside write.
d.click_node(A,wait=.12);d.key(['ctrl+a']);d.type_text('201000',delay=50);d.key(['Tab'])
log=OUT/'run-unchanged.log';start=len(log.read_text(errors='ignore').splitlines());found=None;tstart=time.time()
while time.time()-tstart<2:
 lines=log.read_text(errors='ignore').splitlines()[start:]
 for line in lines:
  if 'MONEYTRACE' in line and 'outside-write' in line:
   m=re.search(r'MONEYTRACE (\d+)',line);found=(int(m.group(1)),line);break
 if found:break
 time.sleep(.005)
click_delay=None
if found:
 n=d.find(B);x,y,w,h,wid=d.window_geom();bb=n['bounds'];sx=round(x+bb['x']+bb['w']-10);sy=round(y+bb['y']+bb['h']/2)
 d.run(['xdotool','windowactivate',str(wid),'mousemove',str(sx),str(sy),'click','1'],check=False)
 click_delay=round(time.time()*1000-found[0],1)
time.sleep(.15);d.type_text('9',delay=80);time.sleep(.3)
bk=val(B);sa,sb=read();sh=shot('check-d-click-recomputed-end-9')
write({'check':'d click just-recomputed 13B right end within 100ms then 9','click_after_write_ms':click_delay,'after_keys':bk,'saved_13b':sb,'trace':found and found[1].strip(),'shot':sh})
d.key(['Tab']);time.sleep(.7)
# e: zero box entered by Tab, type 42.
set_a('0')
d.click_node(A,wait=.12);d.key(['shift+Tab']);time.sleep(.12);d.key(['Tab']);time.sleep(.15);d.type_text('42',delay=80);time.sleep(.3)
ak=val(A);sa,sb=read();sh=shot('check-e-zero-42')
write({'check':'e Tab zero box type 42','after_keys':ak,'saved_13a':sa,'expected':'42','pass':ak=='42','shot':sh})
d.key(['Tab']);time.sleep(.7)
print('done')
