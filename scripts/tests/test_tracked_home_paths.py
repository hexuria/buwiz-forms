from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from scripts.tracked_home_paths import scan_text  # noqa: E402


def joined(*parts: str) -> str:
    return "".join(parts)


class TrackedHomePathTests(unittest.TestCase):
    def test_positive_control_windows_home_path_is_flagged(self):
        path = joined("C", ":\\Users\\", "example", "user", "\\x")
        self.assertTrue(scan_text(f"fixture {path}"))

    def test_positive_control_json_escaped_windows_home_path_is_flagged(self):
        path = joined("C", ":\\\\Users\\\\", "example", "user", "\\\\x")
        self.assertTrue(scan_text('{"path":"' + path + '"}'))

    def test_posix_home_path_is_flagged(self):
        path = joined("/Users/", "example", "user", "/x")
        self.assertTrue(scan_text(f"fixture {path}"))

    def test_placeholders_are_allowed(self):
        clean = "\n".join(
            [
                r"%USERPROFILE%\AppData\Local\Temp\x",
                r"C:\Users\<name>\x",
                "/Users/<name>/x",
                "/home/<name>/x",
                "$HOME/x",
                "relative/path/x",
            ]
        )
        self.assertFalse(scan_text(clean))

    def test_ci_account_names_are_not_placeholders(self):
        windows = joined("C", ":\\Users\\", "runner", "admin", "\\x")
        posix = joined("/home/", "ubu", "ntu", "/x")
        self.assertTrue(scan_text(f"fixture {windows}"))
        self.assertTrue(scan_text(f"fixture {posix}"))

    def test_curly_interpolation_is_not_a_literal_home_owner(self):
        self.assertFalse(scan_text("built in /Users/{USER}/project"))


if __name__ == "__main__":
    unittest.main()
