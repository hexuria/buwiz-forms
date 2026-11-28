"""nsh/agent/ios-handoff.sh: the token hand-off's time budget is seconds, not a raw loop count."""

import os
import shlex
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "nsh/agent/ios-handoff.sh"


@unittest.skipIf(sys.platform == "win32", "the iOS hand-off is a bash script for the Mac")
class IosHandoffBudgetTests(unittest.TestCase):
    def attempts_without_container(self, pause, wait_seconds=None, fast_sleep=False):
        """Run the hand-off with no host container and answer how many times it asked xcrun for one."""
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            bin_dir = tmp / "bin"
            bin_dir.mkdir()
            calls = tmp / "xcrun-calls"
            xcrun = bin_dir / "xcrun"
            xcrun.write_text(f"#!/bin/sh\necho call >> {shlex.quote(str(calls))}\nexit 1\n", encoding="utf-8")
            xcrun.chmod(xcrun.stat().st_mode | stat.S_IEXEC)
            if fast_sleep:
                sleep = bin_dir / "sleep"
                sleep.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
                sleep.chmod(sleep.stat().st_mode | stat.S_IEXEC)
            launcher = subprocess.Popen(["sleep", "30"])
            env = {name: value for name, value in os.environ.items() if not name.startswith("NSH_AGENT_TOKEN_")}
            env.update({"PATH": f"{bin_dir}:{os.environ['PATH']}", "GPUI_AGENT_TOKEN": "token",
                        "NSH_AGENT_TOKEN_PAUSE": pause})
            if wait_seconds is not None:
                env["NSH_AGENT_TOKEN_WAIT_SECONDS"] = wait_seconds
            try:
                result = subprocess.run(["bash", str(SCRIPT), "token", str(launcher.pid)], env=env,
                                        text=True, capture_output=True, timeout=10)
            finally:
                launcher.kill()
                launcher.wait()
            self.assertNotEqual(result.returncode, 0, result.stderr)
            self.assertIn("token was not stored", result.stderr)
            return len(calls.read_text().splitlines())

    def test_the_default_budget_is_900_seconds(self):
        # Three no-op sleeps of 300 seconds each are 900 seconds of budget without the test waiting for them.
        self.assertEqual(self.attempts_without_container("300", fast_sleep=True), 3)

    def test_tries_are_the_ceiling_of_seconds_divided_by_the_pause(self):
        for seconds, pause, expected in (("0.025", "0.01", 3), ("0.03", "0.01", 3), ("0.031", "0.01", 4)):
            with self.subTest(seconds=seconds, pause=pause):
                self.assertEqual(self.attempts_without_container(pause, seconds), expected)


if __name__ == "__main__":
    unittest.main()
