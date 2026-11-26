import os
import re
import subprocess
import time
import xml.etree.ElementTree as ET
from PIL import Image

ADB = os.path.expanduser('~/Android/Sdk/platform-tools/adb')
SERIAL = 'emulator-5554'
OUT = '/home/ubuntu/forms-validation/buwiz-keep/docs/proof/android-money-box-after/keep-text/comma'
os.makedirs(OUT, exist_ok=True)
LOG = os.path.join(OUT, 'comma-key-proof.log')
DB = '/data/data/com.longbridge.nsh.host/files/nsh/dev.goldcoders.bir/test-data/bir_data.db'
URL = 'ws://127.0.0.1:8792/ws'

KEY = {
    '1': (168, 1722), '2': (428, 1722), '3': (696, 1722),
    '4': (168, 1874), '5': (428, 1874), '6': (696, 1874),
    '7': (168, 2030), '8': (428, 2030), '9': (696, 2030),
    ',': (168, 2188), '0': (428, 2188), '.': (696, 2188),
}
BACK = (933, 2029)
DONE = (948, 776)
ROW_TAP = (540, 1260)

failures = []


def sh(cmd, check=True):
    r = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if check and r.returncode:
        raise RuntimeError(r.stderr or r.stdout)
    return r.stdout


def log(s):
    print(s, flush=True)
    with open(LOG, 'a') as f:
        f.write(s + '\n')


def check(name, actual, expected):
    if actual == expected:
        log(f'PASS {name}: {actual!r}')
        return True
    failures.append(f'{name}: expected {expected!r}, got {actual!r}')
    log(f'FAIL {name}: expected {expected!r}, got {actual!r}')
    return False


def adb(*args, check=True):
    return sh([ADB, '-s', SERIAL] + list(args), check=check)


def tap(x, y, pause=0.42):
    adb('shell', 'input', 'swipe', str(x), str(y), str(x), str(y), '150')
    time.sleep(pause)


def dump():
    adb('shell', 'uiautomator', 'dump', '/sdcard/ui.xml', check=False)
    xml = adb('shell', 'cat', '/sdcard/ui.xml', check=False)
    try:
        return ET.fromstring(xml)
    except Exception:
        return ET.fromstring('<hierarchy/>')


def nodes():
    root = dump()
    return [(n.get('text', ''), n.get('bounds', ''), n.get('class', '')) for n in root.iter('node')]


def edits():
    ans = []
    for t, b, c in nodes():
        if c.endswith('EditText'):
            m = re.match(r'\[(\d+),(\d+)\]\[(\d+),(\d+)\]', b)
            if m:
                ans.append((t, tuple(map(int, m.groups()))))
    return ans


def visible_texts():
    return '\n'.join(t for t, _b, _c in nodes() if t)


def shot(name):
    data = subprocess.run([ADB, '-s', SERIAL, 'exec-out', 'screencap', '-p'], stdout=subprocess.PIPE).stdout
    path = os.path.join(OUT, name)
    with open(path, 'wb') as f:
        f.write(data)
    return path


def crop_input(src, name):
    im = Image.open(os.path.join(OUT, src))
    crop = im.crop((25, 0, 1045, 225)).resize((1530, 337))
    crop.save(os.path.join(OUT, name))


def amount_box():
    es = edits()
    if not es:
        return None
    return sorted(es, key=lambda x: x[1][1])[0]


def amount_text():
    e = amount_box()
    assert e, 'no amount EditText visible'
    return e[0]


def wait_for_schedule(timeout=18):
    end = time.time() + timeout
    while time.time() < end:
        text = visible_texts()
        if 'Schedule 1 – Computation of Tax, Items 1 to 6' in text and 'PT010' in text:
            return True
        time.sleep(0.7)
    return False


def launch():
    adb('shell', 'am', 'start', '-S', '-n', 'com.longbridge.nsh.host/com.longbridge.nsh.DevActivity',
        '--es', 'url', URL,
        '--es', 'nsh.env.EBIR_TEST_ENV', '1',
        '--es', 'nsh.env.BIR_AUTH_BYPASS', '1',
        '--es', 'nsh.env.BUWIZ_NO_KEYCHAIN', '1',
        '--es', 'nsh.env.BIR_SFTP_DRY_RUN', '1',
        '--es', 'nsh.env.BIR_DATABASE_PATH', DB, check=False)
    if not wait_for_schedule():
        raise RuntimeError('schedule row did not appear after launch')


