# CI

Pull requests run quick checks, on Linux. The test suites run when someone asks for them: with a `ci:suite*`
label on the pull request, by pushing the commit to a `test/...` branch, by hand, and every night on `main`.
The tests themselves are not slow. The GitHub queue is,
and we were the ones filling it: about 20 jobs for each pull request, with macOS jobs waiting hours for a runner.

`scripts/ci_plan.py` decides what each run does, for both `.github/workflows/ci.yml` and
`.github/workflows/nsh.yml`. `scripts/tests/test_ci_plan.py` tests those rules. This page says the same thing in
words.

## What runs when

| Event | `ci.yml` | `nsh.yml` | Systems |
| --- | --- | --- | --- |
| Pull request opened or pushed to (into `main`, `feat/**`, `gol/**`, `codex/**`) | quick checks | nothing | Linux |
| `ci:suite*` label added to a pull request | quick checks and the chosen suites | the chosen app suites | from the label |
| Any other label added | nothing | nothing | none |
| Draft pull request (opened, or pushed to, as a draft) | nothing, except that a `ci:suite*` label still runs | nothing, except the same label | none, except the label systems |
| Draft marked ready for review | quick checks | nothing | Linux |
| Pull request closed or merged | nothing; cancels its queued and running runs | nothing | none |
| Push to `main` (a merge) | quick checks, never cancelled | nothing | Linux |
| Push to `codex/**` | quick checks | nothing | Linux |
| Push to `test/**` | workspace tests, Windows installer | app build, app Rust tests, screen tests | from the branch name |
| Manual run | what the inputs ask | what the inputs ask; `build` with Linux also builds the Linux release packages | from the inputs |
| Nightly, 18:00 UTC, on `main` | everything | everything, the Linux release packages included | all three |

The release workflows no longer run on pull requests. A `v*` tag still builds, signs and publishes as before.
`release-macos.yml` runs only for a tag or a manual run, and a manual run never publishes. `release-linux.yml` runs when `release.yml` calls it for a tag, when someone runs it by
hand, or when `nsh.yml` calls it (nightly, or a manual run whose suites include `build` on Linux). The `nsh.yml` call
passes an empty tag (version `<app>-pr`) and no secrets. That workflow only builds, smoke-tests and keeps workflow
artifacts; publishing is `release.yml`'s job.

The quick checks are:

- `quick checks (ubuntu-22.04)`: the Python script tests, `just --summary`, `cargo fmt --all -- --check`,
  `cargo clippy --locked --workspace -- -D warnings`, clippy of bir-core with `direct-bir`, the check that nothing
  shipped enables `direct-bir`, the frozen HTML inventory and TIN stamps, and the production-boundary check
  (`bir-rules-codegen -- status --boundaries-only`).
- `NSH app type check (Linux)`: `just nsh-check -v`, which checks the app's crate and then its TSX types, plus the
  check that the app does not enable `direct-bir`.
- `NSH app agent compile check (Linux)`: on a pull request that moves `nsh/Cargo.toml`, `nsh/Cargo.lock` or an
  agent-only app file, compiles the app and Linux plug with `--features agent --no-run`; skipped otherwise.
  It compiles the Linux arm only; the other platforms stay covered by the complete suite.
- `legacy-guard`.
- The validation-rules audits on Linux, only when their paths change (see the table below).
- `changes` decides all of this. `ci-ok` sums it up.

Every Linux `apt-get update` goes through `scripts/ci/apt_update.sh`: it sets aside the third-party source lists
the runner image ships and we do not use, then retries Ubuntu's own sources, so their outage cannot fail the job.

`ci-ok` is the one required status. It fails if any job it asked for did not pass, and it is the check a reader
looks at for the quick gate. A cancelled quick run (a newer push replaced it, or the pull request closed) reports no
`ci-ok`. A `test/**` push, a manual run and the nightly run report `tests-ok` in `ci.yml` instead, so a test run on
a pull request's commit never adds a second `ci-ok` result. A `ci:suite*` run reports both: `ci-ok` because the quick
checks are part of it, and `tests-ok` in both workflows for the suite proof. A reader judging the suites looks at
`tests-ok`; it is red when a planned suite job failed or was cancelled.

No GitHub macOS job starts by itself. macOS runs only for `ci:suite-macos`, `ci:suite-all`, `test/macos/**`,
`test/all/**`, a manual run that names macOS, and the nightly run. Proof for the Mac normally comes from the Devin Macs.

