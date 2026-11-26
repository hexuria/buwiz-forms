import subprocess, time, os, re, xml.etree.ElementTree as ET
from pathlib import Path
from datetime import datetime, timezone

ADB=os.path.expanduser('~/Android/Sdk/platform-tools/adb')
OUT=Path('/home/ubuntu/forms-validation/android-codebox-watch')
LOG=OUT/'watch.log'
STATUS=OUT/'STATUS'
END_TS=datetime(2026,11,26,10,15,0,tzinfo=timezone.utc).timestamp()
FIELD=(146,590,934,716)
ACTIVITY='com.longbridge.nsh.host/com.longbridge.nsh.DevActivity'
URL='ws://127.0.0.1:8792/ws'

def sh(args, timeout=15, text=True):
    try:
        return subprocess.run(args, text=text, capture_output=True, timeout=timeout).stdout
    except Exception as e:
        return f'ERROR {e}'

def adb(*args, timeout=15):
    return sh([ADB,*map(str,args)], timeout=timeout)

def shell(cmd, timeout=15):
    return adb('shell', cmd, timeout=timeout)

def tap(x,y):
    adb('shell','input','tap',x,y)

def uptime():
    return shell('uptime').strip().replace('\n',' ')

def now():
    return datetime.now(timezone.utc).isoformat(timespec='seconds')

def parse_bounds(s):
    m=re.match(r'\[(\d+),(\d+)\]\[(\d+),(\d+)\]',s or '')
    return tuple(map(int,m.groups())) if m else None

def covers(b, f=FIELD):
    return b and b[0]<=f[0] and b[1]<=f[1] and b[2]>=f[2] and b[3]>=f[3]

def dump(path=None):
    remote='/sdcard/watch-ui.xml'
    adb('shell','uiautomator','dump','--compressed',remote,timeout=10)
    local=OUT/'current-ui.xml'
    subprocess.run([ADB,'pull',remote,str(local)],capture_output=True,timeout=10)
    try:
        root=ET.parse(local).getroot()
    except Exception:
        return None, [], local
    nodes=list(root.iter('node'))
    if path:
        path.write_bytes(local.read_bytes())
    return root, nodes, local

def summarize(nodes):
    edit=None; focused=[]; covering=[]; seen_edit=False
    for n in nodes:
        cls=n.get('class') or ''
        b=parse_bounds(n.get('bounds'))
        if 'EditText' in cls and b==FIELD:
            edit={k:n.get(k) for k in ['class','text','bounds','focused','focusable','clickable','enabled']}
            seen_edit=True
        if n.get('focused')=='true': focused.append({k:n.get(k) for k in ['class','text','bounds','focused','focusable','clickable','enabled']})
        if seen_edit and covers(b): covering.append({k:n.get(k) for k in ['class','text','content-desc','bounds','focused','focusable','clickable','enabled']})
    return edit, focused, covering

def input_shown():
    out=shell('dumpsys input_method')
    m=re.search(r'^\s*mInputShown=(\S+)',out,re.M)
    return m.group(1) if m else 'unknown'

def input_lines():
    out=shell('dumpsys input_method',timeout=20)
    lines=[l for l in out.splitlines() if 'mServedView' in l or 'mInputShown' in l]
    return '\n'.join(lines)

def window_lines():
    out=shell('dumpsys window',timeout=20)
    return '\n'.join([l for l in out.splitlines() if 'mCurrentFocus' in l or 'mFocusedApp' in l])

def page(nodes):
    texts='\n'.join(n.get('text') or '' for n in nodes)
    if 'Enter the code' in texts: return 'code'
    if 'Sign in to eBIRForms' in texts: return 'email'
    return 'other'

def foreground_app():
    _,nodes,_=dump()
    pkg=''
    if nodes:
        # any node carries the foreground package
        pkg=nodes[0].get('package') or ''
    if 'com.longbridge.nsh.host' not in pkg:
        adb('shell','am','start','-n',ACTIVITY,'--es','url',URL)
        time.sleep(4)

def wait_page(want, seconds=12):
    end=time.time()+seconds
    while time.time()<end:
        _,nodes,_=dump()
        p=page(nodes)
        if p==want: return p,nodes
        time.sleep(.6)
    return (page(nodes) if nodes else 'dump-error'),nodes

def fresh_code_page(counter, came_from_change=False):
    foreground_app()
    _,nodes,_=dump()
    p=page(nodes)
    if p=='code':
        tap(540,1036)  # Change email
        time.sleep(1.2)
        came_from_change=True
        p,nodes=wait_page('email',5)
    if p!='email':
        # bring the app forward once more and re-read; never force-stop during routine checks
        foreground_app()
        p,nodes=wait_page('email',5)
    tap_email = not came_from_change
    if tap_email:
        tap(540,760); time.sleep(.3)
    email=f'watch{counter}@example.test'
    adb('shell','input','text',email)
    time.sleep(.4)
    tap(540,990)
    p,nodes=wait_page('code',14)
    return p,nodes,email

