"""`python3 scripts/e2e <target> [suite]`: the app's end-to-end suites, run through gpui-agent recipes.

See runner.py for what a run does and docs/proof/E2E.md for how a machine uses it.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from runner import main  # noqa: E402

if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