## Drafts and closed pull requests

A draft pull request runs nothing unless someone adds a `ci:suite*` label to it; that label is an explicit request
and runs even on the draft. Otherwise every job of `ci.yml` is skipped before it takes a runner, so the pull request's
checks list shows only skipped jobs, and none is red. A skipped job's name is still evaluated. The planner's lane is
the first input to those names: a lane-`none` run names its quick gate `label-ignored` whatever spelling its label
had, a draft's other empty quick gate is `not-ready`, and each workflow's separate suite-proof job is named
`tests-ok` only in the suite lane. Neither `ci-ok` nor `tests-ok` is reported as a skipped success by an empty run.
Marking the pull request ready for review starts the quick checks, and their `ci-ok` is the one that counts. After
that, every push runs them as before.

Open a pull request as a draft while you are still pushing to it, and mark it ready when you want its checks
(`gh pr create --draft`, then `gh pr ready`). Turning a ready pull request back into a draft does not stop the run
already going. The next push to it does: that push runs nothing, and it cancels the older run.

When a pull request closes or merges, its `closed` event cancels the pull request's runs that are still queued or
running, and runs nothing itself. A pull request's runs share one concurrency group, keyed on its number, and a new
run in that group cancels the older ones. A label that is not a suite label is the exception: its empty run gets a
group of its own and cancels nothing. The `closed` run's skipped quick gate is named `closed`, and neither it nor a
cancelled run reports a check named `ci-ok`, so nothing turns red: the pull request keeps the `ci-ok` of its last
finished run, if it had one.

This does not reach `test/**` runs. Those are pushes to their own branch, in that branch's group. A newer push to the
same `test/...` branch replaces its run, and `gh run cancel <run id>` stops one. `just ci-test-clean` deletes the
branches whose runs have all finished. It keeps the busy ones, and deleting a branch does not stop its runs anyway.

## Asking for the suites on a pull request

```sh
just ci-suite 471            # ci:suite, Linux and Windows
just ci-suite 471 linux      # or windows, macos, all
```

The labels are `ci:suite` (Linux and Windows), `ci:suite-linux`, `ci:suite-windows`, `ci:suite-macos` and
`ci:suite-all`; as in GitHub's workflow expressions, the planner matches those names by ordinal-ignore-case
comparison (the upper-case form) and logs the spelling the event carried. The recipe creates the label in the
repository when needed, removes it first when it is already there, adds it, and prints the pull request's checks
link. Removing and adding is what asks again: the `labeled` event is the request, so a pull request that still
carries the label does not rerun the suites on a later push; a new push needs a new ask. The workflows never remove
the label and never need write permission for it, so fork pull requests use the same read-only token rule as any
other pull request.

A suite-label run plans the quick checks and the suites for the label's systems, choosing the suites from the changed
files with the same planner code as a `test/**` push. Its `tests-ok` job in each workflow is the result to read. A
machine's own run at the exact head is also proof and needs no label; keep the machine's result with the pull request
as before. `just ci-test` and `just ci-test-pr` stay for machines and batch branches.

## Asking for tests on a branch

```sh
just ci-test            # Linux and Windows
just ci-test linux      # or windows, macos, all
just ci-test-clean      # afterwards: deletes test/** branches whose runs have all finished
```

`just ci-test` pushes the current commit to `test/<system>/<branch>`. A leading `feat/`, `fix/` or `spike/` is
dropped, so `feat/ci-fast-lane` becomes `test/linux/ci-fast-lane`. With no system it pushes to
`test/linux-windows/<branch>`. It then prints the Actions page for that branch. Checks attach to the commit, so the
results also show on the pull request whose head is that commit, with no new commit needed. Pushing again replaces
the older run.

The push goes to `origin`, or to `$CI_TEST_REMOTE` if that is set. The remote has to accept commits that touch
workflow files: on the owner's Mac that is the SSH alias, and an https token without the `workflow` scope is
refused. Without `just`, for example on a Windows machine:

```sh
git push --force origin HEAD:refs/heads/test/windows/my-branch
```

| Branch | Systems |
| --- | --- |
| `test/linux/**` | Linux |
| `test/windows/**` | Windows |
| `test/macos/**` | macOS |
| `test/all/**` | all three |
| any other `test/<name>` | Linux and Windows |

