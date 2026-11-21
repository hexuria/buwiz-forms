import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


SCRIPT = (Path(__file__).resolve().parents[2] / 'scripts/run-android-windows.ps1').read_text()
LISTENER = SCRIPT[SCRIPT.index('function Assert-MockAccountListener'):SCRIPT.index('$repo =')]


@unittest.skipUnless(os.name == 'nt' and shutil.which('powershell'), 'Windows PowerShell required')
class LauncherTests(unittest.TestCase):
    def test_hardware_virtualization_flags_only_provide_positive_evidence(self):
        function = SCRIPT[SCRIPT.index('function Test-HardwareVirtualization'):SCRIPT.index('function Test-WHPXRebootPending')]
        for processors, expected in (
            ("@([pscustomobject]@{VirtualizationFirmwareEnabled=$true; VMMonitorModeExtensions=$true})", True),
            ("@([pscustomobject]@{VirtualizationFirmwareEnabled=$false; VMMonitorModeExtensions=$true})", False),
            ("@([pscustomobject]@{VirtualizationFirmwareEnabled=$true; VMMonitorModeExtensions=$false})", False),
            ("@([pscustomobject]@{VirtualizationFirmwareEnabled=$false; VMMonitorModeExtensions=$false})", False),
            ("@([pscustomobject]@{})", False), ("@()", False), ("throw 'query unavailable'", False),
        ):
            with self.subTest(processors=processors):
                result = subprocess.run(
                    ['powershell', '-NoProfile', '-NonInteractive', '-Command',
                     "$ErrorActionPreference='Stop'; function Get-CimInstance { " + processors + ' }; '
                     + function + '; Test-HardwareVirtualization'],
                    text=True, capture_output=True,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.strip(), str(expected))

    def test_whpx_pending_detection_expires_after_boot(self):
        function = SCRIPT[SCRIPT.index('function Test-WHPXRebootPending'):SCRIPT.index('function Assert-MockAccountListener')]
        for marker, feature, pending in (
            (None, 'EnablePending', True), (None, 'Enabled', False), (None, 'error', False),
            ('current', 'error', True), ('previous', 'Enabled', False), ('', 'Enabled', False),
        ):
            with self.subTest(marker=marker, feature=feature), tempfile.TemporaryDirectory() as directory:
                if marker is not None:
                    path = Path(directory) / 'BuwizNsh/android-windows/whpx-reboot-boot-time'
                    path.parent.mkdir(parents=True)
                    path.write_text({'current': '2026-01-01T00:00:00.0000000Z',
                                     'previous': '2025-01-01T00:00:00.0000000Z', '': ''}[marker])
                result = subprocess.run(
                    ['powershell', '-NoProfile', '-NonInteractive', '-Command',
                     "$ErrorActionPreference='Stop'; "
                     "function Get-CimInstance { [pscustomobject]@{LastBootUpTime=[datetime]::Parse('2026-01-01T00:00:00Z').ToUniversalTime()} }; "
                     "function Get-WindowsOptionalFeature { if ($env:TEST_FEATURE -eq 'error') { throw 'feature query unavailable' }; [pscustomobject]@{State=$env:TEST_FEATURE} }; "
                     + function + '; Test-WHPXRebootPending'],
                    env={**os.environ, 'USERPROFILE': directory, 'TEST_FEATURE': feature},
                    text=True, capture_output=True,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.strip(), str(pending))

    def test_acceleration_preflight_reports_or_defers_pending_reboot(self):
        block = SCRIPT[SCRIPT.index('& "$SdkRoot\\emulator\\emulator.exe" -accel-check'):SCRIPT.index('Assert-MockAccountListener (')]
        block = block.replace('& "$SdkRoot\\emulator\\emulator.exe"', 'Fake-Emulator')
        for code, pending, virtualized, skip, expected, continues in (
            (0, False, False, False, 0, True), (0, True, False, True, 0, True),
            (1, True, True, False, 1, False), (1, True, True, True, 0, False),
            (1, True, False, False, 1, False), (1, True, False, True, 0, False),
            (1, False, True, False, 1, False), (1, False, True, True, 1, False),
            (1, False, False, False, 1, False), (1, False, False, True, 1, False),
        ):
            with self.subTest(code=code, pending=pending, virtualized=virtualized, skip=skip):
                result = subprocess.run(
                    ['powershell', '-NoProfile', '-NonInteractive', '-Command',
                     "$ErrorActionPreference='Stop'; "
                     f'function Fake-Emulator {{ $global:LASTEXITCODE={code} }}; '
                     f'function Test-WHPXRebootPending {{ ${str(pending).lower()} }}; '
                     f'function Test-HardwareVirtualization {{ ${str(virtualized).lower()} }}; '
                     f'$SkipIfRebootPending=${str(skip).lower()}; '
                     "function Assert-Native($message) { throw $message }; "
                     + block + "; Write-Output 'launch continues'"],
                    text=True, capture_output=True,
                )
                self.assertEqual(result.returncode, expected, result.stderr)
                self.assertEqual('launch continues' in result.stdout, continues)
                if code:
                    output = result.stdout + result.stderr
                    self.assertIn('Restart-Computer' if pending else 'Hardware acceleration is required', output)
                    self.assertEqual('VT-x could not be verified before the reboot' in output,
                                     pending and not virtualized)

    def test_devin_cpu_flags_do_not_block_pending_reboot_deferral(self):
        function = SCRIPT[SCRIPT.index('function Test-HardwareVirtualization'):SCRIPT.index('function Test-WHPXRebootPending')]
        block = SCRIPT[SCRIPT.index('& "$SdkRoot\\emulator\\emulator.exe" -accel-check'):SCRIPT.index('Assert-MockAccountListener (')]
        block = block.replace('& "$SdkRoot\\emulator\\emulator.exe"', 'Fake-Emulator')
        result = subprocess.run(
            ['powershell', '-NoProfile', '-NonInteractive', '-Command',
             "$ErrorActionPreference='Stop'; $SkipIfRebootPending=$true; "
             "function Get-CimInstance { [pscustomobject]@{VirtualizationFirmwareEnabled=$false; VMMonitorModeExtensions=$false; HypervisorPresent=$true} }; "
             "function Fake-Emulator { $global:LASTEXITCODE=1 }; "
             "function Test-WHPXRebootPending { $true }; "
             + function + block + "; throw 'launch must be deferred'"],
            text=True, capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('VT-x could not be verified before the reboot', result.stdout)
        self.assertIn('Restart-Computer', result.stdout)

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
