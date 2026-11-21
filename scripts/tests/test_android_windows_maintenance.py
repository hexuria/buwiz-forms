import json
import os
from pathlib import Path
import shutil
import subprocess
import unittest


ROOT = Path(__file__).resolve().parents[2]
BLUEPRINT = (ROOT / "docs/platforms/devin-blueprint.yml").read_text()
BLOCK = BLUEPRINT[BLUEPRINT.index("      $runtimeMissing ="):BLUEPRINT.index('      if ("$have" -notlike', BLUEPRINT.index("      $runtimeMissing ="))]


@unittest.skipUnless(os.name == "nt" and shutil.which("powershell"), "Windows PowerShell required")
class MaintenanceTests(unittest.TestCase):
    def test_current_cli_does_not_mask_stale_or_empty_runtime_stamp(self):
        for stamp, expected in (('current', False), ('previous', True), ('', True)):
            result = subprocess.run(
                ['powershell', '-NoProfile', '-NonInteractive', '-Command',
                 "$pin='current'; $have='nsh (current)'; function Test-Path { $true }; function Get-Content { if ($env:STAMP) { $env:STAMP } }; "
                 + BLOCK + '(($have -notlike "*($pin)*") -or [bool]$runtimeMissing) | ConvertTo-Json'],
                env={**os.environ, 'STAMP': stamp}, capture_output=True, text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout), expected)

    def test_runtime_artifacts_are_required_even_with_pinned_cli(self):
        command = '''$pin='current'; function Get-Content { 'current' }; function Test-Path($Path) { return -not ($env:MISSING -and $Path.EndsWith($env:MISSING)) }; '''
        for missing in ("", "android\\bin-dev\\classes.dex", "android\\bin\\lib\\x86_64\\libnsh.so",
                        "runtime\\android\\dev\\classes.dex", "runtime\\android\\release\\lib\\x86_64\\libnsh.so", "nsh-ref"):
            result = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command",
                                     command + BLOCK + "[bool]$runtimeMissing | ConvertTo-Json"],
                                    env={**os.environ, "MISSING": missing}, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout), bool(missing))
        self.assertIn(' -or $runtimeMissing) {', BLUEPRINT)


if __name__ == "__main__":
    unittest.main()
