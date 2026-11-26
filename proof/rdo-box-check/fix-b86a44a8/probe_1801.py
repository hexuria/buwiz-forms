import drive_rdo as d,time,json,subprocess,os
subprocess.run(['xdotool','key','Escape']);time.sleep(.2)
if d.maybe('tb-panel') and 'Show' in (d.maybe('tb-panel').get('name') or ''): d.click_node('tb-panel',wait=.5)
if d.maybe('panel-tp-000111245'): d.click_node('panel-tp-000111245',wait=.8)
for id in ['sb-new','new-return-1801','new-return-open']: d.click_node(id,wait=.7)
time.sleep(1.5)
# expand all parts once
for n in list(d.nodes()):
 id=n.get('id','')
 if id.startswith('fp-part-') and id.endswith('-toggle'):
  base=id[:-7];c=d.maybe(base)
  if c and c['bounds']['h']<=60:d.click_node(id,wait=.2)
time.sleep(.6)
# find sched4 rdo/control and row fields
found=scroll=False
for _ in range(35):
 if d.maybe('fp-frm1801v2018:sched4Rdo_1') or d.maybe('fp-frm1801v2018:sched4Rdo_1-trigger'): found=True;break
 subprocess.run(['gpui-agent','--addr',d.ADDR,'scroll','form-scroll','--dy','600'],env=os.environ,capture_output=True);time.sleep(.2)
ns=d.nodes();rows=[]
for n in ns:
 if 'sched4' in n.get('id','').lower() and ('_1' in n.get('id','') or '_1-' in n.get('id','')): rows.append((n.get('id'),n.get('role'),n.get('name'),n.get('value'),n.get('enabled')))
r={'found':found,'row_nodes':rows[:30],'shot':d.shot('1801-sched4-row')}
print(json.dumps(r));open('/home/ubuntu/forms-validation/rdo-fix-deef-proof/1801-probe.json','w').write(json.dumps(r,indent=2))
