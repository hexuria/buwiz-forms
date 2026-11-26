import drive_rdo as d,time,json
key='fp-frm1701q:txtSpouseRDOCode';d.click_node(key+'-trigger',wait=.8);ns=d.nodes();codes=sorted(set(n.get('id','')[len(key)+5:] for n in ns if n.get('id','').startswith(key+'-opt-')));r={'count_visible':len(codes),'first':codes[:3],'last':codes[-3:],'has000':'000' in codes,'template':any('rdo.description' in (n.get('name') or '') for n in ns),'shot':d.shot('1701q-spouse-catalog-open')}
if d.maybe(key+'-search'):
 d.click_node(key+'-search',wait=.2);d.key(['ctrl+a']);d.type_text('000',delay=30);time.sleep(.5);r['search000_none']=any('Nothing matches' in (n.get('name') or '') for n in d.nodes());r['shot000']=d.shot('1701q-spouse-search000');d.key(['ctrl+a']);d.type_text('132',delay=30);time.sleep(.4);r['has132']=bool(d.maybe(key+'-opt-132'))
print(json.dumps(r));open('/home/ubuntu/forms-validation/rdo-fix-deef-proof/spouse-result.json','w').write(json.dumps(r,indent=2))
