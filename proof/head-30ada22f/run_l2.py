#!/usr/bin/env python3
import sys,time,json
sys.path.insert(0,'/home/ubuntu/forms-validation/money-93c-proof')
import drive_fault2 as d
OUT='/home/ubuntu/forms-validation/money-30ada-proof'
R='fp-txtRate0'; I='fp-txtAmtIncomePay0'
# Row editor is already open from navigation. Click income, then use a real Tab into rate.
d.click_node(I,wait=.25)
pre=d.snapshot()
d.key(['Tab']);time.sleep(.25)
focus1=d.find(R).get('focused')
d.type_text('42',delay=120);time.sleep(.25)
afterkeys=d.value(R);focuskeys=d.find(R).get('focused')
d.shot(OUT+'/l2-after42')
d.key(['Tab']);time.sleep(1.2)
afterleave=d.value(R)
texts=[]
for n in d.nodes():
 for x in [n.get('name'),n.get('value')]:
  if isinstance(x,str) and ('you typed' in x or 'return holds' in x):texts.append(x)
shot=d.shot(OUT+'/l2-after-leave')
row=d.draft_row_form('1601FQ',2026,4)
rate=d.draft_path('1601FQ','schedule1[0].tax_rate',2026,4)
res={'rate_after_keys':afterkeys,'rate_after_leave':afterleave,'focused_after_tabin':focus1,'focused_after_keys':focuskeys,'sentence_texts':texts,'draft_rate':rate,'shot_after42':OUT+'/l2-after42.png','shot_after_leave':shot}
open(OUT+'/l2-result.json','w').write(json.dumps(res,indent=2))
print(json.dumps(res,indent=2))
