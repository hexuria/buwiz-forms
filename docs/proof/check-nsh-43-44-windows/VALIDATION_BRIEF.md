# Independent Windows validation brief (report only; do not modify app/NSH source)

User request summary:
- Validate boxed digit input (OtpInput used by TIN/branch/sign-in/PIN) in the real Windows eBIRForms window.
- App: hexuria/buwiz-forms main 1b18d66806d01558b5355c967915232bfb9e29f2.
- NSH heads, each in its own build folder with its own nsh sibling/tool/runtime:
  - base buwiz-next 150aa172228362dbbb0bd92a6219836088e34499 at C:\otp\base
  - PR43 fix/otp-input-desktop-click 97db81be5494cfb1c63a67f3c1a6bacaf4c6b7f4 at C:\otp\pr43
  - PR44 fix/otp-input-android-longpress 125af95e242d86115c9d677acbdfb60d3bb6397c at C:\otp\pr44
- Never `nsh update`; RUSTC_WRAPPER empty; use 1.99.0 toolchain cargo.
- Safety env in every app run: BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1; never set BIR_SFTP_LIVE; never press Queue.
- Use mock account server (`cargo run --locked -p bir-core --features mock-account --bin mock_account`) and a scratch database; New taxpayer TIN boxes; invented digits only.
- Trusted field value read: focus TIN field, Ctrl+A, Ctrl+C, read clipboard (state this method). Screenshots of boxes after click cases.
- Deliver per-head tables: case, expected, seen. Push proof to hexuria/buwiz-forms branch try/check-nsh-43-44-windows; use fork only if upstream push refuses. Report tables.

Required cases:
1. At base/buwiz-next first, then at 97db81be: with digits 1,2,3 in first three TIN boxes:
   - click after digit 2, then Backspace. Expected at PR43: `13` (digit 2 removed; caret nearest digit edge). At base record today's behavior.
   - click before digit 1, then type 9. Expected at PR43: `9123`.
   - click inside a selection; double-click; drag across two digits. Record desktop behavior/screens.
2. At 125af95e keyboard matrix:
   - cut first/middle/last digit (expected remaining 23, 13, 12 respectively)
   - select all and delete
   - Backspace behavior
   - type over the middle digit
   - paste raw, dashed, spaced digit strings
   - desktop must be unchanged by Android long-press PR.

Workspace/build layout already created:
- C:\otp\base\buwiz-forms and C:\otp\base\nsh
- C:\otp\pr43\buwiz-forms and C:\otp\pr43\nsh
- C:\otp\pr44\buwiz-forms and C:\otp\pr44\nsh
- Each app worktree is detached at 1b18d66806d01558b5355c967915232bfb9e29f2.
- Each NSH worktree is detached at the required head.
- Tool build script C:\otp\build-tools.sh; logs/build-tool.log and build-tool.exit in each folder. It uses direct cargo C:\Users\Administrator\.rustup\toolchains\1.99.0-x86_64-pc-windows-msvc\bin\cargo.exe and prepends that toolchain bin to PATH.
- NSH tool root per head: C:\otp\<head>\tools\bin\nsh.exe.
- Evidence should go under C:\otp\evidence\<head>.

Launch pattern for each head (PowerShell or bash equivalent), from C:\otp\<head>\buwiz-forms\nsh:
  PATH must put C:\otp\<head>\tools\bin first.
  Set safety vars, RUSTUP_TOOLCHAIN=1.99.0, LIBCLANG_PATH=C:\Program Files\LLVM\bin, RUST_MIN_STACK=268435456, RUSTC_WRAPPER empty.
  Set BIR_DATABASE_PATH to unique scratch file such as C:\otp\evidence\<head>\scratch.db.
  Run: nsh run --target windows
  Mock account server must already be running on localhost:3008.
  Close/quit app after each head; never press Queue.

Useful facts:
- NSH CLI cold compile needs rustc on PATH. If autocfg says program not found, prepend toolchain bin.
- Bare `bash` on this Windows VM can mean WSL; use /usr/bin/bash.
- Repo push uses personal GITHUB_TOKEN secret reference, never paste/print it. Devin git function forces a bad author; for proof commits use command git plus explicit GIT_AUTHOR_NAME/EMAIL and GIT_COMMITTER_NAME/EMAIL; noreply email 158243242+devin-ai-integration[bot]@users.noreply.github.com.
- main repo checkout: C:\Users\Administrator\repos\buwiz-forms, currently main 1b18d668.
