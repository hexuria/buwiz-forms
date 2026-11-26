# rooms A1 owner-lock holder Windows proof

Proof of the owner-lock holder tests on Windows for `hexuria/buwiz-forms` commit `bcdb987618adf6b25af55afe4355eeaeb6c97c24` (`try/rooms-a1`).

Environment: Windows Server 2022, x86_64-pc-windows-msvc, rustc 1.99.0, cargo 1.99.0. Commands were run with `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, `CARGO_BUILD_JOBS=4`, empty `RUSTC_WRAPPER`, and an isolated `CARGO_TARGET_DIR` at `C:\Users\Administrator\builds\rooms-a1-windows-holder\target`.

## Files

- `owner-lock-run1.txt` — full output of the first `cargo test -p bir-core --lib owner_lock -- --nocapture` run, including the first compile. It passed all three owner-lock tests. The nested `running 1 test` / `owner_lock_still_refuses_another_process ... ok` output is the child process started by the cross-process test.
- `owner-lock-run2.txt` — full output of the second identical owner-lock run. It passed all three owner-lock tests.
- `owner-lock-run3.txt` — full output of the third identical owner-lock run. It passed all three owner-lock tests.
- `db-module-run1.txt` — full output of `cargo test -p bir-core --lib db:: -- --nocapture`. It passed 281 tests, failed 0, ignored 0, filtered 1475.

## What this does not show

This does not run the app, sign in, contact BIR or the production relay, or prove behavior on another Windows configuration. It is only the requested Rust unit-test evidence at the exact commit on this machine.
