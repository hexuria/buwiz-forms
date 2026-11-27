# Playbook: writing a worker task

A worker starts with no memory and no chat. It gets its role file, its `STATE.md` and one task file, and runs unattended. So the task file must hold everything the work needs.

## Where it goes

`roles/<role>/inbox/NNN-<name>.md` in the state folder, numbered after the role's last task. Start it from the repository root, exactly as the launcher takes it:

```sh
python3 crew/bin/crew run <role> <task-file> --worktree DIR
python3 crew/bin/crew run tools .crew/state/roles/tools/inbox/014-batch-draft-reason.md --worktree ../buwiz-forms-draft-reason
```

The task file path is relative to the repository root, or absolute; with the default state folder that is `.crew/state/roles/<role>/inbox/NNN-<name>.md` (`python3 crew/bin/crew state path` prints the folder). `--worktree` is the worker's own git worktree, made for the task (for example `git worktree add ../buwiz-forms-draft-reason -b fix/batch-draft-reason origin/main`). A worker never runs in the conductor's checkout: the launcher refuses that. Inside its worktree it does not edit `crew/bin/` or git's own files. The worker writes its result to `roles/<role>/outbox/NNN-<name>.md`. Add `--sandbox` only for work the sandbox suits (`crew/playbooks/sandbox.md`).

Never point `--worktree` at a checkout of a pull request from outside the project: its files can carry agent settings the worker would load.

## What a task file holds

1. **A title line:** the number and what the task changes, in one line.
2. **Where:** the worktree folder and the branch, and whether the branch exists or the worker creates it from `main`. One branch per task.
3. **Which files** the worker may change. Name them. Anything else is out of scope.
4. **What to do,** as numbered points. Each point says what is true when it is done, with the reason in one line.
5. **Tests first:** which test to add for each point, and that it must be seen failing before the fix.
6. **Commands to run,** each a plain single command with no environment variable in front, so the allow-list matches it. A worker runs only git, python3, read-only shell tools, the read-only `gh` subset and `python3 crew/bin/crew push`. A step that needs a `just` recipe, cargo, `nsh` or a test run on GitHub is not a worker task: make it a machine order or the conductor's step. Give the exact test commands and what their last line must say.
7. **The commit:** the identity the conductor's settings name (`[crew] commit_identity`, `crew/RULES.md`), checked with `git log -1 --format='%an <%ae> | %cn <%ce>'`; then `python3 crew/bin/crew push`, which pushes the worker's own branch only and refuses a commit whose author or committer is not that identity.
8. **What not to do,** only where a worker could reasonably do it: open, merge or close pull requests; touch another branch.
9. **The result file:** what it must contain.

## What a result file holds

- The pushed head, read back from GitHub (not the local sha).
- For each test command, its last lines.
- For each test written first, how it failed before the fix.
- Every command that was refused, with the refusal text. A refusal is final; the worker goes on.
- What it could not do, and why.

## Keep it small

A task a worker can finish in one run. Several small tasks run in parallel; one large task fails late. When a task needs a build, an app run or a device, it is a machine order instead (`crew/playbooks/machine-order.md`).

## A worked example

```markdown
# Task 014: the batch tool says why a draft is refused

Where: worktree <worktree>, branch `fix/batch-draft-reason` (create it from `origin/main`).
Files: `scripts/ci_batch.py`, `scripts/tests/test_ci_batch.py` only.

1. When `build` refuses a draft, the message reads `#<n> is a draft (a draft runs no checks):
   mark it ready first`, so the reader knows why without opening docs/CI.md.
2. No other refusal changes.

Tests first: add `test_build_refusal_of_a_draft_says_why` to `scripts/tests/test_ci_batch.py`.
Run it and see it fail before you change `ci_batch.py`.

Run, in this order:
- `python3 scripts/tests/test_ci_batch.py`: the last line must be `OK`.
- `python3 -m unittest discover -s scripts/tests`: the last line must be `OK`.

Commit with the identity your settings name; check `git log -1 --format='%an <%ae> | %cn <%ce>'`.
Then `python3 crew/bin/crew push` (it pushes `fix/batch-draft-reason` only).
Do not open, merge or close any pull request.

Result file: the pushed sha read back with `git ls-remote origin fix/batch-draft-reason`,
the last lines of both test commands, how the new test failed first, and anything refused.
```
