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
same installer audit on a disposable Windows runner (a `test/...` push that
includes Windows, a manual run, or the nightly run).

## CI

Pull requests run only the quick checks, on Linux. The one required status is `ci-ok`. The test suites run when
you push the commit to a `test/...` branch (`just ci-test [linux|windows|macos|all]`) and every night on `main`. A
pull request is ready when `ci-ok` is green and the test suites passed at that exact commit, either from a
`test/...` push or from a Devin machine. [docs/CI.md](docs/CI.md) covers what runs when, how to ask for tests,
which paths lead to which suites, and the caches.

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
