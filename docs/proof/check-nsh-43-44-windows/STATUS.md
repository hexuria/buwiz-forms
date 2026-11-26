# Partial Windows validation status: NSH #43/#44

This branch preserves the state reached when the validation was urgently paused.

## How far it got

- Three independent workspace pairs were created under `C:\otp\base`, `C:\otp\pr43`, and `C:\otp\pr44`.
- In every pair, `buwiz-forms` was detached at main `1b18d66806d01558b5355c967915232bfb9e29f2`.
- The sibling NSH worktrees were detached at:
  - base/buwiz-next `150aa172228362dbbb0bd92a6219836088e34499`
  - PR #43 `97db81be5494cfb1c63a67f3c1a6bacaf4c6b7f4`
  - PR #44 `125af95e242d86115c9d677acbdfb60d3bb6397c`
- The exact validation brief is preserved in `VALIDATION_BRIEF.md`.
- Per-head CLI build attempts began in the separate folders. The first attempt failed because the toolchain's `bin` folder was not on `PATH` inside the build script (`rustc: program not found`); the script was corrected and a fresh build was started.
- The validation was paused before any per-head `nsh.exe` tool was known complete, before any app window launched, and before any required UI case, screenshot, clipboard read, table, or result was produced.

## Safety

No app run reached the window. `Queue` was never pressed. `BIR_SFTP_LIVE` was never set. No real taxpayer data or credentials were used. No NSH or app source file was modified.

## Artifact meanings

- `base-build-tool.log`: interrupted/partial build log for the base head after the PATH correction.
- `pr43-build-tool.log`, `pr44-build-tool.log`: logs from the initial failed attempts before the PATH correction; they are included only as evidence of the setup state at pause time.