The cleanup is a recipe, not a CI job. A job that deleted its own branch would need write access to the repository
in CI. It would also break the other workflow's queued jobs on that branch, which still have to check it out.
`just ci-test-clean` uses your own push access and skips every branch that still has a queued or running run.
Deleting a branch does not remove its results from the commit. Batch branches (`test/<system>/batch-...`, see
"Proving several pull requests together") are deleted the same way once their runs finish. On any machine but the
one that built the batch, `just ci-batch-merge` and `just ci-batch-verify` need the branch, so clean up after the
verify.

### Manual runs

Both workflows take the same inputs (Actions tab, Run workflow, or `gh`):

```sh
gh workflow run nsh.yml --repo hexuria/buwiz-forms --ref feat/x \
  -f systems=macos -f suites=screens -f screens='screens/form/**' -f shard=all
gh workflow run ci.yml --repo hexuria/buwiz-forms --ref feat/x -f systems=all -f suites=validation
```

- `systems`: `all`, or a comma list of `linux`, `windows`, `macos`. The default is `linux,windows`.
- `suites`: `all`, or a comma list of these:
  - `rust`: the workspace Rust tests (`ci.yml` `check`), the app crate's Rust tests and the plain-SQLite core
    tests.
  - `screens`: the headless TSX screen tests.
  - `build`: `nsh build`, the agent build, the iPhone and Android compiles, the development host and the
    real-window test. With Linux chosen, it also builds the Linux release packages (`release-linux.yml`, x64 and
    arm64), which never run on a pull request.
  - `installer`: the unsigned Windows Setup build and audit.
  - `validation`: the validation-rules corpus audits.
- `screens`: limits which screen-test files run. A glob such as `screens/form/**` (`*` also crosses folders), or a
  file or folder path such as `screens/form`. The filter is applied first, then the sharding.
- `shard`: `all`, or `0`, `1` or `2` to run one shard of three. File k of the sorted list belongs to shard k % 3,
  so the three shards together run every file exactly once.

A manual run on `main` always runs every suite on one runner per system, whatever the inputs say (see Caches).

## The merge rule

A pull request is ready to merge when both of these hold:

1. The quick checks are green on GitHub (`ci-ok`).
2. The test suites passed at that exact commit, either from a `ci:suite*` label run (`tests-ok` green in both
   workflows), from a `test/...` push (its runs show on the pull request), or from a Devin machine at the pull
   request's head. Or the pull request is in a green batch, merged as the batch
   says (next section).

Whoever GitHub lets merge does: the maintainers, or a crew conductor its maintainer has told to merge, running with
that person's GitHub rights, once the pull request is ready as `crew/RULES.md` defines it (these two checks plus a
reviewer's MERGE at that head). GitHub's permissions and branch protection are the gate; nobody merges with a red or
unread check.

## Proving several pull requests together

Pull requests that touch the same files merge one at a time, and each is proven again with the one before it merged
in: about half a day per merge. A batch (a rollup, as in bors or a GitLab merge train) proves several of them in one
complete run, on exactly the tree `main` will have once they are all merged, and then they merge in that order.

### What a pull request needs before a batch

- Its `ci-ok` check is green at its exact head. `--allow-unchecked <pr>` skips this gate only when the conductor
  means it.
- For every pull request, ci.yml's tests-ok must be green at that exact head, from a pull_request suite run
  or a test-lane push. The proof is judged per needed system: the newest run covering that system across both proof
  sources has the system's jobs present and green, and a run whose planner asked for no test job on that system is
  cheap but still counts when its `tests-ok` is green. If the planner would run the app suite for the changed files,
  `nsh.yml` must pass the same judgment at the exact head too: the system's app jobs are present, none is skipped,
  and every shard is green.
  Different systems may be proved by different runs, but a manual run narrowed by a suite, screen or shard filter is
  partial proof and does not count. When either part is missing or red, the refusal prints `just ci-suite <n>`.
  `just ci-test-pr <n> [system]` remains the branch-push form: it reads the pull request's head from GitHub, fetches
  exactly that object, pushes it to `test/<system>/pr-<n>` and prints the Actions page. `--allow-unsuited <pr>` is
  the single override for this suite gate, only when the conductor means it, and its use is written in the batch's
  record.

Use it for a queue of pull requests that are each reviewed and ready: not drafts, based on `main`, with both gates
green. Leave out a pull request you still expect to push to: any push to it voids the batch.

