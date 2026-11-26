#!/usr/bin/env python3
import sys,time,json,re
from pathlib import Path
sys.path.insert(0,'/home/ubuntu/forms-validation/money-93c-proof')
import bisect_driver,drive_fault2 as d
OUT=Path('/home/ubuntu/forms-validation/money-30ada-proof')
LOG=OUT/'run-unchanged.log'
ROWS=OUT/'matrix.jsonl'
TRDIR=OUT/'traces';TRDIR.mkdir(exist_ok=True)
ROWS.write_text('')
if not bisect_driver.ensure2550():raise RuntimeError('2550M not reachable')
n=0
for delay in (0,120,300,1000):
 for run in range(1,11):
  n+=1
  start=len(LOG.read_text(errors='ignore').splitlines()) if LOG.exists() else 0
  row=bisect_driver.one(delay,run,100000+n*1000,f'linux-reselect-{delay}')
  lines=LOG.read_text(errors='ignore').splitlines()[start:]
  tr=[x for x in lines if 'MONEYTRACE' in x]
  (TRDIR/f'{delay}-{run}.txt').write_text('\n'.join(tr)+'\n')
  row['trace_file']=str(TRDIR/f'{delay}-{run}.txt')
  # measurements: first outside write, following focus, first collapsed caret at end before a reselect/key
  ev=[]
  for line in tr:
   m=re.match(r'.*?MONEYTRACE (\d+) \S+ (\S+) (.*)',line)
   if m:
    try: ev.append((int(m.group(1)),m.group(2),json.loads(m.group(3))))
    except: pass
  ow=next((e for e in ev if e[1]=='outside-write'),None)
  foc=next((e for e in ev if ow and e[1]=='focus-callback' and e[0]>=ow[0]),None)
  col=None
  if foc:
   for e in ev:
    if e[0]>=foc[0] and e[1]=='selection':
     z=e[2];L=z.get('len')
     if L and z.get('start')==L and z.get('end')==L:
      col=e;break
  if ow and foc:row['write_to_focus_ms']=foc[0]-ow[0]
  if foc and col:row['focus_to_collapse_ms']=col[0]-foc[0]
  row['reselect']=any(e[1]=='reselect' for e in ev)
  with ROWS.open('a') as f:f.write(json.dumps(row)+'\n')
  print(json.dumps(row),flush=True)
print('done')
