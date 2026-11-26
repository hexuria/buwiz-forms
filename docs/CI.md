# CI

Pull requests run only quick checks, on Linux. The test suites run when someone asks for them, by pushing the
commit to a `test/...` branch, and every night on `main`. The tests themselves are not slow. The GitHub queue is,
and we were the ones filling it: about 20 jobs for each pull request, with macOS jobs waiting hours for a runner.

`scripts/ci_plan.py` decides what each run does, for both `.github/workflows/ci.yml` and
`.github/workflows/nsh.yml`. `scripts/tests/test_ci_plan.py` tests those rules. This page says the same thing in
words.

## What runs when

| Event | `ci.yml` | `nsh.yml` | Systems |
| --- | --- | --- | --- |
| Pull request (into `main`, `feat/**`, `gol/**`, `codex/**`) | quick checks | nothing | Linux |
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
- `legacy-guard`.
- The validation-rules audits on Linux, only when their paths change (see the table below).
- `changes` decides all of this. `ci-ok` sums it up.

`ci-ok` is the one required status. It fails if any job it asked for did not pass. A `test/**` push, a manual run
and the nightly run report `tests-ok` instead, so a test run on a pull request's commit never adds a second `ci-ok`
result.

No GitHub macOS job starts by itself. macOS runs only for `test/macos/**`, `test/all/**`, a manual run that names
macOS, and the nightly run. Proof for the Mac normally comes from the Devin Macs.

## Asking for tests

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
Deleting a branch does not remove its results from the commit.

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
2. The test suites passed at that exact commit, either from a `test/...` push (its runs show on the pull request)
   or from a Devin machine at the pull request's head.

Only the owner merges.

## Which paths lead to which suites

A path class only lists files that no crate, build script or test reads. Anything not listed is app code and runs
everything in its lane. For example `assets/linux/` (the desktop-entry test reads it), Markdown under `rules/` or
`scripts/`, crate `README.md` files and `evidence/` are all app code, because tests or code generation read them. So does an unknown or empty change range, except that the validation-rules audits never
start on a pull request just because the range is unknown. A `test/**` push compares against `main`, so it tests
the pull request's whole change.

| Changed files | Pull request | `test/**` push |
| --- | --- | --- |
| Docs: `docs/`, top-level `*.md`, `crates/<crate>/docs/`, `.github/*.md`, Markdown under `nsh/`, `assets/brand/`, `assets/badges/` | `legacy-guard` only | nothing |
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
  complete run. On a branch or a pull request, a newer push cancels the older run.
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
