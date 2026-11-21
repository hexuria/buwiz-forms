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
    def test_runtime_artifacts_are_required_even_with_pinned_cli(self):
        command = '''function Test-Path($Path) { return -not ($env:MISSING -and $Path.EndsWith($env:MISSING)) }; '''
        for missing in ("", "android\\bin-dev\\classes.dex", "android\\bin\\lib\\x86_64\\libnsh.so",
                        "runtime\\android\\dev\\classes.dex", "runtime\\android\\release\\lib\\x86_64\\libnsh.so"):
            result = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command",
                                     command + BLOCK + "[bool]$runtimeMissing | ConvertTo-Json"],
                                    env={**os.environ, "MISSING": missing}, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout), bool(missing))
        self.assertIn(' -or $runtimeMissing) {', BLUEPRINT)


if __name__ == "__main__":
    unittest.main()