```sh
just ci-batch 412 415 418           # Linux and Windows; or: just ci-batch all 412 415 418
just ci-batch-status test/linux-windows/batch-412-415-418
just ci-batch-merge  test/linux-windows/batch-412-415-418   # prints the merge commands, runs nothing
just ci-batch-verify test/linux-windows/batch-412-415-418   # after the merges
```

- `just ci-batch [system] <pr> <pr> ...` reads each pull request from GitHub in one read (its full head sha, base,
  draft flag and state) and refuses one that is closed, merged, a draft or not based on `main`, saying which. It then
  starts at `main`'s current commit and applies the squash of each pull request in the order given: the head merged
  onto the running commit with the merge base git finds, committed as one commit, which is what
  `gh pr merge --squash` does once the ones before it are merged. It uses git plumbing only (`git merge-tree`,
  `git commit-tree`) in the checkout's object store: no checkout, index or local branch changes, and nothing is
  written outside the repository. On a conflict it stops, names the pull request and the files, and pushes nothing.
  Otherwise it pushes the result to `test/<system>/batch-<pr>-<pr>...`, which starts the suites like any `test/**`
  push (to `$CI_TEST_REMOTE` or `origin`, as `just ci-test` does). The system is `linux-windows` unless given first
  (`linux`, `windows`, `macos`, `all`); macOS proof normally comes from the Devin Macs. One batch on Linux and Windows
  takes about as many runners as one `just ci-test`, against the account's 20 jobs at a time (5 macOS).
- The branch tip is an empty commit on top of the squashes whose message is the record: `Batch-Main` (main's full
  sha), one `Batch-PR: <number> <full head sha>` per pull request in merge order, `Batch-Tree` (the final tree hash)
  and `Batch-Systems`. The other three commands read only that record. A local copy is kept under
  `refs/ci-batch/`, so on the machine that built the batch they still work after `just ci-test-clean` deleted the
  branch.
- `just ci-batch-status <branch>` says green, red or running for each workflow run of the batch's commit (`CI`,
  `NSH app`), and for red, which jobs failed. It does not guess which pull request broke the batch: it prints the two
  `just ci-batch` commands that run each half of the list, side by side on their own branches. Repeat on the red half.
