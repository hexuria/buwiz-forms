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
JUSTFILE = (ROOT / "justfile").read_text(encoding="utf-8")


class BuildFolderTests(unittest.TestCase):
    def test_no_recipe_sets_the_build_folder_unconditionally(self):
        unix = re.findall(r'CARGO_TARGET_DIR="(?!\$\{CARGO_TARGET_DIR:-)[^"]*"', JUSTFILE)
        self.assertEqual(unix, [], "a recipe overrides a CARGO_TARGET_DIR the caller set")
        windows = [line.strip() for line in JUSTFILE.splitlines()
                   if "$env:CARGO_TARGET_DIR =" in line and "if (-not $env:CARGO_TARGET_DIR)" not in line]
        self.assertEqual(windows, [], "a Windows recipe overrides a CARGO_TARGET_DIR the caller set")
        fixed_tools = [line.strip() for line in JUSTFILE.splitlines() if '"$HOME/BuwizNsh/tools/bin:$PATH"' in line]
        self.assertEqual(fixed_tools, [], "a recipe ignores NSH_TOOLS")

    def test_the_linux_container_keeps_the_callers_folders(self):
        # nsh2404.sh runs every Linux NSH command; it used to pin the shared folder whatever the caller set.
        wrapper = (ROOT / "scripts/setup-linux-nsh.sh").read_text(encoding="utf-8")
        self.assertIn('-e CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/BuwizNsh/target-nsh}"', wrapper)
        self.assertIn('-e PATH="${NSH_TOOLS:-$HOME/BuwizNsh/tools}/bin:', wrapper)
        self.assertIn("-e NSH_RUNTIME \\\n", wrapper)
        self.assertNotIn('-e CARGO_TARGET_DIR="$HOME/BuwizNsh/target-nsh"', wrapper)

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
            result = subprocess.run(["just", "nsh-check"], cwd=ROOT, env=env, text=True, encoding="utf-8", capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn(f"tool={nsh}", result.stdout)
            self.assertIn(f"folder={Path(tmp) / 'own'}", result.stdout)
            env.pop("CARGO_TARGET_DIR")
            result = subprocess.run(["just", "nsh-check"], cwd=ROOT, env=env, text=True, encoding="utf-8", capture_output=True)
            self.assertIn(f"folder={Path.home() / 'BuwizNsh' / 'target-nsh'}", result.stdout)

    @unittest.skipUnless(os.name != "nt" and shutil.which("bash"), "bash")
    def test_the_build_line_names_the_folder_the_build_really_uses(self):
        for version, expected in (("cargo 1.99.0 (abc 2026-01-01)", "folder /tmp/own"),
                                  ("cargo 1.102.0 (d69 2026-11-16) (huacnlee/cargo 0.103.0-1)",
                                   "shared by every checkout (it ignores CARGO_TARGET_DIR=/tmp/own)")):
            with self.subTest(version=version), tempfile.TemporaryDirectory() as tmp:
                for name, text in (("cargo", f'echo "{version}"'), ("nsh", 'echo "nsh 0.6.1 (8c78cfd)"')):
                    tool = Path(tmp) / name
                    tool.write_text(f"#!/bin/sh\n{text}\n")
                    tool.chmod(tool.stat().st_mode | stat.S_IEXEC)
                env = {**os.environ, "PATH": f"{tmp}:{os.environ['PATH']}", "CARGO_TARGET_DIR": "/tmp/own"}
                result = subprocess.run(["bash", "scripts/build-line.sh"], cwd=ROOT, env=env, text=True, encoding="utf-8", capture_output=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn(version, result.stderr)
                self.assertIn(expected, result.stderr)
                self.assertIn("tool nsh 0.6.1 (8c78cfd)", result.stderr)

    @unittest.skipUnless(os.name != "nt" and shutil.which("bash"), "bash")
    def test_evidence_and_packages_refuse_a_cargo_that_shares_one_folder(self):
        with tempfile.TemporaryDirectory() as tmp:
            cargo = Path(tmp) / "cargo"
            cargo.write_text('#!/bin/sh\necho "cargo 1.102.0 (d69 2026-11-16) (huacnlee/cargo 0.103.0-1)"\n')
            cargo.chmod(cargo.stat().st_mode | stat.S_IEXEC)
            env = {**os.environ, "PATH": f"{tmp}:{os.environ['PATH']}"}
            result = subprocess.run(["bash", "scripts/build-line.sh", "--strict"], cwd=ROOT, env=env, text=True, encoding="utf-8", capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("shares one build folder between checkouts", result.stderr)
        for recipe in ("play-bundle", "nsh-test", "nsh-run-agent", "nsh-agent-check", "nsh-shots"):
            start = re.search(rf"^{re.escape(recipe)}\b.*:", JUSTFILE, re.M)
            body = JUSTFILE[start.end():]
            end = re.search(r"^\S", body, re.M)
            body = body[:end.start()] if end else body
            self.assertIn("scripts/pinned-cargo-path.sh", body, recipe)
            self.assertIn("scripts/build-line.sh --strict", body, recipe)


if __name__ == "__main__":
    unittest.main()
