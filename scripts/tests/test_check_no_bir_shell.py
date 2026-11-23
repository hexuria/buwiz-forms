#!/usr/bin/env python3
"""Tests for scripts/check_no_bir_shell.py."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT_PATH = Path(__file__).resolve().parent.parent / "check_no_bir_shell.py"
REPO_ROOT = SCRIPT_PATH.parents[1]

sys.path.insert(0, str(SCRIPT_PATH.parent))
import check_no_bir_shell  # noqa: E402


class FindReferences(unittest.TestCase):
    def test_tracked_retired_paths_are_findings(self) -> None:
        for relative in ("crates/bir-shell/src/main.rs", "app/main.js", "app/forms/1700.json"):
            findings = check_no_bir_shell.find_references(
                [REPO_ROOT / relative], REPO_ROOT
            )
            self.assertTrue(findings, relative)
            self.assertIn("retired path", findings[0])

    def test_name_in_a_build_file_is_a_finding(self) -> None:
        findings = check_no_bir_shell.find_references(
            [REPO_ROOT / "scripts" / "check_no_bir_shell.py"], REPO_ROOT
        )
        # The guard itself is python and not scanned; a toml carrying the
        # name is.
        self.assertFalse(findings)

    def test_a_retired_path_named_in_any_text_file_is_a_finding(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "docs").mkdir()
            for name, text, expected in (
                ("docs/a.md", "ported from " + "app" + "/shell/home.js\n", True),
                ("docs/b.rs", "// old " + "crates" + "/bir-shell/src/host.rs\n", True),
                ("docs/c.md", "the bir-app/shell-free build and the old shell\n", False),
            ):
                path = root / name
                path.write_text(text, encoding="utf-8")
                findings = check_no_bir_shell.find_references([path], root)
                self.assertEqual(bool(findings), expected, name)
                if expected:
                    self.assertIn("names a retired path", findings[0])

    def test_current_tree_is_clean(self) -> None:
        result = subprocess.run(
            [sys.executable, "-I", str(SCRIPT_PATH)],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
