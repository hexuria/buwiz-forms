"""nsh/agent/ios-handoff.sh: the iOS hand-off of the window plane's token (#449).

The script runs against a stub `xcrun` that answers a temporary folder as the host's data container, while a short
`sleep` stands in for the launcher it outlives no longer than.
"""

import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "nsh/agent/ios-handoff.sh"
TOKEN = "this-runs-token"


@unittest.skipIf(sys.platform == "win32", "the iOS hand-off is a bash script for the Mac")
class IosHandoffTests(unittest.TestCase):
    def hand_off(self, before):
        """Run the hand-off once with the token file holding `before` (None: no file); answer the file afterwards."""
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            data = tmp / "container"
            caches = data / "Library/Caches"
            caches.mkdir(parents=True)
            token_file = caches / "gpui-agent-token"
            if before is not None:
                token_file.write_text(before)
            bin_dir = tmp / "bin"
            bin_dir.mkdir()
            xcrun = bin_dir / "xcrun"
            xcrun.write_text(f'#!/bin/sh\nprintf "%s\\n" "{data}"\n')
            xcrun.chmod(xcrun.stat().st_mode | stat.S_IEXEC)
            # A launcher that is not this process's child, so it is reaped the moment it ends (a child would stay a
            # zombie that `kill -0` still finds until this test waited for it).
            launcher = subprocess.run(["sh", "-c", "sleep 1 >/dev/null 2>&1 & echo $!"], text=True,
                                      capture_output=True, check=True).stdout.strip()
            result = subprocess.run(
                ["bash", str(SCRIPT), "token", launcher],
                env={**os.environ, "PATH": f"{bin_dir}:{os.environ['PATH']}", "GPUI_AGENT_TOKEN": TOKEN,
                     "NSH_AGENT_TOKEN_TRIES": "40", "NSH_AGENT_TOKEN_PAUSE": "0.1"},
                text=True, capture_output=True, timeout=30,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            staged = sorted(p.name for p in caches.iterdir() if p.name != "gpui-agent-token")
            self.assertEqual(staged, [], "nothing staged is left behind")
            mode = stat.S_IMODE(token_file.stat().st_mode) if token_file.exists() else None
            return (token_file.read_text() if token_file.exists() else None), mode

    def test_an_absent_token_is_written_privately(self):
        text, mode = self.hand_off(None)
        self.assertEqual(text, TOKEN)
        self.assertEqual(mode, 0o600)

    def test_this_runs_token_is_left_as_it_is(self):
        text, _ = self.hand_off(TOKEN)
        self.assertEqual(text, TOKEN)

    def test_a_finished_runs_token_is_replaced(self):
        # #449: the file held a token from a run that ended, so the host never got this run's and the run waited.
        text, mode = self.hand_off("a-finished-runs-token")
        self.assertEqual(text, TOKEN)
        self.assertEqual(mode, 0o600)


if __name__ == "__main__":
    unittest.main()