def eval_tap(label,x,y,counter):
    p,nodes,email=fresh_code_page(counter)
    before=uptime()
    tap(x,y); time.sleep(1.0)
    ui_path=OUT/f'{label}-after-tap.xml'
    _,nodes2,_=dump(ui_path)
    edit,focused,covering=summarize(nodes2)
    shown=input_shown()
    focused_cls=(focused[0].get('class') if focused else 'none')
    focus_ok=bool(edit and edit.get('focused')=='true' and focused_cls=='android.widget.EditText')
    # Use one digit only, so nothing submits. Compare the native field text.
    digit='1' if label=='group-centre' else '2'
    keycode='8' if digit=='1' else '9'
    adb('shell','input','keyevent',keycode)
    time.sleep(.6)
    _,nodes3,_=dump()
    edit_after,focused_after,_=summarize(nodes3)
    text_after=(edit_after or {}).get('text') or ''
    digit_ok=(text_after==digit)
    line=(f'{now()} uptime="{before}" label={label} tap={x},{y} page={p} email={email} '
          f'focus={focused_cls} edit={edit} inputShown={shown} focus_ok={focus_ok} '
          f'digit={digit} digit_ok={digit_ok} edit_after={edit_after}')
    with LOG.open('a') as f: f.write(line+'\n')
    return focus_ok,digit_ok,edit,covering,line

def capture_bad(tag, edit, covering):
    d=OUT/('bad-'+datetime.now(timezone.utc).strftime('%Y%m%d-%H%M%S'))
    d.mkdir(exist_ok=True)
    (d/'uptime.txt').write_text(uptime()+'\n')
    (d/'dumpsys-window-focus.txt').write_text(window_lines()+'\n')
    (d/'dumpsys-input-method-lines.txt').write_text(input_lines()+'\n')
    (d/'dumpsys-input-method-full.txt').write_text(shell('dumpsys input_method',timeout=20))
    _,nodes,_=dump(d/'uiautomator.xml')
    edit2,focused,covering2=summarize(nodes)
    (d/'uiautomator-analysis.txt').write_text(f'edit={edit2 or edit}\nfocused={focused}\ncovering_after_edit={covering2 or covering}\n')
    subprocess.run([ADB,'exec-out','screencap','-p'],stdout=(d/'screen.png').open('wb'))
    (d/'README.txt').write_text('Bad state capture before force-stop/reinstall. See watch.log for ordered probes.\n')
    return d

def probe_after_rebootless_start(kind,counter):
    # Caller decides whether force-stop or reinstall happened. This only evaluates a group-centre tap.
    ok,dig,edit,cov,line=eval_tap(kind+'-group-centre',540,650,counter)
    return ok,dig,line

def main():
    OUT.mkdir(exist_ok=True)
    STATUS.write_text('running\n')
    with LOG.open('a') as f: f.write(f'{now()} watch-start uptime="{uptime()}" end=2026-11-26T10:15:00Z\n')
    counter=3
    bad=None
    while time.time()<END_TS and not bad:
        for label,x,y in [('group-centre',540,650),('box1-centre',204,652)]:
            counter+=1
            try:
                focus_ok,digit_ok,edit,covering,line=eval_tap(label,x,y,counter)
            except Exception as e:
                with LOG.open('a') as f: f.write(f'{now()} ERROR label={label} {type(e).__name__}: {e}\n')
                continue
            if not focus_ok:
                bad=(label,x,y)
                d=capture_bad(label,edit,covering)
                with LOG.open('a') as f: f.write(f'{now()} BAD_STATE label={label} tap={x},{y} capture_dir={d} digit_ok={digit_ok}\n')
                break
        if bad: break
        next_sleep=1800 + (int(time.time())%900)  # 30-45 minutes, deterministic enough
        with LOG.open('a') as f: f.write(f'{now()} clean-check next_sleep={next_sleep}\n')
        time.sleep(next_sleep)
    if bad:
        label,x,y=bad
        # (b) force-stop and reopen, no reinstall.
        adb('shell','am','force-stop','com.longbridge.nsh.host')
        time.sleep(1)
        adb('shell','am','start','-n',ACTIVITY,'--es','url',URL)
        time.sleep(6)
        counter+=1
        ok,dig,line=probe_after_rebootless_start('after-force-stop',counter)
        with LOG.open('a') as f: f.write(f'{now()} PROBE_b_force_stop focus_ok={ok} digit_ok={dig}\n')
        if not ok:
            # (c) uninstall/install, no reboot.
            adb('uninstall','com.longbridge.nsh.host',timeout=30)
            apk='/home/ubuntu/forms-validation/android-8a507e47/nsh/.nsh/android/NshDev.apk'
            adb('install','-r',apk,timeout=120)
            adb('shell','am','start','-n',ACTIVITY,'--es','url',URL)
            time.sleep(6)
            counter+=1
            ok2,dig2,line=probe_after_rebootless_start('after-reinstall',counter)
            with LOG.open('a') as f: f.write(f'{now()} PROBE_c_reinstall focus_ok={ok2} digit_ok={dig2}\n')
            if not ok2:
                # (d) best-effort email-first probe. The long-press/paste part is left marked for the operator if clipboard control is unavailable.
                counter+=1
                ok3,dig3,line=probe_after_rebootless_start('after-email-first',counter)
                with LOG.open('a') as f: f.write(f'{now()} PROBE_d_email_first focus_ok={ok3} digit_ok={dig3}; long_press_paste=NOT_AUTOMATED_NEEDS_OPERATOR\n')
        STATUS.write_text('bad\n')
    else:
        with LOG.open('a') as f: f.write(f'{now()} watch-end-clean uptime="{uptime()}"\n')
        STATUS.write_text('clean\n')

if __name__=='__main__': main()
