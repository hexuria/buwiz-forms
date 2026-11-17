from __future__ import annotations

import re
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
BIR_WORKFLOW_PATH = REPOSITORY_ROOT / ".github/workflows/bir.yml"


class BirCliReleasePolicyTests(unittest.TestCase):
    def test_cli_releases_never_become_the_latest_release(self) -> None:
        # /releases/latest is the desktop app (the Linux installer and the
        # download links resolve it). A bir-v* release published with
        # GitHub's default make_latest would replace it.
        workflow = BIR_WORKFLOW_PATH.read_text(encoding="utf-8")
        steps = re.split(r"(?m)^      - ", workflow)
        releases = [step for step in steps if step.startswith("uses: softprops/action-gh-release@")]
        self.assertEqual(len(releases), 1, "bir.yml should publish exactly one GitHub release")
        self.assertRegex(releases[0], r"(?m)^          make_latest: false\s*$")


if __name__ == "__main__":
    unittest.main()
