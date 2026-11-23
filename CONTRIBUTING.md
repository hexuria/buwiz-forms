# Contributing

## Windows packaging checks

Run the script suite and build the installers before changing installer behavior.
The installers are built from the NSH app, the way
`.github/workflows/release-windows.yml` builds them. The packaging script needs
PowerShell 7 (`pwsh`), Inno Setup 6, WiX 3 and the Windows SDK:

```powershell
python -m unittest discover -s scripts/tests -v
cd nsh
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = "-C target-feature=+crt-static"
nsh package --target windows --out ..\target\nsh-windows
cd ..
pwsh -NoProfile -File scripts/package_nsh_windows.ps1 -PackageRoot target/nsh-windows -OutDir target/release-artifacts
```

The script writes the Setup program, the MSI, the MSIX and the update archive. It
runs the source validation (`scripts/audit_installer_sources.py`) on the staged
payload before Inno Setup, and the no-legacy audit on every package.

The final Setup audit can be run separately on a machine without an existing
eBIRForms installation:

```powershell
pwsh -NoProfile -File scripts/audit_final_setup.ps1 `
  -Installer target/release-artifacts/eBIRForms-Windows-x64-<version>-Setup.exe `
  -Python python
```

The audit refuses to run when an eBIRForms uninstall registration already
exists. Never remove a real installation to make the audit pass. CI runs the
same installer audit on a disposable Windows runner.

## CI change selection

The `changes` job in `.github/workflows/ci.yml` sorts every changed file into a
class and runs only what those classes need. The one required status is
`ci-ok`, which passes when every job the decision asked for passed.

| Changed files | What runs |
| --- | --- |
| Docs only: `docs/`, top-level Markdown, `crates/<crate>/docs/`, `.github/*.md` | nothing heavy |
| Windows installer files: `assets/windows/`, `packaging/windows/`, `scripts/audit_final_setup.ps1`, `scripts/audit_installer_sources.py`, `scripts/package_msix.ps1`, `scripts/package_nsh_windows.ps1`, `scripts/smoke_msix.ps1` | `check` on Windows only (it builds and audits the Setup.exe) |
| macOS packaging (`assets/macos/`, `scripts/package_macos_release.sh`, `scripts/setup_macos_release_secrets.sh`, `*entitlements*.plist`), Linux packaging (`packaging/linux/`), setup scripts, release workflows, `scripts/tests/` | the Python script tests (`scripts` job) |
| Anything else, including `.github/workflows/ci.yml` | `check` on macOS, Linux and Windows, plus `windows-smoke` |

Scheduled and manual runs and unknown or empty change ranges run everything. A
path may only join a packaging class if no crate, build script or Rust test
reads it: `assets/linux/` is read by the Linux packaging's desktop-entry test, so it is
app code. Markdown under `rules/` or `scripts/`, crate `README.md` files, and
anything under `evidence/` are app code too, because they feed code generation
or policy checks.

Branches do not have to be up to date with `main` to merge. A failed run on
`main` (the push a merge makes, or the nightly run) opens or updates the "CI
failed on main" issue.

## Anti-drift

> Any change to observable semantics names the verification boundary it affects.
>
> - Concurrency, interleaving, scheduling, retry, cancellation, recovery, ownership handoff, or liveness updates the system model, or the change states why that model is unaffected.
> - Executable Rust behavior updates the Rust verification layer. A theorem-owned kernel updates its proof. Workflow or DSL semantics update conformance or differential tests.
> - Do not clone one state machine across Rust, TLA+, Lean, and a DSL for symmetry. Passing independent suites does not establish equivalence.

## UI performance

Read [docs/ui-performance-rules.md](docs/ui-performance-rules.md) before changing `nsh/`.
