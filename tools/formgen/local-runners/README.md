# Local formgen runners

This directory is reserved for machine-local launchers and their output. It
keeps commands that must be run from Terminal.app inside the formgen worktree
without making those temporary files part of the generated-form corpus.

Only this README and `.gitignore` are tracked. Launchers, logs, JSON reports,
status files, and exit markers placed here are ignored by Git.

Each full-gate launcher must:

- pin and verify the exact branch HEAD;
- refuse a dirty worktree;
- run only `tools/formgen/gate.py`, never a concurrent `batch.py` or `audit.py`;
- publish its exit marker atomically; and
- keep its report and log in this directory.

Temporary runners are local coordination aids, not promotion or release
evidence. The full gate's fresh, evaluable result remains the done-condition.
