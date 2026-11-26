#!/usr/bin/env python3
# Real-window fault-hunt driver. Semantic snapshots only locate/read; entry is real xdotool.
import json, math, os, re, sqlite3, subprocess, sys, time
from pathlib import Path
sys.path.insert(0, '/home/ubuntu/forms-validation/money-focus-proof')
import drive_focus as d

OUT = Path('/home/ubuntu/forms-validation/money-focus-proof')
RESULTS = OUT/'focus-results.jsonl'
TIN='00012345300000'
# Filled after opening the 2551Q return.
YEAR=2026; QUARTER=4; MPERIOD=11
AMT='fp-txtATCAmt1'
SUR='fp-frm2551Qv2018:txt20'
BASE='fp-frm2550m:txtTax18E'
RECOMP='fp-frm2550m:txtTax18F'

def write(row):
    row=dict(row); row['t']=time.time()
    with RESULTS.open('a') as f: f.write(json.dumps(row,ensure_ascii=False)+'\n')
    print(json.dumps(row,ensure_ascii=False),flush=True)

def keychars(text):
    return [d.keyname(c) for c in text]

def xchain(*args):
    # One xdotool process: click/first key and following keys have no extra Python/tool gap.
    p=d.run(['xdotool']+list(args),timeout=20)
    return p

def click_type_burst(node_id,text):
    n=d.find(node_id); sx,sy,wid,logical=d.screen_point(n)
    d.run(['xdotool','windowactivate',str(wid)],check=False)
    xchain('mousemove',str(sx),str(sy),'click','1','key','--delay','0',*keychars(text))
    d.log('click-type-burst',id=node_id,text=text,screen=[sx,sy])

def tab_type_burst(text,delay=0):
    xchain('key','Tab','key','--delay',str(delay),*keychars(text))
    d.log('tab-type-burst',text=text,delay=delay)

def type_keys(text,delay=120):
    d.type_text(text,delay=delay)

def value(node):
    try: return d.value(node)
    except Exception: return None

def nodeinfo(node):
    try: return d.find(node)
    except Exception: return {}

def draft2551(path):
    return d.draft_path('2551Q',path,YEAR,QUARTER)

def draft2550(path):
    return d.draft_path('2550M',path,YEAR,MPERIOD)

def infer_periods():
    global YEAR,QUARTER,MPERIOD
    rows=d.latest_drafts()
    q=[r for r in rows if r[1]=='2551Q']
    m=[r for r in rows if r[1]=='2550M']
    if q: YEAR=q[-1][2]; QUARTER=q[-1][3]
    if m: MPERIOD=m[-1][3]
    return rows

def open_form(code):
    for id in ['sb-new',f'new-return-{code}','new-return-open']:
        try:
            n=d.maybe(id)
            if n: d.click_node(id,wait=.75)
            else: print('missing open control',id)
        except Exception as e: print('open click err',id,e)
        time.sleep(.5)
    time.sleep(2)

def expand_parts():
    for rounds in range(5):
        did=False
        ns=d.nodes()
        for n in ns:
            nid=n.get('id','')
            if nid.startswith('fp-part-') and nid.endswith('-toggle'):
                m=re.match(r'fp-part-(\d+)-toggle',nid)
                cont=d.maybe('fp-part-'+m.group(1),ns) if m else None
                h=(cont or {}).get('bounds',{}).get('h',999)
                if h < 100:
                    try: d.click_node(nid,wait=.25); did=True
                    except Exception: pass
        if not did: break
        time.sleep(.4)

def scroll_until(node_id, rounds=30):
    for i in range(rounds):
        if d.maybe(node_id): return True
        p=subprocess.run(['gpui-agent','--addr',d.ADDR,'scroll','form-scroll','--dy','500'],capture_output=True,text=True,env={**os.environ,'GPUI_AGENT_TOKEN':'focuslocal'})
        time.sleep(.28)
    return bool(d.maybe(node_id))

def finish_amount(case,keys,path2541='schedule_1[0].taxable_amount',fid=AMT,prefix='x',form='2551Q',expected=None,notes=''):
    time.sleep(.12)
    after_keys=value(fid)
    shot_keys=d.shot(prefix+'-after-keys')
    d.tab()
    time.sleep(1.15)
    after_leave=value(fid)
    saved=(draft2551(path2541) if form=='2551Q' else draft2550(path2541))
    shot_leave=d.shot(prefix+'-after-leaving')
    row={'case':case,'keys_action':keys,'visible_after_keys':after_keys,'visible_after_leaving':after_leave,'saved_draft_value':saved,'expected':expected,'notes':notes,'shot_after_keys':shot_keys,'shot_after_leaving':shot_leave,'raw_node':nodeinfo(fid)}
    if expected:
        row['pass']=(after_keys==expected.get('after_keys') and after_leave==expected.get('after_leave') and (expected.get('saved') is None or saved==expected.get('saved')))
    write(row)
    return row

