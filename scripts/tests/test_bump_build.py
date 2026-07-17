from __future__ import annotations

import contextlib
import importlib.util
import io
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "bump_build.py"
SPEC = importlib.util.spec_from_file_location("bump_build", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
bump_build = importlib.util.module_from_spec(SPEC)
with mock.patch.dict(sys.modules, {"jwt": mock.Mock(), "requests": mock.Mock()}):
    SPEC.loader.exec_module(bump_build)


class BumpBuildTests(unittest.TestCase):
    def test_print_next_does_not_modify_tracked_build_source(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            justfile = root / "justfile"
            original = 'BUILD_NUMBER := env_var_or_default("BUILD_NUMBER", "26")\n'
            justfile.write_text(original, encoding="utf-8")
            previous = Path.cwd()
            try:
                os.chdir(root)
                stdout = io.StringIO()
                stderr = io.StringIO()
                with (
                    mock.patch.object(bump_build, "get_latest_build", return_value=41),
                    contextlib.redirect_stdout(stdout),
                    contextlib.redirect_stderr(stderr),
                ):
                    result = bump_build.main(["--print-next"])
            finally:
                os.chdir(previous)

            self.assertEqual(result, 0)
            self.assertEqual(stdout.getvalue(), "42\n")
            self.assertIn("Fetching latest build number", stderr.getvalue())
            self.assertEqual(justfile.read_text(encoding="utf-8"), original)

    def test_app_recipe_passes_ephemeral_counter_into_strict_package_recipe(self) -> None:
        justfile = (SCRIPT_PATH.parents[1] / "justfile").read_text(encoding="utf-8")
        app_recipe = justfile.split('app *args="":', 1)[1].split(
            "# Build the Inno Setup", 1
        )[0]

        self.assertIn("scripts/bump_build.py --print-next", app_recipe)
        self.assertIn('BUILD_NUMBER="$NEXT_BUILD_NUMBER"', app_recipe)
        self.assertIn('just _package-mac-appstore', app_recipe)
        self.assertNotIn('app *args="": bump-build', justfile)
        self.assertIn(
            '_package-mac-appstore args="": build-packaged-form-renderer',
            justfile,
        )


if __name__ == "__main__":
    unittest.main()
