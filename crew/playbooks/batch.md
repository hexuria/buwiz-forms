# Playbook: a batch

A batch proves several ready pull requests together, on exactly the tree `main` will have once they are all merged, in one complete run. Then they merge in that order. Only the conductor batches and merges.
The tool is `scripts/ci_batch.py` through the `just ci-batch*` recipes; `docs/CI.md` ("Proving several pull requests together") is its full description.

## 1. The entry gate

A pull request enters a batch only when all of these hold. Check each one at the head you read from GitHub now.

- It is open, not a draft, and based on `main`.
- The reviewer's verdict is MERGE, or MERGE AFTER whose named items have all landed.
- `ci-ok` is green at its exact head. The tool refuses it otherwise.
- If it touches the app, its full test suite passed at that exact head (a `just ci-test` run, or a machine's run), read job by job, with the run id. `ci-ok` is only the quick checks.
- Nobody expects to push to it again. Any push voids the batch.

Leave out anything that fails a line. Say which line in `LOG.md`.

## 2. Dry run

```sh
just ci-batch --dry-run <pr> <pr> ...
```

It builds the batch's tree in order and pushes nothing. A conflict names the pull request and the files: take that one out, or reorder, and dry-run again. Put pull requests that touch the same files next to each other so the conflict, if any, shows here.

## 3. Build

```sh
just ci-batch <pr> <pr> ...            # Linux and Windows
just ci-batch all <pr> <pr> ...        # every system
```

It pushes `test/<system>/batch-<pr>-<pr>...`, which starts the full suites. Write the branch and each head into `queue.json` under the batch. From now on nobody pushes to those pull requests and nothing else merges into `main`.

## 4. Watch

Run `crew batchwatch <branch> <pr> <pr> ...` in the background. It returns when the batch settles. `just ci-batch-status <branch>` shows the state at any time.

## 5. Read the results

Green or red is not a result. Open each workflow run of the batch and read each job: its name, its conclusion, and for a test job the counts.

- **Red, a known flake.** The failing test matches an entry in `KNOWN-FLAKES.md` in the state folder (the test name, the error text and the system; `crew/playbooks/state.md`). Re-run the failed job once (`gh run rerun <run id> --failed`). Record the run id and the outcome under that entry. A second red of the same flake is no longer a flake: treat it as below.
- **Red, anything else.** `just ci-batch-status <branch>` prints the two halves to run. Run both; repeat on the red half until one pull request is left. That pull request goes back to its author with the failing job, the test and the message. The rest form a new batch.
- **A new flake.** A red that passes on re-run, in a file none of the pull requests touch, gets a new entry: the test, where it was seen (run id and job), the cause if known, and what to do until it is fixed. Then a task to fix it goes to the owning role.

## 6. A batch that needs a log read before merge

Some greens still need a look at the log before you merge:
- a job that passed only on a re-run;
- a pull request whose claim is that a named test now runs: check it ran and was not skipped;
- a job whose number of tests dropped against the last run on `main`.

Read the lines that settle it, and put them in `LOG.md` with the run id.

## 7. Merge at the tested heads

```sh
just ci-batch-merge <branch>
```

It refuses unless the batch is green and `main` and every head are still as recorded, then prints one `gh pr merge ... --squash --match-head-commit <sha>` per pull request. Run them in that order, each with an explicit `--body` so no `Co-authored-by` line is added. Merge nothing else in between.

## 8. Verify

```sh
just ci-batch-verify <branch>
```

Equal: `main` is the tree the batch proved. Not equal: the proof does not cover `main`; run `just ci-main` and read it. Then clear the batch from `queue.json`, write a `LOG.md` line with the merged numbers and `main`'s new sha, and clean the branch with `just ci-test-clean`.
