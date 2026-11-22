import importlib.util
import os
import shlex
import sys
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPOSITORY_ROOT / "agent-recipes" / "generated_recipe_runs.py"
SPEC = importlib.util.spec_from_file_location("generated_recipe_runs", SCRIPT_PATH)
GENERATED_RECIPE_RUNS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GENERATED_RECIPE_RUNS)


@unittest.skipUnless(os.name == "posix", "uses POSIX shell metacharacters")
class GeneratedRecipeRunsTests(unittest.TestCase):
    def test_shot_command_does_not_execute_case_name_as_shell(self):
        with tempfile.TemporaryDirectory() as directory:
            pwned = Path(directory) / "pwned"
            output = Path(directory) / f"x; touch {pwned}"
            command = f"{shlex.quote(sys.executable)} -c pass {{out}}"

            GENERATED_RECIPE_RUNS.run_shot_cmd(command, str(output))

            self.assertFalse(pwned.exists())

    def test_shot_command_keeps_its_own_braces(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "shot.png"
            script = "import sys; open(sys.argv[2], 'w').write(sys.argv[1])"
            command = " ".join(
                shlex.quote(part)
                for part in (sys.executable, "-c", script, '{"x":1} {} {0}', "{out}")
            )

            ran = GENERATED_RECIPE_RUNS.run_shot_cmd(command, str(output))

            self.assertEqual(ran.returncode, 0)
            self.assertEqual(output.read_text(), '{"x":1} {} {0}')

    def test_shot_command_reports_its_exit_code(self):
        command = f"{shlex.quote(sys.executable)} -c 'raise SystemExit(3)' {{out}}"

        ran = GENERATED_RECIPE_RUNS.run_shot_cmd(command, "unused.png")

        self.assertEqual(ran.returncode, 3)


if __name__ == "__main__":
    unittest.main()
