"""scripts/ci/test_failure_summary.py: failing-test names in the CI job summary."""

from __future__ import annotations

import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "scripts/ci/test_failure_summary.py"
spec = importlib.util.spec_from_file_location("test_failure_summary", MODULE_PATH)
assert spec is not None and spec.loader is not None
test_failure_summary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(test_failure_summary)


class ExtractionTests(unittest.TestCase):
    def test_python_failure_and_error_names_are_read_once_without_color(self):
        text = (
            "test_one (scripts.tests.test_x.TestX) ... FAIL\n"
            "\x1b[31mFAIL\x1b[0m: test_one (scripts.tests.test_x.TestX)\n"
            "ERROR: test_two (scripts.tests.test_y.TestY)\n"
            "FAIL: test_one (scripts.tests.test_x.TestX)\n"
        )

        self.assertEqual(test_failure_summary.python_failures(text), [
            "test_one (scripts.tests.test_x.TestX)",
            "test_two (scripts.tests.test_y.TestY)",
        ])

    def test_rust_failure_names_are_read_from_every_binary_without_color(self):
        text = (
            "test bir_print::print_missing_values ... \x1b[31mFAILED\x1b[0m\n"
            "test result: FAILED. 1 passed; 1 failed\n"
            "test src/lib.rs - docs::example (line 12) ... FAILED\n"
            "test bir_core::ok_test ... ok\n"
        )

        self.assertEqual(test_failure_summary.rust_failures(text), [
            "bir_print::print_missing_values",
            "src/lib.rs - docs::example (line 12)",
        ])


class SummaryTests(unittest.TestCase):
    def test_markdown_lists_every_failing_test_name_from_both_logs(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            python_log = root / "python.txt"
            rust_log = root / "rust.txt"
            python_log.write_text("FAIL: test_py (scripts.tests.test_x.TestX)\n", encoding="utf-8")
            rust_log.write_text("test bir_print::print_missing_values ... FAILED\n", encoding="utf-8")

            markdown = test_failure_summary.markdown(python_log, rust_log)

        self.assertIn("## Failing test names", markdown)
        self.assertIn("### Python script tests", markdown)
        self.assertIn("- `test_py (scripts.tests.test_x.TestX)`", markdown)
        self.assertIn("### Rust workspace tests", markdown)
        self.assertIn("- `bir_print::print_missing_values`", markdown)
        self.assertNotIn("none found", markdown)

    def test_a_missing_log_and_a_log_without_failures_are_explicit(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            python_log = root / "python.txt"
            rust_log = root / "rust.txt"
            python_log.write_text("test_one ... ok\n", encoding="utf-8")

            markdown = test_failure_summary.markdown(python_log, rust_log)

        self.assertIn("### Python script tests\n\n- none found in the captured output", markdown)
        self.assertIn(f"### Rust workspace tests\n\n- log missing: `{rust_log}`", markdown)

    def test_the_command_appends_to_github_step_summary_and_exits_zero(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            python_log = root / "python.txt"
            rust_log = root / "rust.txt"
            summary = root / "summary.md"
            python_log.write_text("ERROR: test_py (scripts.tests.test_x.TestX)\n", encoding="utf-8")
            rust_log.write_text("test bir_core::broken ... FAILED\n", encoding="utf-8")
            env = dict(os.environ, GITHUB_STEP_SUMMARY=str(summary))

            result = subprocess.run(
                [sys.executable, str(MODULE_PATH), str(python_log), str(rust_log)],
                env=env, capture_output=True, text=True, check=False,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            written = summary.read_text(encoding="utf-8")
        self.assertIn("- `test_py (scripts.tests.test_x.TestX)`", written)
        self.assertIn("- `bir_core::broken`", written)


if __name__ == "__main__":
    unittest.main()
