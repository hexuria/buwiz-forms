"""The `[windows] nsh-run-agent` recipe's Android hand-off, run for real through `just` against a fake SDK `adb`
and a fake `nsh` (no device, no build): which device the data plane's token goes to, where `adb` is looked for,
and what the token writer does when the launch fails or is cancelled."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import unittest


REPO = Path(__file__).resolve().parents[2]
TOKEN = 'synthetic-token-0123456789abcdef'
HOST = 'com.longbridge.nsh.host'
TOKEN_PATH = 'files/nsh/dev.goldcoders.bir/agent-token'
# The window plane's token (hexuria/nsh-gpui-agent), which the recipe stores for the same launch.
WINDOW_TOKEN_PATH = 'files/gpui-agent-token'

# The SDK's `adb.exe` is a native program, as the real one is: the token writer starts it directly, with no command
# interpreter between them, and writes the token to its standard input. This one hands its arguments and that same
# standard input to FAKE_ADB.
ADB_EXE = r'''
use std::{env, process::{exit, Command}};
fn main() {
    let script = env::current_exe().unwrap().with_extension("py");
    let ran = Command::new(env::var_os("FAKE_PYTHON").unwrap()).arg(script).args(env::args_os().skip(1)).status().unwrap();
    exit(ran.code().unwrap_or(1));
}
'''

# `adb`: answers `devices` from FAKE_DEVICES and keeps each device's "private files" in FAKE_STATE. Like the real
# one, it refuses a command that names no device when several are online.
FAKE_ADB = r'''
import atexit
import os, sys, time
from pathlib import Path
state = Path(os.environ['FAKE_STATE'])
args = sys.argv[1:]
def trace(step):
    path = os.environ.get('NSH_RUN_AGENT_TRACE')
    if path:
        try:
            with open(path, 'a') as timeline:
                timeline.write(f'{time.time():.3f}\tadb\t{step}\t' + ' '.join(args) + '\n')
        except OSError:
            pass
trace('start')
atexit.register(lambda: trace('exit'))
with open(state / 'adb.log', 'a') as log:
    log.write('\t'.join(args) + '\n')
devices = [line.split('\t') for line in os.environ['FAKE_DEVICES'].split('\n') if line]
if args == ['devices']:
    trace('devices')
    print('List of devices attached')
    for serial, status in devices:
        print(f'{serial}\t{status}')
    print()
    sys.exit(0)
online = [serial for serial, status in devices if status == 'device']
if args[:1] == ['-s']:
    serial, args = args[1], args[2:]
    if serial not in online:
        trace('device-missing')
        sys.exit(f"adb: device '{serial}' not found")
elif len(online) == 1:
    serial = online[0]
else:
    trace('device-ambiguous')
    sys.exit('adb: more than one device/emulator')
trace('device-selected')
command = ' '.join(args)
if 'run-as' not in command:
    trace('not-run-as')
    sys.exit(0)
if not (state / f'installed-{serial}').exists():
    trace('package-missing')
    sys.exit(f'run-as: unknown package: {os.environ["FAKE_HOST"]}')
token = state / f'token-{serial}'
if 'cat >' in command:
    trace('token-write')
    token.write_bytes(sys.stdin.buffer.read())
elif 'rm -f' in command:
    trace('token-remove')
    token.unlink(missing_ok=True)
else:
    trace('run-as-other')
'''

# `nsh run`: FAKE_NSH says how the launch goes. `ok` installs the host on the device nsh picks (the first online one)
# and reads and deletes the token as the app does; `fail` stops before installing; `fail-late` installs, waits for
# the token and fails without reading it; `hang` never installs and never returns; `hold` installs, waits for the
# token and keeps running without reading it (an app still starting when the developer presses Ctrl+C).
FAKE_NSH = r'''
import atexit
import os, sys, time
from pathlib import Path
state = Path(os.environ['FAKE_STATE'])
def trace(step):
    path = os.environ.get('NSH_RUN_AGENT_TRACE')
    if path:
        try:
            with open(path, 'a') as timeline:
                timeline.write(f'{time.time():.3f}\tnsh\t{step}\t' + ' '.join(sys.argv[1:]) + '\n')
        except OSError:
            pass
trace('start')
atexit.register(lambda: trace('exit'))
# The recipe's build line asks `nsh --version` before anything runs (#357): answer it as the real nsh does, without
# counting it as a launch, or every "nothing was launched" check sees nsh.args and the hang mode hangs right there.
if sys.argv[1:] == ['--version']:
    trace('version')
    print('nsh 0.6.1 (fake)')
    sys.exit(0)
(state / 'nsh.args').write_text(' '.join(sys.argv[1:]))
(state / 'nsh.pid').write_text(str(os.getppid()))
trace('launch-recorded')
mode = os.environ['FAKE_NSH']
if mode == 'fail':
    trace('mode-fail')
    sys.exit(3)
if mode == 'slow-fail':
    trace('mode-slow-fail')
    time.sleep(1)
    sys.exit(3)
if mode == 'hang':
    trace('mode-hang')
    time.sleep(600)
serial = next(l.split('\t')[0] for l in os.environ['FAKE_DEVICES'].split('\n') if l.endswith('\tdevice'))
(state / f'installed-{serial}').write_text('')
trace('installed')
token = state / f'token-{serial}'
trace('token-wait')
for _ in range(400):
    if token.exists() and token.stat().st_size:
        break
    time.sleep(0.05)
else:
    trace('token-timeout')
    sys.exit('fake nsh: the token never arrived on ' + serial)
trace('token-seen')
if mode == 'fail-late':
    trace('mode-fail-late')
    sys.exit(3)
if mode == 'hold':
    trace('mode-hold')
    (state / 'nsh.holding').write_text('')
    time.sleep(600)
trace('consume-token')
(state / 'consumed').write_bytes(token.read_bytes())
token.unlink()
'''

# Ctrl+C as the console sends it: to every process attached to the recipe's console (just, its PowerShell, the writer's
# job, nsh, adb). Run as a process of its own, which leaves its console for that one and ignores the event itself.
CTRL_C = r'''
import ctypes, sys
kernel32 = ctypes.WinDLL('kernel32', use_last_error=True)
kernel32.FreeConsole()
if not kernel32.AttachConsole(int(sys.argv[1])):
    sys.exit(f'AttachConsole: error {ctypes.get_last_error()}')
kernel32.SetConsoleCtrlHandler(None, True)
if not kernel32.GenerateConsoleCtrlEvent(0, 0):
    sys.exit(f'GenerateConsoleCtrlEvent: error {ctypes.get_last_error()}')
'''

# Starts the recipe so that Ctrl+C reaches it: a process started by one that ignores Ctrl+C (a test runner in a process
# group of its own, as CI's is) ignores it too, and so would everything the recipe starts. This one stops ignoring it
# before it starts the recipe, and only waits for it itself.
HEEDS_CTRL_C = r'''
import ctypes, signal, subprocess, sys
ctypes.WinDLL('kernel32').SetConsoleCtrlHandler(None, False)
signal.signal(signal.SIGINT, signal.SIG_IGN)
sys.exit(subprocess.call(sys.argv[1:]))
'''

TWO_DEVICES = 'emulator-5554\toffline\nemulator-5556\tdevice\nR58M123ABC\tdevice'


@unittest.skipUnless(os.name == 'nt' and shutil.which('pwsh') and shutil.which('just') and shutil.which('rustc'),
                     'Windows, pwsh, just and rustc required')
class NshRunAgentWindowsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        built = Path(tempfile.mkdtemp(prefix='nsh-run-agent-adb-'))
        cls.addClassCleanup(shutil.rmtree, built, ignore_errors=True)
        (built / 'adb.rs').write_text(ADB_EXE)
        subprocess.run(['rustc', '--edition', '2021', '-o', str(built / 'adb.exe'), str(built / 'adb.rs')],
                       check=True, capture_output=True, text=True)
        cls.adb_exe = built / 'adb.exe'

    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix='nsh-run-agent-'))
        self.addCleanup(shutil.rmtree, self.root, ignore_errors=True)
        self.state = self.root / 'state'
        self.temp = self.root / 'temp'
        self.home = self.root / 'home'
        self.local = self.root / 'local'
        for folder in (self.state, self.temp, self.home / 'BuwizNsh/tools/bin', self.local):
            folder.mkdir(parents=True)
        self.fake(self.home / 'BuwizNsh/tools/bin', 'nsh', FAKE_NSH)
        self.sdk = self.root / 'sdk'
        tools = self.sdk / 'platform-tools'
        tools.mkdir(parents=True)
        (tools / 'adb.py').write_text(FAKE_ADB)
        shutil.copy(self.adb_exe, tools / 'adb.exe')

    def fake(self, folder, name, source):
        (folder / f'{name}.py').write_text(source)
        (folder / f'{name}.cmd').write_text(f'@"{sys.executable}" "%~dp0{name}.py" %*\r\n')

    def env(self, mode, devices=TWO_DEVICES, sdk=True, **extra):
        # A PATH with pwsh and just but no adb, and a home, a temporary folder and a local application data folder
        # of the test's own.
        path = os.pathsep.join([str(Path(shutil.which(tool)).parent) for tool in ('pwsh', 'just')]
                               + [str(Path(os.environ['SystemRoot']) / 'System32')])
        env = {key: value for key, value in os.environ.items()
               if key.upper() not in ('ANDROID_HOME', 'ANDROID_SDK_ROOT', 'ANDROID_SERIAL', 'BIR_DATABASE_PATH', 'PATH')}
        env.update(PATH=path, USERPROFILE=str(self.home), HOMEDRIVE=self.home.drive, HOMEPATH=str(self.home)[2:],
                   TEMP=str(self.temp), TMP=str(self.temp), LOCALAPPDATA=str(self.local),
                   GPUI_AGENT_TOKEN=TOKEN, FAKE_STATE=str(self.state), FAKE_DEVICES=devices, FAKE_NSH=mode,
                   FAKE_HOST=HOST, FAKE_PYTHON=sys.executable, NSH_RUN_AGENT_TRACE=str(self.state / 'timeline.log'))
        if sdk:
            env['ANDROID_HOME'] = str(self.sdk)
        env.update(extra)
        return env

    def start(self, env, through=(), **how):
        if env.get('NSH_RUN_AGENT_TRACE'):
            with open(env['NSH_RUN_AGENT_TRACE'], 'a') as timeline:
                timeline.write(f'{time.time():.3f}\ttest\tstart\n')
        process = subprocess.Popen([*through, 'just', '--justfile', str(REPO / 'justfile'), 'nsh-run-agent', 'android'],
                                   cwd=REPO, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, **how)
        process.output_parts = []
        process.output_lock = threading.Lock()

        def read_output():
            try:
                while True:
                    chunk = process.stdout.read1(4096)
                    if not chunk:
                        break
                    with process.output_lock:
                        process.output_parts.append(chunk)
            except (OSError, ValueError):
                pass

        process.output_reader = threading.Thread(target=read_output, daemon=True)
        process.output_reader.start()
        return process

    def output_so_far(self, process):
        with process.output_lock:
            return b''.join(process.output_parts).decode('utf-8', errors='replace')

    def output_after_wait(self, process):
        process.output_reader.join(timeout=5)
        if not process.output_reader.is_alive():
            process.stdout.close()
        return self.output_so_far(process)

    def masked(self, text):
        return text.replace(TOKEN, '<GPUI_AGENT_TOKEN>')

    def process_tree_text(self, pid):
        powershell = shutil.which('pwsh')
        if not powershell:
            return 'pwsh not found; process tree unavailable'
        script = "Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name,CreationDate,CommandLine | ConvertTo-Json -Compress"
        try:
            ran = subprocess.run([powershell, '-NoProfile', '-Command', script], capture_output=True, text=True, timeout=10)
        except (OSError, subprocess.TimeoutExpired) as error:
            return f'process tree capture failed: {error}'
        if ran.returncode != 0:
            return 'process tree capture failed: ' + ran.stderr.strip()
        try:
            rows = json.loads(ran.stdout or '[]')
        except json.JSONDecodeError as error:
            return f'process tree capture was not json: {error}'
        if isinstance(rows, dict):
            rows = [rows]
        children = {}
        by_pid = {}
        for row in rows:
            by_pid[row['ProcessId']] = row
            children.setdefault(row.get('ParentProcessId'), []).append(row)
        lines = ['pid	parent	started	name	command line']

        def walk(one_pid, depth):
            row = by_pid.get(one_pid)
            if row is None:
                if depth == 0:
                    lines.append(f'{one_pid}	?	?	?	<already exited>')
                return
            command = (row.get('CommandLine') or '').replace('\r', ' ').replace('\n', ' ')
            lines.append('\t'.join([
                str(row['ProcessId']), str(row.get('ParentProcessId')), str(row.get('CreationDate')),
                '  ' * depth + row.get('Name', '?'), command]))
            for child in sorted(children.get(one_pid, []), key=lambda item: item['ProcessId']):
                walk(child['ProcessId'], depth + 1)

        walk(pid, 0)
        return self.masked('\n'.join(lines))

    def timeline_text(self):
        path = self.state / 'timeline.log'
        if not path.exists():
            return '<no timeline file>'
        try:
            return self.masked(path.read_text(errors='replace'))
        except OSError as error:
            return f'<timeline unreadable: {error}>'

    def timeout_evidence(self, process=None, output=''):
        parts = []
        if process is not None:
            parts.append('process tree at timeout:\n' + self.process_tree_text(process.pid))
        parts.append('partial output so far:\n' + (self.masked(output) or '<empty>'))
        parts.append('step timeline:\n' + self.timeline_text())
        return '\n'.join(parts)

    def run_recipe(self, env, timeout=120):
        process = self.start(env)
        try:
            process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            evidence = self.timeout_evidence(process, self.output_so_far(process))
            subprocess.run(['taskkill', '/T', '/F', '/PID', str(process.pid)], capture_output=True)
            self.fail('the recipe did not return\n' + evidence)
        output = self.output_after_wait(process)
        if process.output_reader.is_alive():
            self.fail('the recipe output pipe did not close; output may be incomplete\n'
                      + self.timeout_evidence(process, output))
        self.assertNotIn(TOKEN, output, 'the token is never printed')
        return process.returncode, output

    def adb_calls(self):
        log = self.state / 'adb.log'
        return [line.split('\t') for line in log.read_text().splitlines()] if log.exists() else []

    def assert_no_token_left_on_this_machine(self):
        for path in self.temp.rglob('*'):
            if path.is_file():
                self.assertNotIn(TOKEN.encode(), path.read_bytes(), f'{path} holds the token')

    def wait_for(self, what, condition, seconds=60, process=None):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            if condition():
                return
            time.sleep(0.1)
        evidence = '\n' + self.timeout_evidence(process, self.output_so_far(process) if process else '') if process else ''
        self.fail(what + evidence)

    def test_with_two_devices_the_token_goes_to_the_one_nsh_launches(self):
        code, output = self.run_recipe(self.env('ok'))
        self.assertEqual(code, 0, output)
        self.assertEqual((self.state / 'consumed').read_bytes(), TOKEN.encode(), 'byte for byte, on the launched device')
        calls = [call for call in self.adb_calls() if call != ['devices']]
        self.assertTrue(calls)
        for call in calls:
            self.assertEqual(call[:2], ['-s', 'emulator-5556'], call)
        self.assertIn('adb -s emulator-5556 forward', output)
        self.assert_no_token_left_on_this_machine()

    def test_android_serial_must_name_the_device_nsh_launches(self):
        code, output = self.run_recipe(self.env('ok', ANDROID_SERIAL='emulator-5556'))
        self.assertEqual(code, 0, output)
        self.assertEqual((self.state / 'consumed').read_bytes(), TOKEN.encode())
        shutil.rmtree(self.state)
        self.state.mkdir()
        code, output = self.run_recipe(self.env('ok', ANDROID_SERIAL='R58M123ABC'))
        self.assertNotEqual(code, 0, output)
        self.assertIn('ANDROID_SERIAL', output)
        self.assertIn('emulator-5556', output)
        self.assertFalse((self.state / 'nsh.args').exists(), 'nothing is launched on another device')

    def test_no_online_device_is_refused_before_the_launch(self):
        code, output = self.run_recipe(self.env('ok', devices='emulator-5554\toffline'))
        self.assertNotEqual(code, 0, output)
        self.assertIn('no Android device', output)
        self.assertFalse((self.state / 'nsh.args').exists())

    def test_android_studios_default_sdk_is_found_without_variables_or_path(self):
        shutil.move(self.sdk, self.local / 'Android/Sdk')
        code, output = self.run_recipe(self.env('ok', sdk=False))
        self.assertEqual(code, 0, output)
        self.assertEqual((self.state / 'consumed').read_bytes(), TOKEN.encode())

    def test_android_sdk_root_is_used_when_android_home_is_unset(self):
        code, output = self.run_recipe(self.env('ok', sdk=False, ANDROID_SDK_ROOT=str(self.sdk)))
        self.assertEqual(code, 0, output)

    def test_a_missing_sdk_or_adb_is_reported_before_the_launch(self):
        code, output = self.run_recipe(self.env('ok', sdk=False))
        self.assertNotEqual(code, 0, output)
        self.assertIn('Android SDK not found', output)
        self.assertNotIn('Join-Path', output)
        self.assertFalse((self.state / 'nsh.args').exists())
        shutil.rmtree(self.sdk / 'platform-tools')
        code, output = self.run_recipe(self.env('ok'))
        self.assertNotEqual(code, 0, output)
        self.assertIn('platform-tools', output)
        self.assertFalse((self.state / 'nsh.args').exists())

    def assert_the_writer_has_stopped(self, serial='emulator-5556'):
        # The host appearing now is what a still-running writer waits for.
        (self.state / f'installed-{serial}').write_text('')
        calls = len(self.adb_calls())
        time.sleep(3)
        self.assertEqual(len(self.adb_calls()), calls, 'the token writer still polls the device')
        self.assertFalse((self.state / f'token-{serial}').exists(), 'a token was stored after the launch ended')
        self.assert_no_token_left_on_this_machine()

    def test_a_failed_launch_stops_the_token_writer(self):
        started = time.monotonic()
        code, output = self.run_recipe(self.env('fail'))
        self.assertEqual(code, 3, output)
        self.assertLess(time.monotonic() - started, 60, 'the recipe does not wait for the writer')
        self.assert_the_writer_has_stopped()

    def test_a_launch_that_fails_after_the_token_was_stored_removes_it(self):
        code, output = self.run_recipe(self.env('fail-late'))
        self.assertEqual(code, 3, output)
        self.assertFalse((self.state / 'token-emulator-5556').exists(), 'the unread token is removed from the device')
        self.assert_the_writer_has_stopped()

    def test_writer_job_errors_are_printed_without_stop_noise_or_the_token(self):
        code, output = self.run_recipe(self.env('slow-fail', NSH_RUN_AGENT_TEST_WRITER_ERROR='1'))
        self.assertEqual(code, 3, output)
        self.assertIn('injected adb failure in token writer', output)
        self.assertNotIn('The remote pipeline has been stopped', output)
        self.assertNotIn(TOKEN, output)

        shutil.rmtree(self.state)
        self.state.mkdir()
        code, output = self.run_recipe(self.env('fail'))
        self.assertEqual(code, 3, output)
        self.assertNotIn('injected adb failure in token writer', output)
        self.assertNotIn('The remote pipeline has been stopped', output)

    def test_a_job_that_does_not_stop_is_reported(self):
        code, output = self.run_recipe(self.env('slow-fail', NSH_RUN_AGENT_TEST_JOB_NO_STOP='1'))
        self.assertEqual(code, 3, output)
        self.assertIn('the token writer job did not stop within 10 seconds and was removed', output)
        self.assertIn('the window-plane plugger job did not stop within 10 seconds and was removed', output)
        self.assertNotIn(TOKEN, output)

    def test_a_cancelled_launch_stops_the_token_writer(self):
        # Cancelled the hard way: the recipe's own PowerShell is ended while nsh runs, so nothing of its own cleans up.
        process = self.start(self.env('hang'))
        self.addCleanup(lambda: subprocess.run(['taskkill', '/T', '/F', '/PID', str(process.pid)], capture_output=True))
        self.wait_for('nsh was not launched', lambda: (self.state / 'nsh.pid').exists()
                      and (self.state / 'nsh.pid').read_text(), process=process)
        self.wait_for('the token writer never polled', lambda: len(self.adb_calls()) > 2, process=process)
        # While it waits for the host, the writer keeps the token in memory only: a file would outlive a writer that is
        # ended with the launch.
        self.assert_no_token_left_on_this_machine()
        # nsh.cmd's parent is the recipe's PowerShell (the fake records its own parent, the cmd.exe running nsh.cmd).
        shell = subprocess.run(
            ['pwsh', '-NoProfile', '-Command',
             f"(Get-CimInstance Win32_Process -Filter 'ProcessId={(self.state / 'nsh.pid').read_text()}').ParentProcessId"],
            text=True, capture_output=True).stdout.strip()
        self.assertTrue(shell.isdigit(), shell)
        subprocess.run(['taskkill', '/F', '/PID', shell], capture_output=True, check=True)
        # The fake nsh outlives its shell here (it is what would be the real, interrupted one): end it too.
        subprocess.run(['taskkill', '/T', '/F', '/PID', (self.state / 'nsh.pid').read_text()], capture_output=True)
        process.wait(timeout=60)
        self.output_after_wait(process)
        time.sleep(2)
        self.assert_the_writer_has_stopped()

    def test_ctrl_c_after_the_token_was_stored_removes_it_and_stops_the_writer(self):
        # nsh is a native program here, as the real one is: a batch file would stop at "Terminate batch job (Y/N)?".
        tools = self.home / 'BuwizNsh/tools/bin'
        (tools / 'nsh.cmd').unlink()
        shutil.copy(self.adb_exe, tools / 'nsh.exe')
        # A console of its own, so that Ctrl+C reaches the recipe's processes and not this test's.
        process = self.start(self.env('hold'), through=(sys.executable, '-c', HEEDS_CTRL_C),
                             creationflags=subprocess.CREATE_NEW_CONSOLE)
        self.addCleanup(lambda: subprocess.run(['taskkill', '/T', '/F', '/PID', str(process.pid)], capture_output=True))
        self.wait_for('nsh never saw the token', lambda: (self.state / 'nsh.holding').exists(), process=process)
        stored = self.state / 'token-emulator-5556'
        self.assertEqual(stored.read_bytes(), TOKEN.encode(), 'stored on the launched device, unread')
        sent = subprocess.run([sys.executable, '-c', CTRL_C, str(process.pid)], capture_output=True, text=True)
        self.assertEqual(sent.returncode, 0, sent.stderr)
        try:
            process.wait(timeout=60)
        except subprocess.TimeoutExpired:
            self.fail('the recipe did not end on Ctrl+C\n' + self.timeout_evidence(process, self.output_so_far(process)))
        output = self.output_after_wait(process)
        self.assertNotIn(TOKEN, output)
        self.assertFalse(stored.exists(), 'the unread token is removed from the device: ' + output)
        # Both planes' tokens go: the data plane's, and the window plane's the recipe stores beside it (#293).
        removals = [call for call in self.adb_calls() if 'rm -f' in call[-1]]
        self.assertEqual(removals, [['-s', 'emulator-5556', 'shell',
                                     f'run-as {HOST} rm -f {TOKEN_PATH} {TOKEN_PATH}.new'
                                     f' {WINDOW_TOKEN_PATH} {WINDOW_TOKEN_PATH}.new']])
        self.assert_no_token_left_on_this_machine()
        self.assert_the_writer_has_stopped()



if __name__ == '__main__':
    unittest.main()
