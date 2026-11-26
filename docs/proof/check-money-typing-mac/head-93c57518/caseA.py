# A. ENTRY cases driver. Run after setup (2551Q drawer open, fp-txtATCAmt1 reachable).
import sys, time, json
sys.path.insert(0,'/Users/devin/moneyacc/cases')
from acclib import *

rec = Rec()
results=[]

def tab_in():
    for i in range(5):
        keyop('tab'); time.sleep(0.55)

def all_msgs():
    # any node text that is sentence-like under the box
    d = snap()
    hits=[]
    def walk(n):
        nm = (n.get('name') or '')
        if ('not an amount' in nm or 'box kept' in nm.lower() or 'centavos' in nm or 'commas' in nm
            or 'draft stored' in nm or 'the box shows' in nm or 'too long' in nm.lower() or 'digits' in nm.lower()):
            hits.append(nm[:220])
        for c in n.get('children',[]) or []: walk(c)
    for n in d['tree']['nodes']: walk(n)
    return hits

def run(label, entry, keysops, nid=NID):
    # entry: 'click' | 'tab' | 'shifttab'; keysops: string to type or list of ops
    set_box('')   # zero the box -> 0.00
    time.sleep(0.3)
    unfocused_box()
    v0,_ = fieldvalue(nid)
    rec.log(f'--- {label}: box before {v0!r} entry={entry}')
    if entry=='click':
        clicknode(nid); time.sleep(0.6)
    elif entry=='tab':
        tab_in()
    elif entry=='shifttab':
        clicknode(nid); time.sleep(0.5)
        keyop('tab'); time.sleep(0.5)
        keyop('shifttab'); time.sleep(0.7)
    if isinstance(keysops, str):
        states = watched_type(keysops, 120)
    else:
        states = watch_ops(keysops)
    shown = leave_and_read()
    close_done()
    sv = saved_amount()
    msgs = all_msgs()
    rec.log(f'{label}: states={states} after-leaving={shown!r} saved={sv!r} msgs={msgs if msgs else "none"}')
    results.append({'label':label,'states':states,'shown':shown,'saved':sv,'msgs':msgs})

amounts = [
    ('15000.50','15000.50'),
    ('99999.99','99999.99'),
    ('1000000.005','1000000.005'),
]
for entry in ('click','tab','shifttab'):
    for keys,_ in amounts:
        run(f'A {entry} {keys}', entry, keys)
# digits + Left Left Backspace
run('A click 1234567890 LL BS','click',[('typed','1234567890'),('key','left'),('key','left'),('key','backspace')])
run('A tab 1234567890 LL BS','tab',[('typed','1234567890'),('key','left'),('key','left'),('key','backspace')])
run('A shifttab 1234567890 LL BS','shifttab',[('typed','1234567890'),('key','left'),('key','left'),('key','backspace')])
# burst
import subprocess as sp
def run_burst(entry):
    set_box('')
    time.sleep(0.3)
    unfocused_box()
    if entry=='click':
        clicknode(NID); time.sleep(0.6)
    else:
        tab_in()
    states = watched_type('15000.50', 0)
    shown = leave_and_read()
    close_done()
    sv = saved_amount()
    rec.log(f'A burst {entry}: states={states} after={shown!r} saved={sv!r}')
    results.append({'label':f'A burst {entry}','states':states,'shown':shown,'saved':sv,'msgs':all_msgs()})
for entry in ('click','tab'):
    run_burst(entry)

rec.save('/Users/devin/moneyacc/cases/caseA.log')
json.dump(results, open('/Users/devin/moneyacc/cases/caseA.json','w'), indent=1)
print('CASEA DONE')
