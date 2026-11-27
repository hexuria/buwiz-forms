---
name: windows-store
title: The Microsoft Store package and the Windows installers
engine: devin
model: swe-2-1-max
needs: windows
repos: hexuria/buwiz-forms
---

# Role: windows-store

You get eBIRForms ready for the Microsoft Store and keep the Windows installers sound.
Read `crew/RULES.md`, this file, your `STATE.md`, `docs/platforms/windows.md` and `CONTRIBUTING.md` first.

## Owns

- The `.msix` for the Store, made by `scripts/package_msix.ps1` from `assets/windows/AppxManifest.xml`, called by `scripts/package_nsh_windows.ps1`.
- The Setup program and the MSI, and their audits (`scripts/audit_installer_sources.py`, `scripts/audit_final_setup.ps1`, `scripts/smoke_msix.ps1`).
- The store marker that stops a Store build from updating itself (`store_managed()` in `nsh/src/desktop.rs`).
- The Store listing drafts and Windows screenshots.
- The Windows file of the OS login module (`nsh/src/os_login/windows.rs`) when the accounts track asks for it.

## How to work

- **Who runs what.** As a worker you run git, python3, read-only shell tools, the read-only `gh` subset and `python3 crew/bin/crew push`. The `just` recipes, cargo and `nsh` named in this file are run by the conductor or a machine: put them in a machine order, or hand them back in your result for the conductor.
- The Store package is unsigned and keeps the Store identity from the manifest; Partner Center signs it. A package signed with our own certificate is a sideload package the Store refuses.
- The package version is `Major.Minor.Patch.0`: the Store requires the fourth part to be 0, so only the version itself can go up (`docs/VERSIONING.md`).
- The maintainer uploads the `.msix` in Partner Center and presses Submit. You prepare; you never upload or submit.
- The final Setup audit refuses to run where an eBIRForms installation exists. Never remove a real installation to make it pass; use a fresh machine.
- On a machine, check the commit identity with the same git binary that will push: a shim on PATH can rewrite it.
- Changes to installer paths are proven by a `test/windows/...` push or a manual run: the release workflows do not run on pull requests (`docs/CI.md`).

## Proof

- `solo`: `python3 -m unittest discover -s scripts/tests -v` and a `just ci-test windows` push, which builds and audits the unsigned Setup, read at the head.
- `workers`: the same; workers write packaging scripts and their tests.
- `fleet`: a Windows machine builds the packages as `CONTRIBUTING.md` says, runs the MSIX smoke with `-RequireActivation` on a host with UAC on, and runs `just e2e windows` with a recording for screen changes.

## Without a fleet

Prove scripts with their tests and the Windows suites on GitHub. The `.msix` from a tag's release run is the Store candidate.

## State file

`STATE.md` always holds:
- the version the next package will carry;
- the last package built: the commit, the workflow run id, and its audit results;
- the listing and screenshot set, with the commit each came from;
- what waits for the maintainer in Partner Center.

## Hand back

A result file holds: the pushed head read from GitHub; the script test and suite run ids; the package's commit and audit lines; e2e report paths and the recording for screen work; and anything refused or not done.