def set_amount(text,fid=AMT):
    d.click_node(fid,wait=.25)
    d.key(['ctrl+a'])
    d.type_text(str(text),delay=120)
    d.tab(); time.sleep(1.2)

def zero_amount(fid=AMT):
    set_amount('0',fid)
    time.sleep(.25)

def focus_prev_then_tab():
    # In Schedule 1 row editor the ATC dropdown precedes the amount. Open/close it without changing it, then Tab.
    try:
        d.click_node('fp-drpATC1-trigger',wait=.35); d.key(['Escape']); time.sleep(.25)
    except Exception:
        # fall back: focus amount, shift-tab away, then tab back
        d.click_node(AMT,wait=.2); d.key(['shift+Tab']); time.sleep(.25)
    d.key(['Tab']); time.sleep(.18)

def focus_next_then_shiftab():
    # Focus amount, Tab to the next stop, then Shift+Tab back into it.
    d.click_node(AMT,wait=.2)
    d.key(['Tab']); time.sleep(.25)
    d.key(['shift+Tab']); time.sleep(.18)

def e1():
    # Tab entry.
    set_amount('1234.56')
    focus_prev_then_tab()
    d.type_text('15000.50',delay=120)
    finish_amount('E1 Tab into 1,234.56 type 15000.50','Tab, type 15000.50 at 120ms',prefix='e1-tab',expected={'after_keys':'15000.50','after_leave':'15,000.50','saved':15000.5})
    # Shift+Tab entry.
    set_amount('1234.56')
    focus_next_then_shiftab()
    d.type_text('15000.50',delay=120)
    finish_amount('E1 Shift+Tab into 1,234.56 type 15000.50','Shift+Tab, type 15000.50 at 120ms',prefix='e1-shifttab',expected={'after_keys':'15000.50','after_leave':'15,000.50','saved':15000.5})
    # Click Ctrl+A baseline.
    set_amount('1234.56')
    d.click_node(AMT,wait=.25); d.key(['ctrl+a']); d.type_text('15000.50',delay=120)
    finish_amount('E1 click Ctrl+A type 15000.50','click, Ctrl+A, type 15000.50 at 120ms',prefix='e1-click-ctrla',expected={'after_keys':'15000.50','after_leave':'15,000.50','saved':15000.5})

def h1_click():
    for i in range(1,11):
        zero_amount()
        click_type_burst(AMT,'15000.50')
        finish_amount(f'H1 click zero burst run {i}','click then first key at once; type 15000.50 delay 0',prefix=f'h1-click-{i}',expected={'after_keys':'15000.50','after_leave':'15,000.50','saved':15000.5},notes='watch for leading 0.00')

def h1_tab():
    for i in range(1,11):
        zero_amount()
        # focus preceding ATC without opening if possible; then combined Tab+typing in one xdotool process
        try:
            d.click_node('fp-drpATC1-trigger',wait=.25); d.key(['Escape']); time.sleep(.2)
        except Exception:
            d.click_node(AMT,wait=.2); d.key(['shift+Tab']); time.sleep(.2)
        tab_type_burst('15000.50',delay=0)
        finish_amount(f'H1 Tab zero burst run {i}','Tab then first key at once; type 15000.50 delay 0',prefix=f'h1-tab-{i}',expected={'after_keys':'15000.50','after_leave':'15,000.50','saved':15000.5},notes='watch for leading 0.00')

def h2():
    for text,prefix in [('15000..50','h2-dots'),('150a00.50','h2-letter')]:
        zero_amount()
        click_type_burst(AMT,text)
        row=finish_amount(f'H2 typo {text}',f'click then type {text} delay 0',prefix=prefix,notes='typo mid-burst; inspect raw_node/messages')
        # capture any visible validation/message text nearby
        row['page_texts']=[n.get('name') for n in d.nodes() if n.get('name') and ('valid' in n.get('name','').lower() or 'decimal' in n.get('name','').lower() or 'amount' in n.get('name','').lower())][:20]
        write({'case':prefix+'-messages','messages':row['page_texts']})


def focused_nodes():
    out=[]
    for n in d.nodes():
        if n.get('focused') or n.get('focus') or n.get('states') and 'focused' in str(n.get('states')):
            out.append({'id':n.get('id'),'name':n.get('name'),'value':n.get('value'),'keys':sorted(n.keys())})
    return out

def expand_all():
    n=d.maybe('fp-parts-toggle')
    if n:
        try:
            # On the desktop this button expands all collapsed parts; if already expanded the text says collapse, so inspect first.
            txt=(n.get('name') or '')+' '+(n.get('label') or '')
            if 'Collapse' not in txt:
                d.click_node('fp-parts-toggle',wait=.8)
        except Exception as e: print('expand all err',e)
    expand_parts()

