# Independent Windows check of PR #385

Checked `hexuria/buwiz-forms` pull request #385 at `2185cb00e02611b8326be4e00d4431b9fb67779b` on Windows Server 2022. The comparison checkout was the then-current `origin/main`, `bcbaf324187137010b19814d6ef6e59db4d1f5d9`. Both checkouts used a clean sibling `hexuria/nsh` checkout at `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (`nsh 0.6.1 (8c78cfd)`).

Build/test line: repo toolchain `1.99.0-x86_64-pc-windows-msvc`, cargo `C:\Users\Administrator\.rustup\toolchains\1.99.0-x86_64-pc-windows-msvc\bin\cargo.exe`, `RUSTC_WRAPPER` empty, PR target `C:\Users\Administrator\builds\check-385\pr\buwiz-forms\target-check-385`, main target `C:\Users\Administrator\builds\check-385\main\buwiz-forms\target-check-385`. Safety environment for every run: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, `CARGO_BUILD_JOBS=4`.

## Files

- `issue349.txt` — issue text fetched from GitHub.
- `pr385.txt` — pull-request text fetched from GitHub.
- `pr-diff.txt` — `git diff origin/main...2185cb00e02611b8326be4e00d4431b9fb67779b` for the three touched files.
- `bir-app-api-agent-test.txt` — full output of `cargo test -p bir-app-api --features agent` at the PR head. Result: 844 passed, 0 failed, 7 ignored in the lib suite; doc-tests 0.
- `nsh-agent-test-first.txt` — full output of the first CI-shaped NSH command, run from `nsh/`: `cargo test --manifest-path Cargo.toml --features agent --test agent --test agent_send`. Result: `agent` 7 passed / 0 failed; `agent_send` 1 passed / 0 failed.
- `nsh-agent-repeat-summary.txt` and `repeats/nsh-agent-repeat-01.txt` through `repeats/nsh-agent-repeat-20.txt` — twenty consecutive repetitions of the same command. All 20 passed.
- `observation-pr.txt` / `.json` — real hosts-folder observation at the PR head while running only `an_external_agent_creates_the_demo_taxpayer_opens_a_2551q_fills_a_box_and_reads_it_back` the way the agent test does. Polling observed two distinct record contents: first executable `agent-e7cc3c45fc04aa8b.exe`, then executable `bir`. The wrong-executable content was observed for about 6.55 ms. Watchdog counted two rename-into-place events for the record.
- `observation-main.txt` / `.json` — the same observation at `origin/main` `bcbaf324187137010b19814d6ef6e59db4d1f5d9`. Polling observed two distinct record contents: first executable `agent-adb111aefb026f75.exe`, then executable `bir`. Watchdog counted three rename-into-place events for the record.

## Observation method and limits

The observer was a separate Python process. It detected the test's temporary runtime directory, started a watchdog recursive observer, and polled the record file with a requested 0.5 ms period for two seconds after first seeing record content. The main run completed 4542 poll iterations in about two seconds; the PR run completed 3493. Polling timestamps are sample times. Watchdog timestamps are event-delivery times, not exact write times; their ordering and rename count are the useful part.

The first and second records at main have the same bytes because both are `publish_current` records for the same process: the write count differs, but the distinct content count does not. The third write is the correct `executable: "bir"` record. At the PR the intermediate `publish_current` write is gone, so there are only the library's first write and the correct write.

The final `absent` transitions in the polling sections are test-end cleanup, not startup behavior. Startup deletion/replacement gaps smaller than the poll interval can be missed; the watchdog event sequence is included for that reason.

## Windows file-sharing note

The library writes a temporary file and renames it over the record. On Windows, replacing a file can fail if a reader keeps the destination open without delete sharing. A small control experiment here: Python's `os.replace` failed with `PermissionError [WinError 5]` while Python held the destination open, while the same test using Rust `std::fs::File::open` succeeded because Rust opens with sharing that permits the replacement. Thus ordinary Rust readers such as gpui-agent's loader are not the risky case, but non-Rust readers with stricter sharing can make a correct publish fail. The PR does not introduce that library behavior; it still depends on it for the final write.

## What this does not show

This does not run the full app UI, press Queue, contact BIR, or test a release binary. It does not prove the unmodified gpui-agent library can never write late under extreme scheduler starvation; the PR waits at most two seconds for the library's one record rather than using a hard synchronization handshake.
