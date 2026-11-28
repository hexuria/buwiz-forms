import os
import re
import subprocess
import time
import xml.etree.ElementTree as ET
from PIL import Image

ADB = os.path.expanduser('~/Android/Sdk/platform-tools/adb')
SERIAL = 'emulator-5554'
OUT = '$HOME/forms-validation/buwiz-keep/docs/proof/android-money-box-after/keep-text'
os.makedirs(OUT, exist_ok=True)
LOG = os.path.join(OUT, 'android-keep-text-proof.log')
VIDEO = '/sdcard/keep-text-proof.mp4'
DB = '/data/data/com.longbridge.nsh.host/files/nsh/dev.goldcoders.bir/test-data/bir_data.db'
URL = 'ws://127.0.0.1:8792/ws'

KEY = {
    '1': (168, 1722), '2': (428, 1722), '3': (696, 1722),
    '4': (168, 1874), '5': (428, 1874), '6': (696, 1874),
    '7': (168, 2030), '8': (428, 2030), '9': (696, 2030),
    '0': (428, 2188), '.': (696, 2188),
}
BACK = (933, 2029)
DONE = (948, 776)
ROW_TAP = (540, 1260)
# On this emulator's right-aligned amount box, x=900 lands after the comma of 1,234.56.
MIDDLE_TAP_X = 900

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
    # Crop the open drawer amount row and enlarge it so the caret/handle is readable in the proof.
    im = Image.open(os.path.join(OUT, src))
    crop = im.crop((500, 40, 1045, 225)).resize((1090, 370))
    crop.save(os.path.join(OUT, name))


def amount_box():
    es = edits()
    if not es:
        return None
    return sorted(es, key=lambda x: x[1][1])[0]


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
        tap(*ROW_TAP, pause=0.85)
    raise RuntimeError('row drawer did not open: ' + repr(nodes()[:25]))


