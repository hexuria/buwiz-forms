#!/usr/bin/env python3
# Validation driver for the real Linux window. Semantic snapshots locate controls; entry uses real xdotool key events.
import json, os, re, sqlite3, subprocess, sys, time
from pathlib import Path

OUT = Path('/home/ubuntu/forms-validation/rdo-stuck-proof')
DB = OUT / 'deef-validation.db'
ADDR = os.environ.get('GPUI_AGENT_ADDR', '127.0.0.1:17433')
DISPLAY = ':0'
LOG = OUT / 'drive-log.jsonl'

def run(cmd, check=True, env=None, timeout=15):
    e = os.environ.copy(); e['DISPLAY'] = DISPLAY; e['GPUI_AGENT_TOKEN'] = os.environ.get('GPUI_AGENT_TOKEN','rdostucklocal')
    if env: e.update(env)
    p = subprocess.run(cmd, text=True, capture_output=True, env=e, timeout=timeout)
    if check and p.returncode:
        raise RuntimeError(f"command failed {cmd}: {p.stderr[-500:]} {p.stdout[-500:]}")
    return p

def log(event, **kw):
    with LOG.open('a') as f:
        f.write(json.dumps({'t': time.time(), 'event': event, **kw}, ensure_ascii=False) + '\n')

def agent_json(args, timeout=15):
    p = run(['gpui-agent', '--addr', ADDR, '--reply-timeout-ms', '10000'] + args, timeout=timeout)
    try: return json.loads(p.stdout)
    except Exception as e: raise RuntimeError(f'agent JSON parse failed for {args}: {p.stdout[:500]} stderr {p.stderr[:300]}')

def snapshot():
    return agent_json(['snapshot'], timeout=20)

def walk(node):
    yield node
    for ch in node.get('children') or []:
        yield from walk(ch)

def nodes(tree=None):
    tree = tree or snapshot()
    t = tree.get('tree', tree)
    if isinstance(t, dict) and isinstance(t.get('nodes'), list):
        out=[]
        for root in t['nodes']: out.extend(list(walk(root)))
        return out
    root = t.get('root', t) if isinstance(t, dict) else t
    return list(walk(root))

def find(node_id, tree=None):
    for n in nodes(tree):
        if n.get('id') == node_id: return n
    raise KeyError(f'node {node_id} not found')

def maybe(node_id, tree=None):
    try: return find(node_id, tree)
    except Exception: return None

def wait_node(node_id, timeout=10):
    end = time.time() + timeout
    while time.time() < end:
        try:
            n = find(node_id)
            return n
        except Exception:
            time.sleep(.2)
    raise TimeoutError(f'{node_id} did not appear')

def value(node_id):
    n = find(node_id)
    return n.get('value') if n.get('value') is not None else n.get('name','')

def window_id():
    # Prefer the validation app window; titles seen for NSH development hosts include eBIRForms.
    for pat in ['eBIRForms', 'BIR Form 2551Q', 'nsh-app']:
        p = run(['xdotool', 'search', '--onlyvisible', '--name', pat], check=False)
        ids = [x for x in p.stdout.split() if x.strip()]
        if ids: return ids[-1]
    raise RuntimeError('validation window not found')

def window_geom():
    wid = window_id()
    p = run(['xdotool', 'getwindowgeometry', '--shell', wid])
    d = dict(line.split('=',1) for line in p.stdout.strip().splitlines() if '=' in line)
    return int(d['X']), int(d['Y']), int(d['WIDTH']), int(d['HEIGHT']), int(wid)

SCALE = float(os.environ.get('MONEY_SCALE', '1'))

def screen_point(n):
    x, y, w, h, wid = window_geom()
    b = n.get('bounds') or {}
    cx = float(b.get('x',0)) + float(b.get('w',0))/2
    cy = float(b.get('y',0)) + float(b.get('h',0))/2
    # Window geometry is the outer window; NSH bounds are logical pixels in the surface. On this X11 session scale is 1.
    return round(x + cx * SCALE), round(y + cy * SCALE), wid, (cx, cy)

def click_node(node_id, wait=0.35):
    n = find(node_id)
    sx, sy, wid, logical = screen_point(n)
    run(['xdotool', 'windowactivate', str(wid)], check=False)
    time.sleep(.1)
    run(['xdotool', 'mousemove', str(sx), str(sy), 'click', '1'])
    time.sleep(wait)
    log('click', id=node_id, screen=[sx,sy], logical=logical)
    return sx, sy

def click_xy(x, y, wait=0.2):
    run(['xdotool', 'mousemove', str(x), str(y), 'click', '1'])
    time.sleep(wait)

def key(keys, delay=None):
    cmd = ['xdotool', 'key']
    if delay is not None: cmd += ['--delay', str(delay)]
    if isinstance(keys, str): keys = [keys]
    run(cmd + list(keys), timeout=20)

KEYMAP = {'.':'period', '/':'slash', '-':'minus', ',':'comma', ' ':'space'}
def keyname(ch):
    return KEYMAP.get(ch, ch)

def type_text(text, delay=120):
    keys = [keyname(c) for c in text]
    key(keys, delay=delay)
    log('xdotool-type', text=text, delay=delay, keys=' '.join(keys))

def ctrl_a(): key(['ctrl+a'])
def backspace(): key(['BackSpace'])
def tab(): key(['Tab'])
def left(n=1): key(['Left'] * n, delay=120)

def shot(name):
    path = OUT / f'{name}.png'
    run(['scrot', '-o', str(path)], timeout=10)
    return str(path)

def draft_row():
    con = sqlite3.connect(str(DB), timeout=2)
    try:
        row = con.execute("SELECT id, status, data_json FROM form_drafts WHERE tin=? AND form_code='2551Q' AND taxable_year=2025 AND quarter=1 ORDER BY id DESC LIMIT 1", ('00012345300000',)).fetchone()
        if not row: return None
        try: data = json.loads(row[2])
        except Exception: data = {}
        return {'id': row[0], 'status': row[1], 'data': data}
    finally: con.close()

def draft_value(key='txtATCAmt1'):
    r = draft_row()
    if not r: return None
    d = r['data']
    # The 2551Q draft stores typed field keys as typed model JSON: Schedule 1 row 1 taxable amount is here.
    if key == 'txtATCAmt1':
        try: return d.get('schedule_1',[{}])[0].get('taxable_amount')
        except Exception: return None
    if key in d: return d.get(key)
    for parent in ('values','fields','draft'):
        if isinstance(d.get(parent), dict) and key in d[parent]: return d[parent].get(key)
    return None

def wait_saved_change(key, previous, timeout=8):
    end = time.time() + timeout
    last = None
    while time.time() < end:
        last = draft_value(key)
        if last != previous and last is not None: return last
        time.sleep(.2)
    return last

def read_all_draft(key='txtATCAmt1'):
    r = draft_row() or {}
    return {'draft_id': r.get('id'), 'status': r.get('status'), 'value': draft_value(key)}

def activate():
    wid = window_id()
    run(['wmctrl','-i','-a',wid], check=False)
    time.sleep(.2)

def maximize():
    wid = window_id()
    run(['wmctrl','-i','-r',wid,'-b','add,maximized_vert,maximized_horz'], check=False)
    time.sleep(.3)
