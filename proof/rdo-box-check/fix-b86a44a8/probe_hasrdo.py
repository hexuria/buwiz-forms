import drive_rdo as d,time,json,subprocess,os
subprocess.run(['xdotool','key','Escape']);time.sleep(.3)
if d.maybe('tb-panel') and 'Show' in (d.maybe('tb-panel').get('name') or ''): d.click_node('tb-panel',wait=.6)
if d.maybe('panel-tp-000123453'): d.click_node('panel-tp-000123453',wait=1)
for id in ['sb-new','new-return-1601C','new-return-open']: d.click_node(id,wait=.65)
time.sleep(1.3)
# expand part1 using generic container
if d.maybe('fp-part-1') and d.maybe('fp-part-1')['bounds']['h']<=60: d.click_node('fp-part-1-toggle',wait=.6)
# scroll until profile fold, expand, edit
for _ in range(15):
 if d.maybe('fp-profile-1'): break
 subprocess.run(['gpui-agent','--addr',d.ADDR,'scroll','form-scroll','--dy','450'],env=os.environ,capture_output=True);time.sleep(.25)
if d.maybe('fp-profile-1'): d.click_node('fp-profile-1',wait=.6)
r={'profile_row':bool(d.maybe('fp-frm1601c:txtRDOCode')),'shot_folded':d.shot('1601c-hasrdo-folded')}
if d.maybe('fp-edit-1:1'): d.click_node('fp-edit-1:1',wait=.8)
key='fp-frm1601c:txtRDOCode'
# edit may open list immediately; capture
ns=d.nodes();codes=sorted(set(n.get('id','')[len(key)+5:] for n in ns if n.get('id','').startswith(key+'-opt-')))
r.update(after_edit_shot=d.shot('1601c-hasrdo-edit-list'),visible_count=len(codes),first=codes[:3],has000='000' in codes,trigger=d.value(key+'-trigger') if d.maybe(key+'-trigger') else None)
print(json.dumps(r));open('/home/ubuntu/forms-validation/rdo-fix-deef-proof/hasrdo-result.json','w').write(json.dumps(r,indent=2))
