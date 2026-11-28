# Windows main redness triage at e9f34205

Commit: `e9f3420563f327f8a3e994858b2fa5728d4ef6e4`; sibling kit: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`.

Safety variables used for app-starting tests: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`. No live send, Queue, or app UI run was used.

## A. TSX screen tests

The runner's default per-test limit is 30 seconds (`crates/nsh-testing/src/module.rs`). Neither target test declares a longer whole-test limit. The bridge test has two explicit 20-second waits; the session test has none.

| test | isolated runs | isolated passed | shard run | verdict |
| --- | ---: | ---: | --- | --- |
| `shell/agent/bridge.test.tsx` / “desktop: go, open, choose and review reach the frame in order” | target durations 2.1, 3.7, 2.5, 4.5, 1.9, 2.1, 2.0, 2.0, 1.9, 2.2 s; file wall times 8.1–13.7 s | 10/10 | shard 1 passed; target 2.1 s; file wall 8.1 s | passes |
| `screens/form/session.fixes.test.ts` / “[F1] dispose with an amount box still focused…” | target durations 135, 129, 136, 127, 131, 128, 134, 127, 131, 128 ms; file wall times 109.7–112.8 s because the file has 64 tests | 10/10 | shard 0 passed; target 128 ms; file wall 109.7 s | passes |

No failing assertion occurred in any isolated or shard run. The slowest target-test durations were 4.5 s and 136 ms against the 30 s default limit.

Each shard ran the workflow's sorted ordinal-index modulo-3 loop over all 221 test files, once per file. Shard 0: 74 files, 0 failures. Shard 1: 74 files, 0 failures.

## B. Python script tests

Command: `python -m unittest discover -s scripts/tests` at the requested commit.

Total: `Ran 656 tests in 528.061s` — `FAILED (failures=6, skipped=77)`; no errors.

| test | runs | passed | verdict |
| --- | ---: | ---: | --- |
| `test_crew.StateTests.test_state_push_skips_the_privacy_check_only_for_a_private_github_state_remote` | 1 | 0/1 | Windows-only test fault |
| `test_store_gmail.CheckTests.test_off_refuses_a_build_with_the_id_or_the_secret` (`1234567890` payload) | 1 | 0/1 | Windows-only test fault |
| `test_store_gmail.CheckTests.test_off_refuses_a_build_with_the_id_or_the_secret` (`GOCSPX-AbC` payload) | 1 | 0/1 | Windows-only test fault |
| `test_store_gmail.RecipeTests.test_a_failing_switch_stops_the_build` | 1 | 0/1 | Windows-only test fault |
| `test_store_gmail.SwitchTests.test_off_drops_both_variables_in_the_shell` | 1 | 0/1 | Windows-only test fault |
| `test_store_gmail.SwitchTests.test_on_keeps_both_variables` | 1 | 0/1 | Windows-only test fault |

Diagnostics: the crew test passes with an empty global git config; this machine rewrites `https://github.com/` to a local git proxy and the test does not isolate that. The Gmail shell tests use a bare Windows Python path inside bash, losing backslashes. The recipe shell-behavior failure is shell-environment sensitive on Windows; the same `set -e` shape exits nonzero under Git Bash. The two check failures are slash/backslash assertion mismatches.

## C. Rust app-crate tests

Command: `cargo test --manifest-path Cargo.toml --no-fail-fast` from `nsh/`, stock rustup cargo, fresh per-job target folder. Three runs:

| run | app-crate result line | guard test |
| ---: | --- | --- |
| 1 | `test result: ok. 170 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 51.36s` | passed |
| 2 | `test result: ok. 170 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 4.66s` | passed |
| 3 | `test result: ok. 170 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 4.63s` | passed |

No Rust test failed. `desktop::tests::the_guard_locks_the_database_the_door_opens` passed in all three runs.

## Evidence layout

- `script-tests-main.log`: complete Python output.
- `tsx-summary.txt`: parsed isolated and shard summary.
- `tsx/summary.csv`: isolated wall-clock runs.
- `tsx/shard0-summary.csv`, `tsx/shard1-summary.csv`: per-file shard results.
- `tsx/*.log`: individual TSX file logs.
- `rust/summary.csv`, `rust/run*.log`: Rust runs.
- `store-gitbash-diagnostic.log`, `state-noglobal-diagnostic.log`: classification diagnostics.
- `nsh-check-main.log`: compile/type-check evidence gathered before the test runs.