def ensure_field(fid):
    if scroll_until(fid,35):
        return True
    # try upward too when overscrolled
    for i in range(10):
        if d.maybe(fid): return True
        subprocess.run(['gpui-agent','--addr',d.ADDR,'scroll','form-scroll','--dy','-500'],capture_output=True,env={**os.environ,'GPUI_AGENT_TOKEN':'focuslocal'})
        time.sleep(.25)
    return bool(d.maybe(fid))


def scroll_visible(node_id, rounds=60):
    env={**os.environ,'GPUI_AGENT_TOKEN':'focuslocal'}
    for i in range(rounds):
        n=d.maybe(node_id)
        if n:
            y=(n.get('bounds') or {}).get('y',0); h=(n.get('bounds') or {}).get('h',0)
            if 100 <= y <= 1650:
                return True
            dy=500 if y>1650 else -500
            subprocess.run(['gpui-agent','--addr',d.ADDR,'scroll','form-scroll','--dy',str(dy)],capture_output=True,text=True,env=env)
        else:
            subprocess.run(['gpui-agent','--addr',d.ADDR,'scroll','form-scroll','--dy','500'],capture_output=True,text=True,env=env)
        time.sleep(.25)
    return False

def h3():
    # 2551Q Item 20 surcharge is an editable amount override. Fill it, run Check, then use the form's own to-do step to focus it.
    if not ensure_field(SUR):
        write({'case':'H3 setup','error':'surcharge field not found'}); return
    set_amount('1500',SUR)
    time.sleep(.6)
    pre={'surcharge':draft2551('surcharge'),'override':draft2551('penalty_overrides.surcharge')}
    d.shot('h3-prefilled-1500')
    # Run validation through the visible Check button.
    try: d.click_node('fp-check',wait=1.8)
    except Exception: pass
    d.shot('h3-after-check')
    # Step forward through the to-do rows until Item 20 is current/focused. Capture each step.
    stepinfo=[]
    for i in range(4):
        try: d.click_node('fp-pill-next',wait=1.0)
        except Exception as e: stepinfo.append({'click_error':str(e)})
        info={'step':i+1,'pill':(nodeinfo('fp-pill-big').get('name') or nodeinfo('fp-pill-big').get('value')),'focused':focused_nodes(),'sur':nodeinfo(SUR)}
        stepinfo.append(info)
        if any(x.get('id')==SUR for x in info['focused']): break
    d.shot('h3-stepped-focus')
    # Type without clicking the amount: only the page's step should have put keyboard focus there.
    d.type_text('2500',delay=120)
    row=finish_amount('H3 form focus into filled surcharge then type 2500','Check, fp-pill-next to Item 20, type 2500 at 120ms',path2541='surcharge',fid=SUR,prefix='h3',expected={'after_keys':'2500','after_leave':'2,500.00','saved':2500.0},notes='page focus method: visible Check then to-do next button; prefill '+json.dumps(pre))
    row['stepinfo']=stepinfo
    write({'case':'H3 stepinfo','stepinfo':stepinfo,'prefill':pre})

def h4():
    # 2550M Item 18E domestic goods base recomputes editable 18F input tax at 12%.
    if not ensure_field(BASE):
        write({'case':'H4 setup','error':'base field not found'}); return
    for delay in [120,0]:
        for i in range(1,6):
            base=str(1000+i*100)
            # Reset/recompute: type base in 18E, leave it so 18F is recalculated, then come back to 18E.
            d.click_node(BASE,wait=.25); d.key(['ctrl+a']); d.type_text(base,delay=120); d.tab(); time.sleep(.9)
            before=value(RECOMP)
            # Click base again (caret irrelevant), then one xdotool process does Tab and the 999 burst.
            d.click_node(BASE,wait=.2)
            xchain('key','Tab','key','--delay',str(delay),*keychars('999'))
            case=f'H4 recomputed 18F type 999 run {i} delay {delay}'
            row=finish_amount(case,f'base {base}, Tab into recomputed 18F, type 999 delay {delay}',path2541='domestic_goods_input_tax',fid=RECOMP,prefix=f'h4-{delay}-{i}',form='2550M',expected={'after_keys':'999','after_leave':'999.00','saved':999.0},notes='recomputed before typing: '+str(before))
            row['recomputed_before']=before
    write({'case':'H4 complete'})

def open_initial_2551q():
    open_form('2551Q')
    time.sleep(1)
    infer_periods()
    expand_all()
    ensure_field(AMT)
    write({'case':'setup','drafts':d.latest_drafts(),'year':YEAR,'quarter':QUARTER,'shot':d.shot('setup-2551q')})

if __name__=='__main__':
    print('driver module')
