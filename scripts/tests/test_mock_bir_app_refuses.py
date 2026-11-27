"""`just mock-bir-app` refuses to run (accounts-and-lock A1, I3).

A release app ignores BIR_DATABASE_PATH since A1, so the recipe's installed app would open the owner's live data
folder instead of the database copy. Until it is reworked, the recipe must stop before it starts anything.
"""

import os
import re
import shutil
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
JUSTFILE = (ROOT / "justfile").read_text(encoding="utf-8")


def recipe_body(name):
    start = re.search(rf"^{re.escape(name)}\b.*:\n", JUSTFILE, re.M)
    if start is None:
        raise AssertionError(f"no recipe {name}")
    body = JUSTFILE[start.end():]
    following = re.search(r"^\S", body, re.M)
    return body[: following.start()] if following else body


class MockBirAppTests(unittest.TestCase):
    def test_the_recipe_starts_nothing_and_names_no_database(self):
        body = recipe_body("mock-bir-app")
        self.assertIn("exit 1", body)
        self.assertIn("ignores BIR_DATABASE_PATH", body)
        commands = [line.strip() for line in body.splitlines() if line.strip()]
        self.assertEqual(commands[0], "#!/usr/bin/env bash")
        self.assertTrue(commands[1].startswith('echo "mock-bir-app refused:'), commands[1])
        self.assertEqual(commands[2:], ["exit 1"], "the recipe runs nothing but its refusal")

    @unittest.skipUnless(sys.platform == "darwin" and shutil.which("just"), "the recipe is macOS only")
    def test_the_recipe_refuses_before_launching(self):
        result = subprocess.run(
            ["just", "mock-bir-app", "/nonexistent/bir_data.db", "/nonexistent/eBIRForms.app"],
            cwd=ROOT,
            env={**os.environ},
            text=True,
            encoding="utf-8",
            capture_output=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("mock-bir-app refused", result.stderr)


if __name__ == "__main__":
    unittest.main()
