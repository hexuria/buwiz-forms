# Playbook: a batch

A batch proves several ready pull requests together, on exactly the tree `main` will have once they are all merged, in one complete run. Then they merge in that order. Only the conductor batches and merges.
The tool is `scripts/ci_batch.py` through the `just ci-batch*` recipes; `docs/CI.md` ("Proving several pull requests together") is its full description.

## 1. The entry gate

A pull request enters a batch only when it is **ready** as `crew/RULES.md` ("Ready, merge and tag") defines it, checked at the head you read from GitHub now, and when:

- it is open, not a draft, and based on `main`;
- nobody expects to push to it again (any push voids the batch).

The batch tool refuses a pull request whose `ci-ok` is not green at its head; it does not check the suites or the verdict, so you do. Leave out anything that fails a line, and say which line in `LOG.md`.

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

## 4. Watch, without merging

```sh
python3 crew/bin/crew batchwatch --no-merge <branch> <pr> <pr> ...
```

Run it in the background. It returns when the batch settles and merges nothing. `just ci-batch-status <branch>` shows the state at any time.

## 5. Read every job

Green or red is not a result. Open both workflow runs of the batch's commit (`CI` and `NSH app`) and read every job: its name, its conclusion, and for a test job the counts. Also read any log line a pull request's review asked for.

- **Red, a known flake.** The failing test matches an entry in `KNOWN-FLAKES.md` in the state folder (the test name, the error text and the system; `crew/playbooks/state.md`). Re-run the failed job once (`gh run rerun <run id> --failed`). Record the run id and the outcome under that entry. A second red of the same flake is no longer a flake: treat it as below.
- **Red, anything else.** `just ci-batch-status <branch>` prints the two halves to run. Run both; repeat on the red half until one pull request is left. That pull request goes back to its author with the failing job, the test and the message. The rest form a new batch.
- **A new flake.** A red that passes on re-run, in a file none of the pull requests touch, gets a new entry: the test, where it was seen (run id and job), the cause if known, and what to do until it is fixed. Then a task to fix it goes to the owning role.

Some greens still need a log read before merge:
- a job that passed only on a re-run;
- a pull request whose claim is that a named test now runs: check it ran and was not skipped;
- a job whose number of tests dropped against the last run on `main`.

Put the lines that settle each one in `LOG.md` with the run id. Merge only when every job is read.

## 6. Merge at the tested heads

```sh
python3 crew/bin/crew batchwatch <branch> <pr> <pr> ...
```

Without `--no-merge`, on a green batch it runs the merge commands `just ci-batch-merge <branch>` prints, in order, then verifies. The launcher merges only when `crew.local.ini` says `[crew] merge = yes` and each pull request has a verdict file of MERGE at its recorded head (`crew/playbooks/review.md`). Without `merge = yes`, it prints the commands and merges nothing; whoever may merge runs them in that order, each with an explicit `--body`.

`just ci-batch-merge <branch>` refuses unless the batch is green and `main` and every head are still as recorded. Each printed command carries `--match-head-commit <sha>`. Merge nothing else in between. If one merge fails, stop: note which pull requests merged and which did not, and verify nothing until that is settled.

## 7. Verify

```sh
just ci-batch-verify <branch>
```

Equal: `main` is the tree the batch proved. Not equal: the proof does not cover `main`; run `just ci-main` and read it. Then clear the batch from `queue.json` and write a `LOG.md` line with the merged numbers and `main`'s new sha. Delete only this batch's branch: `just ci-test-clean` removes every finished test branch, other roles' unmerged batches included.
