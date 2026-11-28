Free disk: 47.1 GB

# Windows build-machine proof

Machine identity is intentionally omitted from this public proof artifact; it is reported in the operator session.

## Environment

- OS: Microsoft Windows Server 2022 Standard, version 10.0.20348
- buwiz-forms commit: `92b3e2013f7a60a86dda120dd93e5473cb4b0c16` (all proof binaries are built from this checkout)
- NSH pinned ref: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`
- NSH checkout: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`
- nsh version: `nsh 0.6.1 (8c78cfd)`
- NSH checkout state: clean; it was not edited
- Safety environment for proof commands: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; `BIR_SFTP_LIVE` and `BIR_DATABASE_PATH` unset
- Build folders: `$HOME/BuwizNsh/target-nsh` for the NSH proof commands; workspace `target/` for `cargo test -p bir-core`. Both belong only to this checkout/commit.

## Setup proof

- `scripts\setup-windows.ps1`: PASS; log `setup-windows.log`
- `scripts\setup-android-windows.ps1 -NoEmulator`: PASS; log `setup-android-windows.log`

## Requested proof commands

### `just nsh-check` — PASS

Exit code `0`, duration 36.2 minutes. Last lines of `nsh-check.log`:

```text
command: just nsh-check
repo commit: 92b3e2013f7a60a86dda120dd93e5473cb4b0c16
started: 2026-11-28T15:44:54.7289553+00:00
free disk GB before: 56.2
--- output ---
✓ checked nsh-app-ebirforms (13m 39s)
✓ built the Rust crate nsh-app-ebirforms (21m 9s)
--- result ---
exit code: 0
ended: 2026-11-28T16:21:06.4669411+00:00
duration minutes: 36.2
free disk GB after: 49.3
```

### `just nsh-build` — PASS

Exit code `0`, duration 0.1 minutes. Last lines of `nsh-build.log`:

```text
command: just nsh-build
repo commit: 92b3e2013f7a60a86dda120dd93e5473cb4b0c16
started: 2026-11-28T16:21:06.5303432+00:00
free disk GB before: 49.3
--- output ---
✓ built the Rust crate nsh-app-ebirforms (2.6s)
✓ built App.tsx into .nsh\out (1.1 MB js, 3.2 MB bytecode, 62 assets, 2.0s)
--- result ---
exit code: 0
ended: 2026-11-28T16:21:12.3049074+00:00
duration minutes: 0.1
free disk GB after: 49.3
```

### `cargo test --locked -p bir-core` — PASS

Exit code `0`, duration 20.72 minutes. Last lines of `cargo-test-bir-core.log`:

```text

   Doc-tests bir_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

--- result ---
exit code: 0
ended: 2026-11-28T16:41:55.5576316+00:00
duration minutes: 20.72
free disk GB after: 47.1
```

## Media

- `screenshot-proof-passed.png`: final terminal state showing every proof command passing
- `windows-setup-proof.webp`: annotated screen-recording highlights
- `windows-setup-proof-timelapse.mp4`: compressed full-session timelapse

Paths in committed logs are normalized to `$HOME`; no keys, tokens, certificates, or `.env` values are intentionally included.