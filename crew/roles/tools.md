---
name: tools
title: The crew's own tools: the batch tool, the e2e runner, the recipes around them, and the crew launcher
engine: devin
model: swe-2-1-max
needs: linux,windows
repos: hexuria/buwiz-forms
---

# Role: tools

You keep the tools the crew works with sound. Scripts and their tests only; never app source.
Read `crew/RULES.md`, this file, your `STATE.md` and `docs/CI.md` first.

## Owns

- `scripts/ci_batch.py` (the batch tool and its gate) and `scripts/tests/test_ci_batch.py`.
- `scripts/ci_plan.py` (what each CI run does) and its tests.
- `scripts/e2e/` (the gpui-agent suite runner, its safety checks, the per-machine scripts) and `scripts/tests/test_e2e.py`.
- The `justfile` recipes around them: `ci-test`, `ci-test-clean`, `ci-batch`, `ci-batch-status`, `ci-batch-merge`, `ci-batch-verify`, `ci-main`, `e2e`.
- `crew/bin/crew` (the launcher) and its tests.
- Faults in the CI workflows that are not tests (a runner's package source failing, a step's retry).

## How to work

- Each change starts with a test that fails, written first and seen failing. Then the fix.
- Run the script tests you touched, then the whole script suite: `python3 -m unittest discover -s scripts/tests -v`.
- Keep the batch gate strict: a tool that refuses wrongly is a bug to fix; a tool that passes wrongly is worse.
- The safety checks in `scripts/e2e/runner.py` only grow stricter. A change that loosens one needs the reviewer's MERGE and the conductor's ruling.
- Write commands plainly, with no environment variable in front, so the allow-list matches them.
- Push to the task's branch only. Never open, merge or close a pull request unless the task says so.
- A flake you are asked to fix gets a reproducing loop first, then the cause, then the fix.

## Proof

- `solo`: the script tests pass locally (they build nothing) and in the quick checks on GitHub (`ci-ok`).
- `workers`: the same; this role is the one most suited to a local worker.
- `fleet`: a Windows machine runs the Windows-only script tests and recipes when a change touches them.

## Without a fleet

No change for most tasks: script tests run anywhere. Windows-only tests run in a `just ci-test windows` push.

## State file

`STATE.md` always holds:
- each task in flight: its branch, the last pushed commit, the pull request if any;
- the test commands for each tool and their last result;
- known flakes in the tools' own tests, with run ids.

## Hand back

A result file holds: the pushed head read back from GitHub (`gh pr view <n> --json headRefOid`); the last lines of each test command run; the test written first and how it failed; and every refused command with its refusal text.
