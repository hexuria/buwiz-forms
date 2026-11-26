# Testing-agent notes — interrupted validation run

Interrupted at 2026-11-26T02:37:54Z by lead instruction: stop validation, start no further builds/cases, preserve existing evidence, and finish immediately. No recording was active.

## What was actually done

1. Read `C:\otp\VALIDATION_BRIEF.md` and confirmed the required heads, safety constraints, launch pattern, evidence layout, and test matrix.
2. Inspected the existing workspaces:
   - `C:\otp\base\buwiz-forms` / `C:\otp\base\nsh`
   - `C:\otp\pr43\buwiz-forms` / `C:\otp\pr43\nsh`
   - `C:\otp\pr44\buwiz-forms` / `C:\otp\pr44\nsh`
3. Confirmed all three pre-existing `build-tool.exit` files contained `101`; their logs showed the original failure mode: build scripts could not run `rustc --version` because the Rust toolchain bin directory was not on `PATH`.
4. Found that `C:\otp\build-tools.sh` on disk still did **not** prepend the toolchain bin directory, despite the handoff saying the current script did. I did not edit that script.
5. Started one manual base-head tool build only, using the direct toolchain cargo and environment:
   - working directory: `C:\otp\base\nsh`
   - cargo: `C:\Users\Administrator\.rustup\toolchains\1.99.0-x86_64-pc-windows-msvc\bin\cargo.exe`
   - `PATH` prefixed with `C:\Users\Administrator\.rustup\toolchains\1.99.0-x86_64-pc-windows-msvc\bin`
   - `RUSTUP_TOOLCHAIN=1.99.0`, `RUSTC_WRAPPER` empty, `LIBCLANG_PATH=C:\Program Files\LLVM\bin`, `RUST_MIN_STACK=268435456`, `CARGO_INCREMENTAL=0`
   - command shape: `cargo install --locked --path crates/nsh-cli --root C:\otp\base\tools --force`
   - output log: `C:\otp\base\build-tool.log`
6. That base tool build was still compiling when the interruption arrived. It had reached compilation of `nsh-bundler` and `nsh-testing`; it had not linked or installed `nsh.exe`. I terminated the shell/process after the interruption. `C:\otp\base\tools\bin\nsh.exe`, `C:\otp\pr43\tools\bin\nsh.exe`, and `C:\otp\pr44\tools\bin\nsh.exe` did not exist at stopping time.
7. Started compiling the mock account server from the main checkout, without altering source:
   - working directory: `C:\Users\Administrator\repos\buwiz-forms`
   - command shape: `cargo run --locked -p bir-core --features mock-account --bin mock_account -- --auto-approve`
   - output log: `C:\otp\mock-account.log`
   - the same direct toolchain/`PATH` safety environment was used.
8. The mock-account build was still compiling when interrupted and was terminated. It never reached the runtime line `[mock-account] listening on ...`; nothing was listening on localhost port 3008 when checked.
9. Read relevant app/NSH source only (no edits): first-run setup flow, `NewTaxpayer.tsx`, `DigitCells` geometry, and `crates/nsh-base/src/otp_input.rs` at the three heads. This established that PR43 contains the desktop click/caret implementation, while PR44 appears focused on long-press behavior and does not carry the PR43 desktop implementation.
10. Created only the evidence directories `C:\otp\evidence\base`, `C:\otp\evidence\pr43`, and `C:\otp\evidence\pr44`, plus this notes file.

## What was not done

- No application window was launched.
- No `nsh run --target windows` command was executed.
- No mock account server successfully started.
- No scratch database was created or used.
- No first-run UI flow was reached.
- No digits were typed.
- No mouse click, double-click, drag, keyboard, paste, cut, Backspace, or selection case was executed.
- No clipboard read was attempted.
- No screenshot or video was captured.
- No report table with observed application behavior could be produced.
- None of the required validation cases were completed.
- Nothing was pushed.

## Launch/read methods status

The exact required launch/read methods were reviewed but never executed. Planned launch method was per head from `C:\otp\<head>\buwiz-forms\nsh`: prepend `C:\otp\<head>\tools\bin` to `PATH`; set `BUWIZ_NO_KEYCHAIN=1`, `EBIR_TEST_ENV=1`, `BIR_AUTH_BYPASS=1`, `BIR_SFTP_DRY_RUN=1`; leave `BIR_SFTP_LIVE` unset; set `RUSTUP_TOOLCHAIN=1.99.0`, `LIBCLANG_PATH=C:\Program Files\LLVM\bin`, `RUST_MIN_STACK=268435456`, empty `RUSTC_WRAPPER`; use a unique `BIR_DATABASE_PATH` under `C:\otp\evidence\<head>`; then run `nsh run --target windows` after the mock account server was listening on localhost:3008. Planned trusted read method was to focus the TIN cells, press Ctrl+A then Ctrl+C in the real window, and inspect the Windows clipboard with PowerShell `Get-Clipboard`, with an independent corroborating method only if clipboard access failed.

## Files/artifacts present at stop time

- `C:\otp\evidence\testing-agent-notes.md` — this file.
- `C:\otp\base\build-tool.log` — partial/manual base nsh-cli build log.
- `C:\otp\mock-account.log` — partial mock-account compile log.
- Empty evidence directories: `C:\otp\evidence\base`, `C:\otp\evidence\pr43`, `C:\otp\evidence\pr44`.

No behavioral evidence artifact exists yet.
