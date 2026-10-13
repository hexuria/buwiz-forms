# Contributing

## Windows packaging checks

Run the script suite and packaging recipe before changing installer behavior.
`just exe` stages the payload and runs strict source validation before ISCC:

```powershell
python -m unittest discover -s scripts/tests -v
just exe
```

To repeat the source gate after staging:

```powershell
python scripts/audit_installer_sources.py --installer installer.iss --stage-dir target/installer-stage
```

After `just exe`, the final Setup audit can be run separately on a machine
without an existing eBIRForms installation:

```powershell
pwsh -NoProfile -File scripts/audit_final_setup.ps1 `
  -Installer target/release-artifacts/eBIRForms-Windows-x64-<version>-Setup.exe `
  -Python python
```

The audit refuses to run when an eBIRForms uninstall registration already
exists. Never remove a real installation to make the audit pass. CI runs the
same installer audit on a disposable Windows runner.

## Anti-drift

> Any change to observable semantics names the verification boundary it affects.
>
> - Concurrency, interleaving, scheduling, retry, cancellation, recovery, ownership handoff, or liveness updates the system model, or the change states why that model is unaffected.
> - Executable Rust behavior updates the Rust verification layer. A theorem-owned kernel updates its proof. Workflow or DSL semantics update conformance or differential tests.
> - Do not clone one state machine across Rust, TLA+, Lean, and a DSL for symmetry. Passing independent suites does not establish equivalence.
