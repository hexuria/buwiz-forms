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
RUNTIME_FUNCTION = SCRIPT[SCRIPT.index('function Test-AndroidRuntime'):SCRIPT.index('$principal =')]
RUNTIME_BLOCK = SCRIPT[SCRIPT.index('$variants ='):SCRIPT.index('if ($Run) { &')]


@unittest.skipUnless(os.name == "nt" and shutil.which("powershell"), "Windows PowerShell required")
class AndroidWindowsEnvrcTests(unittest.TestCase):
    def test_runtime_pin_stamps_control_rebuilds_and_survive_interruption(self):
        for case, expected in (('current', []), ('missing', ['xtask android --dev']),
                               ('stale', ['xtask android --release']), ('empty', ['xtask android --release']),
                               ('artifact', ['xtask android --release']), ('build-error', None), ('copy-error', None)):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                installs = [root / 'tools' / path for path in
                            ('android/bin-dev', 'runtime/android/dev', 'android/bin', 'runtime/android/release')]
                sources = [root / 'checkout/target/android' / variant for variant in ('debug-dev', 'release')]
                for path in installs + sources:
                    (path / 'lib/x86_64').mkdir(parents=True)
                    (path / 'lib/x86_64/libnsh.so').write_text('runtime')
                    (path / 'classes.dex').write_text('java')
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
                    for path in installs:
                        self.assertEqual((path / 'nsh-ref').read_text().strip(), 'current')

    def test_acceleration_failure_only_defers_for_pending_reboot(self):
        block = SCRIPT[SCRIPT.index('if ($LASTEXITCODE -ne 0 -and $restartNeeded)'):SCRIPT.index('$repo =')]
        for pending, code, expected in ((True, 1, 0), (False, 1, 1), (False, 0, 0)):
            result = subprocess.run(
                ['powershell', '-NoProfile', '-NonInteractive', '-Command',
                 "$ErrorActionPreference='Stop'; function Assert-Native($message) { if ($LASTEXITCODE -ne 0) { throw $message } }; "
                 + f'$restartNeeded=${str(pending).lower()}; $LASTEXITCODE={code}; ' + block + '; exit 0'],
                text=True, capture_output=True,
            )
            self.assertEqual(result.returncode, expected, result.stderr)
            if pending:
                self.assertIn('continuing CLI and Android runtime provisioning', result.stdout)

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