- `just ci-batch-merge <branch>` refuses unless the batch is green. It reads `main`'s sha and each pull request again
  from GitHub, refuses if any of them moved (or is no longer open, ready and based on `main`), and then prints the
  `gh pr merge <n> --repo hexuria/buwiz-forms --squash --match-head-commit <recorded sha>` commands in order. It runs
  none of them: whoever merges (the maintainer, or the crew's conductor) runs them in that order.
- `just ci-batch-verify <branch>`, after the merges, compares `main`'s tree hash with the batch's `Batch-Tree`: equal
  or not equal. Not equal means the batch's proof does not cover `main`; prove `main` itself (`just ci-main`).

The proof is valid only when all of this holds:

1. When merging, `main` is still the recorded `Batch-Main` and every head is still its recorded sha
   (`just ci-batch-merge` checks both; `--match-head-commit` checks each head again at its own merge).
2. The pull requests merge in the recorded order, as squash merges, with nothing else merged in between.
3. Afterwards `main`'s tree equals `Batch-Tree` (`just ci-batch-verify`).

If any of these fails, the batch proves nothing about `main`: run the batch again, or prove `main`.

## Which paths lead to which suites

A path class only lists files that no crate, build script or test reads. Anything not listed is app code and runs
everything in its lane. For example `assets/linux/` (the desktop-entry test reads it), Markdown under `rules/` or
`scripts/`, crate `README.md` files and `evidence/` are all app code, because tests or code generation read them. So does an unknown or empty change range, except that the validation-rules audits never
start on a pull request just because the range is unknown. A `test/**` push compares against `main`, so it tests
the pull request's whole change. A `ci:suite*` label event compares the same range and uses the same choosing code.

| Changed files | Pull request | `test/**` push |
| --- | --- | --- |
| Docs: `docs/`, top-level `*.md`, `crates/<crate>/docs/`, `.github/*.md`, Markdown under `nsh/`, `assets/brand/`, `assets/badges/`, `assets/readme/` (the README's pictures) | `legacy-guard` only | nothing |
| Screen tests only: `nsh/**/*.test.ts`, `nsh/**/*.test.tsx` | NSH type check | screen tests |
| Other `nsh/` files, except `nsh/src/` and `nsh/Cargo.*` | NSH type check | app build, app Rust tests, screen tests |
| `nsh/src/`, `nsh/Cargo.toml`, `nsh/Cargo.lock` | quick checks, NSH type check | workspace tests, app build, app Rust tests, screen tests |
| The crates the app is built on (`crates/bir-app-api`, `bir-core`, `bir-host-shim`, `bir-print`, `bir-rules`), `forms/`, `Cargo.toml`, `Cargo.lock`, `justfile`, `rust-toolchain.toml`, `scripts/ci_plan.py`, `scripts/nsh_screen_tests.sh` | quick checks, NSH type check | everything above, plus the plain-SQLite core tests (Linux) |
| Windows installer: `assets/windows/`, `packaging/windows/`, `scripts/{audit_final_setup,package_msix,package_nsh_windows,smoke_msix}.ps1`, `scripts/audit_installer_sources.py`, `release-windows.yml` | Python script tests | installer build and audit (Windows only) |
| `.github/workflows/nsh.yml` | Python script tests, NSH type check | installer, app build, app Rust tests, screen tests, core tests |
| `nsh/nsh.json`, `print-overrides/`, `assets/icon*.png`, `scripts/audit_no_legacy.py` | as their class above | the installer too |
| macOS packaging (`assets/macos/`, `*entitlements*.plist`, the two macOS release scripts), `packaging/linux/`, setup scripts, release workflows, `scripts/tests/` | Python script tests | nothing |
| Validation-rules corpus: `rules/`, `crates/bir-rules`, `bir-rules-codegen`, `bir-rules-platform`, `crates/bir-core/src/form_rules/`, `validation_summary.rs`, `evidence/validation-rules/`, `rust-toolchain*`, `ci.yml`, `scripts/ci_plan.py` | the validation-rules audits on Linux, as well as the rest of the row's class | no audits (the pull request ran them) |
| Anything else (app code, `ci.yml` included) | quick checks | workspace tests |

The validation-rules audits also run on Linux for a pull request with the `run-validation` label, or one from an
`ebirforms-*` branch. They run on all three systems only in the nightly run and in a manual run whose suites include
`validation`. They check the extracted rules corpus, a library the app does not use for filing today. The
production-boundary check is a different thing: it stays in the quick checks of every pull request.

## main, the nightly run and caches

- Each merge to `main` runs the quick checks in a concurrency group of its own, so no merge cancels another and none
  cancels the nightly run. Before this, four merges in three minutes cancelled each other and left `main` with no
  complete run. On a branch or a pull request, a newer push cancels the older run. A pull request's group is keyed
  on its number, so closing or merging it cancels its runs too.
- The nightly run tests everything on all three systems, with one runner per system for the screen tests.
- After a batch of merges, `just ci-main` starts the same complete run straight away (`gh workflow run` on
  `main` for both workflows).
- A failed push or nightly run on `main` opens or updates the "CI failed on main" issue.

Only `main`'s runs save caches that other branches can read. A cache saved on any other ref can be read by that
ref alone. Branches and pull requests therefore only restore caches, and every save step checks for
`refs/heads/main`. On `main`, only complete runs save: the workspace Rust cache is written by the test lane's
`check`, and the NSH dependency builds by the `nsh.yml` jobs. The first save of a key defines that key for good, so
a partial save would leave the entry incomplete for every later run. That is also why a manual run on `main` is
always complete, and why the NSH type check on a merge runs without the dependency cache instead of saving one.

Pushes to `main` no longer build, so caches are refreshed only by the nightly run and by `just ci-main`. After a
merge that changes `Cargo.lock`, `nsh/Cargo.lock`, the toolchain or the NSH pin, branches restore the older cache
(a partial hit) until the next complete run of `main`. Their runs are slower but still correct. Run `just ci-main`
after merging such a change.

Since `release-macos.yml` and `release-linux.yml` lost their pull-request trigger, they no longer save a cache per
pull request (about 1.7 GB each for macOS). Their tag and manual runs cache as before. `release-linux.yml` saves its
dependency builds (`buwiz-nsh-release-linux`, x64 and arm64) whenever it runs on `main`, so the nightly call adds
those entries to `main`'s caches as well.
