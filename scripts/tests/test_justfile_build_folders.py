"""The nsh recipes take the caller's build folder and tool folder.

A recipe that sets CARGO_TARGET_DIR unconditionally makes every caller share one folder, and Cargo cannot tell two
checkouts of the same crate version apart there: a "before" build can link the "after" checkout's crates, form
definitions included, with no error. So every nsh recipe keeps a CARGO_TARGET_DIR (and NSH_TOOLS) that is already set;
everyday recipes default to the shared folder, packaging recipes to the checkout's own.
"""

import os
import re
import shutil
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
JUSTFILE = (ROOT / "justfile").read_text()


class BuildFolderTests(unittest.TestCase):
    def test_no_recipe_sets_the_build_folder_unconditionally(self):
        unix = re.findall(r'CARGO_TARGET_DIR="(?!\$\{CARGO_TARGET_DIR:-)[^"]*"', JUSTFILE)
        self.assertEqual(unix, [], "a recipe overrides a CARGO_TARGET_DIR the caller set")
        windows = [line.strip() for line in JUSTFILE.splitlines()
                   if "$env:CARGO_TARGET_DIR =" in line and "if (-not $env:CARGO_TARGET_DIR)" not in line]
        self.assertEqual(windows, [], "a Windows recipe overrides a CARGO_TARGET_DIR the caller set")
        self.assertNotIn('"$HOME/BuwizNsh/tools/bin:$PATH"', JUSTFILE, "a recipe ignores NSH_TOOLS")

    def test_packages_default_to_the_checkouts_own_folder(self):
        for recipe, folder in (("play-bundle", "$root/target/play-bundle"),
                               ("ios-appstore", "$root/target/nsh-store"),
                               ("_package-mac-appstore", "$root/target/nsh-store")):
            start = re.search(rf"^{re.escape(recipe)}\b.*:", JUSTFILE, re.M)
            self.assertIsNotNone(start, recipe)
            body = JUSTFILE[start.end():]
            body = body[:re.search(r"^\S", body, re.M).start()] if re.search(r"^\S", body, re.M) else body
            self.assertIn(f'CARGO_TARGET_DIR="${{CARGO_TARGET_DIR:-{folder}}}"', body, recipe)

    @unittest.skipUnless(os.name != "nt" and shutil.which("just") and shutil.which("bash"), "just and bash")
    def test_nsh_check_runs_the_callers_tool_in_the_callers_folder(self):
        with tempfile.TemporaryDirectory() as tmp:
            nsh = Path(tmp) / "bin" / "nsh"
            nsh.parent.mkdir()
            nsh.write_text('#!/bin/sh\necho "tool=$0 folder=$CARGO_TARGET_DIR args=$*"\n')
            nsh.chmod(nsh.stat().st_mode | stat.S_IEXEC)
            env = {**os.environ, "NSH_TOOLS": tmp, "CARGO_TARGET_DIR": str(Path(tmp) / "own")}
            result = subprocess.run(["just", "nsh-check"], cwd=ROOT, env=env, text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn(f"tool={nsh}", result.stdout)
            self.assertIn(f"folder={Path(tmp) / 'own'}", result.stdout)
            env.pop("CARGO_TARGET_DIR")
            result = subprocess.run(["just", "nsh-check"], cwd=ROOT, env=env, text=True, capture_output=True)
            self.assertIn(f"folder={Path.home() / 'BuwizNsh' / 'target-nsh'}", result.stdout)


if __name__ == "__main__":
    unittest.main()