def focus_amount():
    open_row()
    e = amount_box()
    assert e
    _text, (x1, y1, x2, y2) = e
    # The input's right padding: Android puts the caret after the last digit here.
    tap(x2 - 7, (y1 + y2) // 2, pause=0.9)
    e = amount_box()
    assert e
    log('focused amount text ' + repr(e[0]))
    return e


def focus_amount_middle():
    open_row()
    e = amount_box()
    assert e
    _text, (_x1, y1, _x2, y2) = e
    tap(MIDDLE_TAP_X, (y1 + y2) // 2, pause=1.0)
    e = amount_box()
    assert e
    return e


def amount_text():
    e = amount_box()
    assert e
    return e[0]


def clear_to_empty():
    e = amount_box()
    assert e
    # Put the caret after the last digit with a tap in the input's right padding, then use only keypad Backspace.
    _t, (x1, y1, x2, y2) = e
    tap(x2 - 7, (y1 + y2) // 2, pause=0.55)
    start = amount_text()
    log('clearing ' + repr(start) + ' with keypad Backspace')
    # Every character of the amount text is significant in this branch (commas included), so one keypad Backspace
    # removes one character. Do not dump the accessibility tree between keys: that costs about a second per key.
    for _ in range(len(start) + 2):
        tap(*BACK, pause=0.22)
    time.sleep(0.45)
    now = amount_text()
    log('after clear ' + repr(now))
    if now != '':
        raise RuntimeError('not empty after keypad Backspace: ' + repr(now))


def type_keys(value):
    # Leave the IME time to commit one key before the next: faster taps once ordered 4 after the decimal point.
    for ch in value:
        tap(*KEY[ch], pause=0.58)
    time.sleep(0.85)


def done_and_shot(name):
    tap(*DONE, pause=1.65)
    return shot(name)


def seed_1234():
    focus_amount()
    clear_to_empty()
    type_keys('1234.56')
    typed = amount_text()
    check('seed typed 1234.56', typed, '1234.56')
    shot('primary-seed-typed.png')
    done_and_shot('primary-seed-saved.png')
    text = visible_texts()
    check('seed saved row formatting', 'PT010 · 1,234.56 · 3.0 · 37.04' in text, True)


def primary_sequence():
    e = amount_box()
    if e is None:
        open_row()
    before = amount_text()
    check('primary filled amount before tap', before, '1,234.56')
    shot('primary-before-focus.png')
    e = focus_amount_middle()
    focused = e[0]
    check('tap keeps filled grouped text exactly', focused, '1,234.56')
    shot('primary-after-focus.png')
    crop_input('primary-after-focus.png', 'primary-after-focus-crop.png')

    tap(*KEY['7'], pause=0.9)
    after_digit = amount_text()
    check('digit 7 inserted where the tap put the caret', after_digit, '1,7234.56')
    shot('primary-after-digit.png')
    crop_input('primary-after-digit.png', 'primary-after-digit-crop.png')

    tap(*KEY['.'], pause=0.9)
    after_dot = amount_text()
    check('second decimal point is a no-op', after_dot, '1,7234.56')
    shot('primary-after-dot.png')

    tap(*BACK, pause=0.9)
    after_back = amount_text()
    check('Backspace removes the inserted digit', after_back, '1,234.56')
    shot('primary-after-backspace.png')
    crop_input('primary-after-backspace.png', 'primary-after-backspace-crop.png')
    done_and_shot('primary-done.png')


def type_amount_case(value, name):
    focus_amount()
    clear_to_empty()
    type_keys(value)
    typed = amount_text()
    check(f'{name} typed amount', typed, value)
    shot(name + '-typed.png')
    done_and_shot(name + '-saved.png')
    return visible_texts()


def matrix():
    cases = [
        ('0.50', 'case-050', 'PT010 · 0.50 · 3.0 · 0.02'),
        ('1234.56', 'case-123456', 'PT010 · 1,234.56 · 3.0 · 37.04'),
        ('99999.99', 'case-9999999', 'PT010 · 99,999.99 · 3.0 · 3,000.00'),
        ('100000', 'case-100000', 'PT010 · 100,000.00 · 3.0 · 3,000.00'),
        ('1000000.01', 'case-100000001', 'PT010 · 1,000,000.01 · 3.0 · 30,000.00'),
    ]
    for value, name, row in cases:
        text = type_amount_case(value, name)
        check(f'{name} saved row visible', row in text, True)
        log(f'{name} visible row expectation: {row}')


def persistence_reopen():
    launch()
    time.sleep(1.0)
    text = visible_texts()
    check('persistent reopen shows final amount', 'PT010 · 1,000,000.01 · 3.0 · 30,000.00' in text, True)
    shot('reopened-persistent.png')


def main():
    open(LOG, 'w').write('')
    adb('reverse', 'tcp:8792', 'tcp:8792', check=False)
    adb('shell', 'settings', 'put', 'system', 'show_touches', '1', check=False)
    adb('shell', 'rm', '-f', VIDEO, check=False)
    launch()
    time.sleep(1)
    rec = subprocess.Popen([ADB, '-s', SERIAL, 'shell', 'screenrecord', VIDEO], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(1.2)
    started = time.time()
    try:
        log('recording started; seeding 1234.56')
        seed_1234()
        log('primary keep-text sequence')
        primary_sequence()
        log('value matrix')
        matrix()
        log('persistence reopen')
        # Keep the recording through the relaunch/open as well.
        persistence_reopen()
        log('elapsed %.1fs' % (time.time() - started))
    finally:
        rec.terminate()
        time.sleep(1.2)
        adb('shell', 'settings', 'put', 'system', 'show_touches', '0', check=False)
        adb('pull', VIDEO, os.path.join(OUT, 'keep-text-proof.mp4'), check=False)
        adb('shell', 'rm', '-f', VIDEO, check=False)
    if failures:
        log('ANDROID_KEEP_TEXT_FAIL ' + ' | '.join(failures))
        raise SystemExit(1)
    log('ANDROID_KEEP_TEXT_PASS')


main()
