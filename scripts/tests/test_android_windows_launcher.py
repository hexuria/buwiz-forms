import os
from pathlib import Path
import shutil
import subprocess
import unittest


SCRIPT = (Path(__file__).resolve().parents[2] / 'scripts/run-android-windows.ps1').read_text()
LISTENER = SCRIPT[SCRIPT.index('function Assert-MockAccountListener'):SCRIPT.index('$repo =')]


@unittest.skipUnless(os.name == 'nt' and shutil.which('powershell'), 'Windows PowerShell required')
class LauncherTests(unittest.TestCase):
    def test_only_expected_avd_can_be_stopped(self):
        block = SCRIPT[SCRIPT.index('    $avdName ='):SCRIPT.index('    & $adb -s $serial emu kill')]
        for avd, expected in (('buwiz_api35', 0), ('unrelated_avd', 1), ('', 1)):
            result = subprocess.run(
                ['powershell', '-NoProfile', '-NonInteractive', '-Command',
                 "$ErrorActionPreference='Stop'; function Fake-Adb { $env:TEST_AVD; 'OK' }; function Assert-Native {} ; $adb='Fake-Adb'; " + block],
                env={**os.environ, 'TEST_AVD': avd}, text=True, capture_output=True,
            )
            self.assertEqual(result.returncode, expected, result.stderr)
            if expected:
                self.assertIn('Refusing to stop', result.stderr)

    def test_existing_listener_requires_auto_approving_mock(self):
        for name, command, listener, expected in (
            ('mock_account.exe', 'mock_account.exe --auto-approve', True, 0),
            ('mock_account.exe', 'mock_account.exe', True, 1),
            ('other.exe', 'other.exe --auto-approve', True, 1),
            ('mock_account.exe', 'mock_account.exe --auto-approve-fake', True, 1),
            ('', '', False, 0),
        ):
            result = subprocess.run(
                ['powershell', '-NoProfile', '-NonInteractive', '-Command',
                 "$ErrorActionPreference='Stop'; function Get-CimInstance { [pscustomobject]@{Name=$env:PROC_NAME; CommandLine=$env:PROC_COMMAND} }; "
                 + LISTENER + ('Assert-MockAccountListener @([pscustomobject]@{OwningProcess=123})' if listener else 'Assert-MockAccountListener @()')],
                env={**os.environ, 'PROC_NAME': name, 'PROC_COMMAND': command}, text=True, capture_output=True,
            )
            self.assertEqual(result.returncode, expected, result.stderr)
            if expected:
                self.assertIn('Stop that listener and rerun', result.stderr)


if __name__ == '__main__':
    unittest.main()