def open_row():
    for _ in range(4):
        if amount_box() is not None:
            return
        tap(*ROW_TAP, pause=0.9)
    raise RuntimeError('row drawer did not open: ' + repr(nodes()[:25]))


def focus_amount_end():
    open_row()
    e = amount_box()
    assert e
    _text, (x1, y1, x2, y2) = e
    tap(x2 - 2, (y1 + y2) // 2, pause=0.9)
    e = amount_box()
    assert e
    log('focused amount text ' + repr(e[0]))
    return e


def clear_to_empty():
    e = amount_box()
    assert e
    _t, (x1, y1, x2, y2) = e
    y = (y1 + y2) // 2
    start = amount_text()
    log('clearing ' + repr(start) + ' with keypad Backspace')
    # Android sometimes leaves the caret before the final right-aligned digit after a padding tap. Reposition and
    # verify progress instead of assuming one tap put it at the end; all deletion itself is keypad Backspace.
    positions = [x2 - 2, x2 - 5, x2 - 12, x2 - 30, x2 - 2]
    for pos in positions:
        now = amount_text()
        if now == '':
            log('after clear ' + repr(now))
            return
        tap(pos, y, pause=0.6)
        for _ in range(len(now) + 2):
            before = amount_text()
            if before == '':
                log('after clear ' + repr(before))
                return
            tap(*BACK, pause=0.45)
            after = amount_text()
            log('backspace while clearing: ' + repr(before) + ' -> ' + repr(after))
            if after == '':
                log('after clear ' + repr(after))
                return
            if after == before:
                break
    raise RuntimeError('not empty after keypad Backspace: ' + repr(amount_text()))


def row_line():
    text = visible_texts()
    for line in text.splitlines():
        if line.startswith('PT010 · '):
            return line
    return ''


def type_sequence_capture(seq, expected_steps, stem):
    for i, ch in enumerate(seq):
        tap(*KEY[ch], pause=0.62)
        shown = amount_text()
        check(f'{stem} after key {i + 1} {ch!r}', shown, expected_steps[i])
    keyed = shot(stem + '-keyed.png')
    crop_input(stem + '-keyed.png', stem + '-keyed-crop.png')
    return keyed


def done_save_reopen(stem, expected_shown, expected_row):
    tap(*DONE, pause=1.8)
    shot(stem + '-saved.png')
    row = row_line()
    check(stem + ' saved row', row, expected_row)
    saved_amount = row.split(' · ')[1] if ' · ' in row else ''
    log(stem + ' saved amount extracted from row: ' + repr(saved_amount))

    open_row()
    shown = amount_text()
    check(stem + ' amount shown after Done/reopen', shown, expected_shown)
    shot(stem + '-reopened-shown.png')
    crop_input(stem + '-reopened-shown.png', stem + '-reopened-shown-crop.png')
    return row, saved_amount, shown


def main():
    open(LOG, 'w').write('')
    adb('reverse', 'tcp:8792', 'tcp:8792', check=False)
    launch()
    log('comma key proof started; keypad comma coordinate ' + repr(KEY[',']))

    focus_amount_end()
    clear_to_empty()
    type_sequence_capture('15,50', ['1', '15', '15,', '15,5', '15,50'], 'keys-15comma50')
    done_save_reopen('keys-15comma50', '1,550.00', 'PT010 · 1,550.00 · 3.0 · 46.50')

    focus_amount_end()
    clear_to_empty()
    type_sequence_capture('1,500', ['1', '1,', '1,5', '1,50', '1,500'], 'keys-1comma500')
    done_save_reopen('keys-1comma500', '1,500.00', 'PT010 · 1,500.00 · 3.0 · 45.00')

    if failures:
        log('ANDROID_COMMA_KEY_FAIL ' + ' | '.join(failures))
        raise SystemExit(1)
    log('ANDROID_COMMA_KEY_PASS')


main()
