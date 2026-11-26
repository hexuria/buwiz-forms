import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = (ROOT / "scripts/setup-android-windows.ps1").read_text(encoding="utf-8")
ENVRC_BLOCK = SCRIPT[SCRIPT.index("if ($env:ENVRC) {"):SCRIPT.index("$machinePath =")]
RUNTIME_FUNCTION = SCRIPT[SCRIPT.index('function Remove-RuntimeLink'):SCRIPT.index('$principal =')]
RUNTIME_BLOCK = SCRIPT[SCRIPT.index('$variants ='):SCRIPT.index('if ($Run) { &')]


@unittest.skipUnless(os.name == "nt" and shutil.which("powershell"), "Windows PowerShell required")
class AndroidWindowsEnvrcTests(unittest.TestCase):
    def test_whpx_setup_records_pending_boot_and_skips_reenabling(self):
        block = SCRIPT[SCRIPT.index('$feature = Get-WindowsOptionalFeature'):SCRIPT.index("& (Join-Path $SdkRoot 'emulator\\emulator.exe') -accel-check")]
        for feature, restart in (('EnablePending', True), ('Disabled', True), ('Enabled', False)):
            with self.subTest(feature=feature), tempfile.TemporaryDirectory() as directory:
                result = subprocess.run(
                    ['powershell', '-NoProfile', '-NonInteractive', '-Command',
                     "$ErrorActionPreference='Stop'; "
                     "function Get-WindowsOptionalFeature { [pscustomobject]@{State=$env:TEST_FEATURE} }; "
                     "function Enable-WindowsOptionalFeature { if ($env:TEST_FEATURE -ne 'Disabled') { throw 'unexpected enable' }; [pscustomobject]@{RestartNeeded=$true} }; "
                     "function Get-CimInstance { [pscustomobject]@{LastBootUpTime=[datetime]::Parse('2026-01-01T00:00:00Z').ToUniversalTime()} }; "
                     + block],
                    env={**os.environ, 'USERPROFILE': directory, 'TEST_FEATURE': feature},
                    text=True, capture_output=True,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                marker = Path(directory) / 'BuwizNsh/android-windows/whpx-reboot-boot-time'
                self.assertEqual(marker.exists(), restart)
                if restart:
                    self.assertEqual(marker.read_text().strip(), '2026-01-01T00:00:00.0000000Z')

    def test_runtime_pin_stamps_control_rebuilds_and_survive_interruption(self):
        for case, expected in (('current', []), ('missing', ['xtask android --dev']),
                               ('stale', ['xtask android --release']), ('empty', ['xtask android --release']),
                               ('artifact', ['xtask android --release']), ('build-error', None), ('copy-error', None)):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                installs = [root / 'tools' / path for path in
                            ('android/bin-dev', 'runtime/android/dev', 'android/bin', 'runtime/android/release')]
                sources = [root / 'checkout/target/android' / variant for variant in ('debug-dev', 'release')]
                artifacts = ('lib/x86_64/libnsh.so', 'classes.dex')
                for kind, paths in (('installed', installs), ('rebuilt', sources)):
                    for index, path in enumerate(paths):
                        (path / 'lib/x86_64').mkdir(parents=True)
                        for artifact in artifacts:
                            (path / artifact).write_text(f'{kind} {index} {artifact}')
                for path in installs:
                    (path / 'nsh-ref').write_text('current\n')
                if case == 'missing':
                    (installs[0] / 'nsh-ref').unlink()
                elif case in ('stale', 'empty', 'build-error'):
                    (installs[2] / 'nsh-ref').write_text('' if case == 'empty' else 'previous')
                elif case in ('artifact', 'copy-error'):
                    (installs[2] / 'classes.dex').unlink()
                command = (
                    "$ErrorActionPreference='Stop'; $pin='current'; $tools=Join-Path $env:TEST_ROOT 'tools'; "
                    "$checkout=Join-Path $env:TEST_ROOT 'checkout'; $script:builds=@(); "
                    "function Write-Step {} ; function Assert-Native($message) { if ($LASTEXITCODE -ne 0) { throw $message } }; "
                    "function cargo { $script:builds += ($args -join ' '); $global:LASTEXITCODE=0; if ($env:TEST_CASE -eq 'build-error') { $global:LASTEXITCODE=1 } }; "
                    "if ($env:TEST_CASE -eq 'copy-error') { function Copy-Item { throw 'interrupted copy' } }; "
                    + RUNTIME_FUNCTION + RUNTIME_BLOCK + 'ConvertTo-Json -InputObject @($script:builds) -Compress'
                )
                result = subprocess.run(['powershell', '-NoProfile', '-NonInteractive', '-Command', command],
                                        env={**os.environ, 'TEST_ROOT': directory, 'TEST_CASE': case},
                                        text=True, capture_output=True)
                if expected is None:
                    self.assertNotEqual(result.returncode, 0)
                    if case == 'copy-error':
                        self.assertIn('interrupted copy', result.stderr)
                        self.assertFalse((installs[2] / 'nsh-ref').exists())
                    else:
                        self.assertIn('runtime build failed', result.stderr)
                        self.assertEqual((installs[2] / 'nsh-ref').read_text(), 'previous')
                else:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(json.loads(result.stdout), expected)
                    for index, path in enumerate(installs):
                        self.assertEqual((path / 'nsh-ref').read_text().strip(), 'current')
                        variant = index // 2
                        build = 'xtask android --dev' if variant == 0 else 'xtask android --release'
                        for artifact in artifacts:
                            contents = ((sources[variant] / artifact).read_text() if build in expected
                                        else f'installed {index} {artifact}')
                            self.assertEqual((path / artifact).read_text(), contents)

    def test_acceleration_failure_only_defers_for_pending_reboot(self):
        block = SCRIPT[SCRIPT.index('if ($LASTEXITCODE -ne 0 -and $restartNeeded)'):SCRIPT.index('$repo =')]
        # (reboot pending, -NoEmulator, the acceleration check's exit code, the script's)
        for pending, no_emulator, code, expected in ((True, False, 1, 0), (False, False, 1, 1), (False, False, 0, 0),
                                                     (False, True, 1, 0), (False, True, 0, 0), (True, True, 1, 0)):
            result = subprocess.run(
                ['powershell', '-NoProfile', '-NonInteractive', '-Command',
                 "$ErrorActionPreference='Stop'; function Assert-Native($message) { if ($LASTEXITCODE -ne 0) { throw $message } }; "
                 + f'$restartNeeded=${str(pending).lower()}; $NoEmulator=${str(no_emulator).lower()}; $LASTEXITCODE={code}; '
                 + block + '; exit 0'],
                text=True, capture_output=True,
            )
            self.assertEqual(result.returncode, expected, result.stderr)
            if code and expected == 0:
                self.assertIn('continuing CLI and Android runtime provisioning', result.stdout)
                # A pending reboot is said first: the emulator will work after it.
                self.assertEqual('awaits a reboot' in result.stdout, pending)
                self.assertEqual('-NoEmulator' in result.stdout, not pending)
            if code == 0:
                self.assertNotIn('continuing', result.stdout)

    def test_no_emulator_is_refused_with_run_and_survives_elevation(self):
        guard = SCRIPT[SCRIPT.index('if ($Run -and $NoEmulator)'):SCRIPT.index('$CmdlineToolsUrl =')]
        for run, no_emulator, expected in ((True, True, 1), (True, False, 0), (False, True, 0)):
            result = subprocess.run(
                ['powershell', '-NoProfile', '-NonInteractive', '-Command',
                 f'$Run=${str(run).lower()}; $NoEmulator=${str(no_emulator).lower()}; ' + guard + '; exit 0'],
                text=True, capture_output=True,
            )
            self.assertEqual(result.returncode, expected, result.stderr)
        self.assertIn("if ($NoEmulator) { $elevationArgs += '-NoEmulator' }", SCRIPT)
        self.assertIn('[switch]$NoEmulator', SCRIPT)

    def run_block(self, envrc, value):
        return subprocess.run(
            ["powershell", "-NoProfile", "-NonInteractive", "-Command",
             "$ErrorActionPreference='Stop'; $vars=@{LIBCLANG_PATH=$env:TEST_VALUE}; " + ENVRC_BLOCK],
            env={**os.environ, "ENVRC": envrc, "TEST_VALUE": value},
            text=True, capture_output=True,
        )

    def test_native_and_git_bash_paths_quote_values(self):
        with tempfile.TemporaryDirectory(prefix="android envrc ") as directory:
            for bash_path in (False, True):
                path = Path(directory) / ("bash.env" if bash_path else "native.env")
                envrc = path.as_posix()
                if bash_path:
                    envrc = "/" + envrc[0].lower() + envrc[2:]
                result = self.run_block(envrc, r"C:\Program Files\LLVM\bin")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(path.read_text().strip(), "export LIBCLANG_PATH='C:/Program Files/LLVM/bin'")

    def test_unset_envrc_does_not_write(self):
        result = self.run_block("", r"C:\Program Files\LLVM\bin")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_single_quote_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "invalid.env"
            result = self.run_block(str(path), "C:/owner's tools")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("ENVRC paths cannot contain single quotes", result.stderr)
            self.assertFalse(path.exists())


if __name__ == "__main__":
    unittest.main()
