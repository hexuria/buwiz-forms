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


if __name__ == "__main__":
    unittest.main()
