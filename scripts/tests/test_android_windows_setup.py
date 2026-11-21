import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = (ROOT / "scripts/setup-android-windows.ps1").read_text(encoding="utf-8")
ENVRC_BLOCK = SCRIPT[SCRIPT.index("if ($env:ENVRC) {"):SCRIPT.index("$machinePath =")]


@unittest.skipUnless(os.name == "nt" and shutil.which("powershell"), "Windows PowerShell required")
class AndroidWindowsEnvrcTests(unittest.TestCase):
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
