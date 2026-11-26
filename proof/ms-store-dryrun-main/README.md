# MS Store dry-run proof — main 8f922d16

A dry run of the Microsoft Store package built from hexuria/buwiz-forms **main at
8f922d163474941a09400c8d1495a50ae44d4297** (nsh/nsh.json version 0.2.1 → package version
0.2.1.0), on the Windows Server 2022 build VM, 2026-11-26/27. No code changed; nothing
uploaded anywhere.

## Build line

- app: `8f922d163474941a09400c8d1495a50ae44d4297` (main)
- ../nsh: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (= main's NSH_PINNED_REF)
- nsh CLI: `nsh 0.6.1 (8c78cfd)`
- cargo: 1.99.0 MSVC, stock rustup (`C:\Users\Administrator\.cargo\bin\cargo`)
- build folder: `C:\ci\msstore\buwiz-forms\target-msstore` (CARGO_TARGET_DIR set
  explicitly, created empty; deleted after the job per the disk rule)
- machine: Microsoft Windows Server 2022 Standard, build 20348, EnableLUA=0,
  1600×1200 @ 100% scale (96 DPI)
- build exactly as `.github/workflows/release-windows.yml` does for a non-release run:
  `nsh package --target windows --out C:\ci\msstore\nsh-windows-pkg` with
  `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS="-C target-feature=+crt-static"`, then
  `./scripts/package_nsh_windows.ps1 -PackageRoot … -OutDir release-artifacts -SmokeMsix`.
  GITHUB_REF_NAME was not set. Safety env (BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1
  BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1) on every build and run.

## What each file shows

- `step1-nsh-package.log` — stdout of `nsh package` (summary form). Shows both compile
  steps, the tree-shaking keep/leave-out lines, the update archive and the installer it
  made. It does NOT show the cargo warnings inline (they are summarized as counts).
- `step1-nsh-package-verbose-warnings.log` — the same build re-invoked with `-v` after the
  fact (fully cached, seconds) purely to surface the warnings the summary counted: the
  `nsh-bundler` patch warning, dead-code warnings (`page_gate_refusal`, `MIN_WIDTH`,
  `MIN_HEIGHT`), and the imap-proto future-incompatibility notice. It is NOT the original
  build's output and does not prove the original compile — the original log does.
- `step1-package_nsh_windows.log` — stdout of the packaging script: the unsigned-artifacts
  ::warning (expected on a non-release run with no PFX), the no-legacy audit, the WiX/Inno
  tool output and the msix smoke-test lines.
- `SHA256SUMS-windows-x64.txt` — the shipped sums file, copied verbatim. All four hashes
  were verified against `Get-FileHash` of the artifacts: all MATCH.
- `AppxManifest.xml` — the manifest unpacked from the built msix, verbatim.
- `step2-msix-listing.txt` — full recursive file listing of the unpacked msix with sizes,
  plus the sensitive-name scan (no .pdb/.env/key/cert anywhere) and the
  ebirforms-no-self-update / resources.pri presence checks.
- `wack-report.xml` — the Windows App Certification Kit report, verbatim, from
  `appcert.exe test` on a self-signed COPY of the msix (test cert subject = manifest
  Publisher; the built artifact itself was never signed or modified). OVERALL PASS,
  one FAILed test — see below.
- `01-start-all-apps.png` — Start menu All-apps list with eBIRForms under "Recently added"
  and under E with the New badge (this proves the package installed into the shell).
- `02-start-search-ebirforms.png` — Start search finding eBIRForms as best match. Note:
  activation from Start (Enter AND direct mouse click on the result) did NOTHING on this
  machine — no process, no error. This is the known EnableLUA=0 silent-activation failure
  on Windows Server.
- `03-sign-in-screen.png` — the installed app at its sign-in screen. Because Start
  activation was dead, the exe was started directly from its install folder
  (`C:\Program Files\WindowsApps\GOLDCODERSCORP.ebirforms_0.2.1.0_x64__nzqkzq9jx6pr0\eBIRForms.exe`)
  with a scratch LOCALAPPDATA, after granting Administrators RX on that one package folder
  (WindowsApps ACLs otherwise block even administrators). Nothing was typed; the run never
  went past this screen.
- `04-settings-apps-installed.png` — Settings > Apps & features showing the eBIRForms row
  (publisher GoldCoders Corp, 163 MB, install date).

## What this proof does NOT show

- The msix/msi/Setup.exe themselves are not committed (by rule); only their hashes, sizes
  and the listing.
- WACK was run on a self-signed COPY, not on a real Microsoft-signed package; some test
  outcomes can differ with a real Store signature.
- The sign-in screen was reached by launching the exe directly, not through packaged-app
  activation — activation does not work at all on this Server 2022 / EnableLUA=0 machine,
  so nothing is proven about activation on a normal Windows 11 machine.
- No interaction past the sign-in screen (by rule).

## WACK summary

OVERALL: **PASS**. 24 tests ran; 23 PASS, 1 FAIL — "Blocked executables", all messages
inside eBIRForms.exe (plus bundle.nshc / a font) about references to
CreateProcessW / ShellExecuteW and strings resembling executable names ("cmd.exe",
"powershell.exe", "cmd", "REG", "CSi", "cmD", "dnx", "cdB"). Detail in wack-report.xml.
