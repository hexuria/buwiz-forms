import drive_rdo as d, subprocess, os, time, json
FORMS=['2551Q','0619E','1701Q','2550Q']
env=os.environ.copy()
def ag(*a):
 p=subprocess.run(['gpui-agent','--addr','127.0.0.1:17432',*a],env=env,capture_output=True,text=True,timeout=25)
 return p.returncode,p.stdout+p.stderr
out={}
for form in FORMS:
 rec={}
 for id in ['sb-new',f'new-return-{form}','new-return-open']:
  rc,s=ag('click',id); time.sleep(.65)
 time.sleep(1.6)
 rec['formpage']=bool(d.maybe('form-page'))
 # expand collapsed parts; refresh each pass
 for rounds in range(3):
  clicked=False
  for n in d.nodes():
   if n.get('id','').startswith('fp-part-') and n.get('id','').endswith('-toggle'):
    b=n.get('bounds') or {}
    if b.get('h',999) <= 60:
     d.click_node(n['id'],wait=.35); clicked=True
  if not clicked: break
 time.sleep(.5)
 rdo=[]
 for n in d.nodes():
  if 'txtRDO' in n.get('id','') or (n.get('id','').startswith('fp-label-') and 'RDO' in (n.get('name') or '')):
   rdo.append({k:n.get(k) for k in ['id','role','name','value','enabled','bounds']})
 rec['rdo_nodes']=rdo
 trig=next((x['id'] for x in rdo if x['id'] and x['id'].endswith('-trigger')),None)
 rec['trigger']=trig
 rec['shot_closed']=d.shot(f'{form.lower()}-nordo-rdo-closed')
 if trig:
  d.click_node(trig,wait=.7)
  opts=[]
  for n in d.nodes():
   if '-opt-' in n.get('id','') or n.get('role') in ('option','listbox'):
    opts.append({k:n.get(k) for k in ['id','role','name','value','bounds']})
  rec['list_nodes']=opts
  rec['option_texts']=[x.get('name') for x in opts if x.get('role')=='option']
  rec['option_count']=len(rec['option_texts'])
  rec['shot_list']=d.shot(f'{form.lower()}-nordo-rdo-list')
  subprocess.run(['xdotool','key','Escape'])
 out[form]=rec
print(json.dumps(out,indent=2))
open('/home/ubuntu/forms-validation/rdo-box-proof/other-forms-results.json','w').write(json.dumps(out,indent=2))
