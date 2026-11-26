import drive_rdo as d,time,json
r={}
def typ(id,text): d.click_node(id,wait=.2);d.key(['ctrl+a']);d.type_text(text,delay=20);time.sleep(.25)
typ('fp-frm1801v2018:sched4Name_1','Invented Business');typ('fp-frm1801v2018:sched4Address_1','Quezon City');typ('fp-frm1801v2018:sched4Exc_1','1000')
key='fp-frm1801v2018:sched4Rdo_1';d.click_node(key+'-trigger',wait=.7);ns=d.nodes();codes=sorted(set(n.get('id','')[len(key)+5:] for n in ns if n.get('id','').startswith(key+'-opt-')));r['picker']={'count_visible':len(codes),'first':codes[:3],'has000':'000' in codes,'shot':d.shot('1801-rdo-picker')};d.click_node(key+'-search',wait=.1);d.type_text('040',delay=20);time.sleep(.4);r['found040']=bool(d.maybe(key+'-opt-040'))
if d.maybe(key+'-opt-040'):d.click_node(key+'-opt-040',wait=.5)
r['picked']=d.value(key+'-trigger') if d.maybe(key+'-trigger') else None
if d.maybe('fp-table-frm1801v2018:sched4Name_1-done'):d.click_node('fp-table-frm1801v2018:sched4Name_1-done',wait=.8)
d.click_node('fp-check',wait=1)
r['messages']=[(n.get('id'),n.get('name')) for n in d.nodes() if 'RDO' in (n.get('name') or '') or 'Schedule 4' in (n.get('name') or '') or 'items need attention' in (n.get('name') or '')]
r['validation_shot']=d.shot('1801-040-validation')
print(json.dumps(r));open('/home/ubuntu/forms-validation/rdo-fix-deef-proof/1801-result.json','w').write(json.dumps(r,indent=2))
