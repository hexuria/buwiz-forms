# nsh-release: ship eBIRForms from the NSH app, then delete bir-shell

Owner's picks, 2026-11-22. The picks are in `spec/picks.txt`. They are final: do not re-ask.

## Picks

| # | Question | Pick |
|---|---|---|
| 1 | Who has the old app | Only the owner and testers. Nobody to carry over. |
| 2 | Identity | The NSH app replaces the old one: id `dev.goldcoders.bir`, name eBIRForms. |
| 3 | Old saved returns | Start empty. No migration code. The old encrypted file is never touched. |
| 4 | Downloads | Mac `.dmg`; Windows `Setup.exe`, `.msi`, `.msix`; Linux `.deb`, `.rpm`, `.tar.gz`, Arch package, install script. |
| 4a | Windows install | For the signed-in user, no admin prompt. |
| 5 | Updates | The app updates itself with NSH's updater. |
| 5a | Release host | hexuria/buwiz-forms. |
| 6 | `bir` command | Keep it, in a tiny new crate, reading the new app's data. |
| 7 | Mac App Store / TestFlight | In this round. Deleting bir-shell waits for it. |
| 8 | Old screens in `app/` | Delete them. Move the form files to their own top-level folder. |
| 9 | `rox-mini-app-round2` | Finished. Close the branch, keep nothing. |
| 9a | Its two speed fixes | Check the NSH app for the same lag; port only what applies. |
| 10 | When to delete | As soon as CI builds every download from the NSH app. Two PRs. |
| 11 | Allowed unattended | Push branches and open PRs; build packages on this Mac; create the update signing key; sign with Developer ID here; notarize with Apple; run a test release on GitHub. |

Already settled before the picks: the NSH app keeps a plain SQLite database; Windows and Linux are tried on real machines from the two handover documents; never merge, never tag a real release, never send to BIR.

Added by the owner on 2026-11-22: the NSH app writes no log file, so Settings > Jobs > Logs is empty. Must be fixed in PR 1 (see A3).

## Facts the plan rests on

Read from the code at main f30e6767 and NSH 95b5600c6. Main has since moved (bb23a15d on 2026-11-23, with #260, #263 and #265 merged) and the pin is now 70d8ff26b (NSH 0.6.1); the NSH packaging and updater code did not change between the two pins. Line numbers can be stale: the code wins.

- `nsh package` (help text, `../nsh/crates/nsh-cli/src/package.rs`, `../nsh/website/docs/packaging.md`):
  - macOS: `.nsh/out/<Name>.app`, ad hoc signed only (`codesign --sign -`), `--arch arm64|x86_64|universal`. No Developer ID, no hardened runtime, no notarization, no `.dmg`, no store package. `--sign` reaches iOS only.
  - Windows: a folder with `<Name>.exe`, the same as `.zip`, and with Inno Setup 6 installed a per-user `<Name>-setup.exe` (`windows.rs:247-255`, `AppId=<id>`, `%LOCALAPPDATA%\Programs\<Name>`). Unsigned. Must be packaged on Windows.
  - Linux: a folder with the runner, bundle and `.desktop`, and a `.tar.gz`. Must be packaged on Linux, per architecture.
  - The app crate is linked with no features. The CLI runs `$CARGO`; `nsh/agent/cargo-with-agent.sh` uses that to add `--features app_nsh_app_ebirforms/agent`. Untested with `nsh package`.
- NSH updater (`../nsh/crates/nsh-updater/src/lib.rs`, `../nsh/website/docs/app-updates.md`): `nsh.json` `"updater": {"github": "owner/repo"}` reads `releases/latest/download/latest.json` plus `latest.json.sig`; Ed25519 key from `nsh keygen`; `nsh update-manifest` writes both; archives `<name>-<version>-<os>-<arch>.tar.gz|zip` are written by `nsh package` only when `"updater"` is set. On macOS it checks the new app with `codesign --verify --deep --strict`.
- The NSH app today: `nsh/nsh.json` has id `dev.goldcoders.bir.nsh`, no icon, no `macos` block, no `updater`. `nsh/src/desktop.rs:9-10,52-53,171-181` answers updates as NOT_CONFIGURED. The banner (`nsh/shell/UpdateSlot.tsx`) and poll are wired.
- Data folders (`crates/bir-core/src/platform/*.rs`): plain build uses `dev.goldcoders.bir.nsh` under Application Support, `$XDG_DATA_HOME`, `%LOCALAPPDATA%`. The old encrypted file lives elsewhere (Group Container, `~/.taxman-ebir`, `%LOCALAPPDATA%\Taxman\eBIRForms`).
- Logging: `bir_core::logging::install` (`crates/bir-core/src/logging.rs:92`) is called only from `crates/bir-shell/src/main.rs:13`. `job_log_tail` (`crates/bir-app-api/src/bir_module.rs:1917`) reads `ebirforms.<date>.log` from `log_files::log_dir()`.
- `bir` command: `bir.yml` builds `--bin bir-headless` from bir-shell. The bin is `bir_shell::run_headless()`, which installs hooks and calls `bir_app_api::agent::headless::run()`. The only hooks it needs forward to `bir_app_api::agent::{send_draft, pop_command}` (`crates/bir-shell/src/bir_host.rs:160-173`). Test `headless_detach` lives in bir-shell.
- Release pipeline today: `release.yml`, `release-macos.yml` (`scripts/package_macos_release.sh`: lipo, Developer ID, hardened runtime, `assets/macos/entitlements.mac.plist`, notarytool, hdiutil DMG), `release-windows.yml` (Inno Setup `installer.iss`, WiX 3.11.2 `installer.wxs`, signtool, MSIX job as CI artifact only), `release-linux.yml` (`packaging/linux/build-packages.sh`: cargo-deb, cargo-generate-rpm, tar, makepkg in Docker, `install-linux.sh`), `scripts/publish_update_manifest.py`. Secrets read: `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `APPLE_CERTIFICATE_P12`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_API_KEY_P8`, `APPLE_API_KEY_ID`, `APPLE_API_ISSUER_ID`, `WINDOWS_CERTIFICATE_PFX`, `WINDOWS_CERTIFICATE_PASSWORD`.
- Mac App Store today: `just app`, `_package-mac-appstore` (`justfile:783-870`), `testflight`, `testflight-agent`, `testflight-upload`; feature `mas_build` (removes the shell cron runner and the global hotkey); root `entitlements.plist` (sandbox); `assets/macos/embedded.provisionprofile`; `productbuild`.
- Still read from `app/`: only `app/forms/*.json` (7 files, `include_str!` in `crates/bir-app-api/src/screen_form_module.rs:450-460,5331`, `combs.rs:52`, `conditions.rs:31,41`) and one test that reads `app/shell/form_page.js` (`crates/bir-app-api/src/screen_form_module/tests/as_needed.rs:2208-2216`).
- `crates/bir-rules-codegen/src/status.rs:70` lists `crates/bir-shell/src` in `SHELL_SOURCE_ROOTS` and fails on a missing path.
- Do not rename `agent::APP = "bir-shell"` (`crates/bir-app-api/src/agent/mod.rs:42`): bir-mcp and the NSH agent tests discover the app under that name.

## Decisions made to fill gaps (final unless the owner says otherwise)

1. **Data folder follows the id.** The plain build's folder becomes `dev.goldcoders.bir` on all three systems. Nothing is moved from `dev.goldcoders.bir.nsh` (start empty).
2. **Version.** `nsh/nsh.json` `version` becomes the release version the workflows check a tag against. It stays 0.2.0; the owner tags.
3. **Names users see stay.** Mac app `eBIRForms.app`; Linux package `ebirforms`, `/usr/bin/ebirforms`, desktop file `dev.goldcoders.bir.desktop`; MSIX identity `GOLDCODERSCORP.ebirforms`. Download file names keep today's pattern (`eBIRForms-macOS-universal-<v>.dmg`, `eBIRForms-Windows-x64-<v>-Setup.exe`, `…-<v>.msi`, `eBIRForms-Linux-<arch>-<v>.deb|.rpm|.tar.gz`).
4. **Windows per user.** Setup.exe is NSH's per-user installer (or `installer.iss` rewritten to the same scope). The `.msi` becomes per-user with a new UpgradeCode. An old per-machine install is not removed automatically; the Windows handover says to uninstall it first.
5. **Which installs self-update.** The Mac app from the `.dmg`, the per-user Setup.exe install and the Linux tarball update themselves. `.deb`, `.rpm`, Arch, `.msi`, `.msix` and the Mac App Store build do not swap their own files: they show the banner with a link to the release page (store builds: nothing).
6. **Update key.** Created on the owner's Mac on 2026-11-23 with `nsh keygen` (key id `53568ad5c3ac7063`). The secret half stays on that Mac, never committed, never printed. Public half, for `nsh/nsh.json`: `azPTfgyyZg7ggQ6w52Rx/Ma9BLfMVjnTvqOflxfMhsc=`. GitHub secret name `NSH_UPDATE_KEY` (the content of the `.secret` file); the owner adds it. Without the secret the release publishes downloads and skips `latest.json` with a warning.
7. **The old v0.1.0 app's update check will fail to read the new `latest.json`.** Accepted: pick 1.
8. **TestFlight upload stays the owner's.** The store package is built and checked here; `testflight-upload` is not run.
9. **Deleting `rox-mini-app-round2`** is the last step, after the speed check, and is confirmed with the owner once (it cannot be undone).
10. **Form files move to `forms/`** at the repository root, with `git mv`.

## PR 1: every download from the NSH app (`feat/nsh-release` → main)

bir-shell and `app/` stay untouched in this PR. Tracks A to G; A is first because the others read its names.

### A. The app itself
- A1 `nsh/nsh.json`: id `dev.goldcoders.bir`, icon, `macos` block (entitlements, Info.plist keys the old `assets/macos/Info.plist` carries that still apply), `updater` (`github: hexuria/buwiz-forms`, public key), single instance.
- A2 Data folder rename in `crates/bir-core/src/platform/*.rs` and every test, guard and doc that names `dev.goldcoders.bir.nsh` (`nsh/src/guards.rs`, `nsh/screens/settings/computer.test.tsx:33`, handovers).
- A3 Logging: the NSH host installs `bir_core::logging::install` at start with a filter that includes `bir_app_api` and the NSH crate. Test: after start, today's log file exists and the Logs tab lists its lines (headless test plus one real-window check).
- A4 Updates on: `nsh/src/desktop.rs` stops answering NOT_CONFIGURED; status, download and install go through `nsh/updater`; decision 5 decides self-update or link per install kind. The banner mounts where `nsh/shell/desktop/README.md` says it is missing. Tests for each status.
- A5 A cargo feature `mas_build` on the NSH crate passing through to bir-app-api and bir-core; with it the hotkey setting and shell cron runner are off and updates are off.

### B. The `bir` command
- New `crates/bir-cli` (root workspace member), bin `bir`, bir-app-api features `agent` + `headless`, plain SQLite. Hooks forward `agent_send_draft` and `agent_pop_command`. `headless_detach` test moves here. Log text `app=bir-shell` in `headless.rs:457` changes to `app=bir`.
- `bir.yml` builds it (no rename step), checks the tag against the workspace version.
- `crates/bir-shell/docs/AGENT.md` content that still applies moves to `docs/agent.md`; links in `nsh/AGENTS.md`, README, justfile follow.

### C. macOS
- `scripts/package_macos_release.sh` (or a successor): `nsh package --target macos --arch universal`, then Developer ID signing with hardened runtime and entitlements, notarize, staple, `.dmg` with hdiutil, checksum, and the updater archive from the signed app.
- `release-macos.yml` and the `just` recipe use it. Prove signing and notarization end to end: on the owner's Mac when the run happens there, otherwise through the release workflow, whose secrets hold the certificate. Open the notarized app on a Mac.
- Check that the updater's swap accepts the Developer ID signed app (`codesign --verify --deep --strict`).

### D. Mac App Store
- `just app` and `testflight*` recipes build the NSH app with `mas_build` through the `$CARGO` wrapper, sandbox entitlements, the provisioning profile, `productbuild`. Prove the wrapper works with `nsh package`; if it does not, record why and use the closest route that still builds the feature in.
- The sandboxed app starts, reaches its data folder in the container, opens a form and prints. Report what the sandbox refuses.

### E. Windows
- `release-windows.yml`: package on the Windows runner; Setup.exe (per user), `.msi` (per user, WiX), `.msix` (now attached to the release), signtool on the exe and each installer when the certificate secrets exist, checksums, the updater `.zip`.
- `ci.yml` "Build and audit unsigned Windows Setup" builds the NSH Setup instead. Keep or drop the PE stack-reserve check by whether the NSH exe needs it; say which and why.
- Cannot be built on this Mac: prove it with workflow runs on a test tag or `workflow_dispatch`, then update `docs/platforms/nsh-windows-handover.md` with the install, update and uninstall steps for a real Windows machine.

### F. Linux
- `packaging/linux/build-packages.sh` and `release-linux.yml`: package on x64 and arm64 runners; `.tar.gz` from NSH; `.deb`, `.rpm` built around the NSH folder (metadata moves out of `crates/bir-shell/Cargo.toml`); Arch package and PKGBUILD (x64); `install-linux.sh`; checksums.
- Prove with workflow runs; update `docs/platforms/nsh-linux-handover.md` for a real Linux machine.

### G. Release glue and the speed check
- `release.yml`: version check against `nsh/nsh.json`, `nsh update-manifest` with `NSH_UPDATE_KEY`, upload `latest.json` and `latest.json.sig`; `scripts/publish_update_manifest.py` goes or is rewritten. `nsh.yml` triggers gain `app/forms/**`.
- Speed check (pick 9a): from PRs #198 and #203 on `rox-mini-app-round2`, test the NSH app for (1) the device-filing poll blocking the window, (2) idle polls re-rendering with nothing changed, (3) slow scrolling and typing on the long annual forms. Measure first; port only what measures as a problem; write the numbers in the PR.

### PR 1 is done when
- A pre-release from a test tag on hexuria/buwiz-forms carries every file in pick 4 built from the NSH app, plus `bir-*` archives from `crates/bir-cli` on a `bir-v*` test tag.
- The notarized Mac `.dmg` installs and opens on a Mac; an update from one test version to the next installs itself.
- The Mac App Store package builds and its app runs sandboxed.
- `just nsh-check -v && just nsh-build && just nsh-test`, `just core-plain-test`, `cargo test -p bir-cli`, the Python script tests and CI are green.
- No test run ever becomes the repository's latest release: the old v0.1.0 app and the new updater both read `releases/latest`. Prefer `workflow_dispatch` runs that keep their files as workflow artifacts; a draft pre-release only where the publish step itself is under test. Test tags and pre-releases made along the way are deleted at the end, and the report lists them.

## PR 2: delete bir-shell (`feat/nsh-release-delete`, stacked on PR 1)

Open it against PR 1's branch; when PR 1 merges, retarget to main and rebase. Never let it merge into a branch that was already merged: that is how #234 and #236 were lost. Change list:

- Delete `crates/bir-shell/`, `installer.iss`/`installer.wxs` if PR 1 replaced them, `scripts/alias_app_bin.py`, `scripts/check_pe_stack_reserve.py` and its test if dropped in E, `scripts/dev_bundle_macos.sh`, `scripts/preview-form.sh`, `scripts/check_file_tax_goal.sh`, the bir-shell parts of `scripts/file_tax_smoke.py` (repoint to `bir`), `daemon.entitlements.plist` and `assets/macos/entitlements.mas.plist` if still unreferenced.
- `git mv app/forms forms`; update the `include_str!` paths, `nsh.yml` and `ci.yml` path filters. Delete the rest of `app/` and `tools/shell_mount.mjs`, `tools/kit_surface.mjs`, the 30 `tools/test_*.mjs`. Rewrite the `as_needed.rs` test so it no longer reads `app/shell/form_page.js`.
- `ci.yml`: remove `BUILD_BIR_SHELL_SETUP`, the `shell-check` job, `shell`/`shell_matrix`/`shell_re`, and shell-check from `ci-ok`; fix `scripts/tests/test_ci_change_detection.py` and `scripts/tests/test_release_workflow_policy.py`. Keep `legacy-guard`.
- `crates/bir-rules-codegen/src/status.rs:70`: drop `crates/bir-shell/src`; inventories must not change.
- bir-app-api: remove `packaged::application_present` and its test; keep `packaged::dir`. Remove the `sqlcipher` default if nothing else turns it on; say what was decided. Fix the stale "run `cargo test -p bir-shell`" notes.
- `justfile`: `run` becomes `nsh-run`; remove `shell-run`, `run-upstream`, `shell-ui-test`, `shell-check` (keep its bir-app-api headless test line in another recipe), old `exe`/`msix`/`_package-*` bodies replaced in PR 1.
- Docs: README, CONTRIBUTING, `docs/DEVELOPMENT.md`, `docs/platforms/{macos,windows,linux,nsh}.md`, `docs/mock-bir.md`, `nsh/AGENTS.md`, handovers ("never edit app/ or crates/bir-shell" goes), `.gitignore`, `integrations/opengrok/bir/README.md`, `scripts/setup-*.sh`.
- Add a guard like `legacy-guard` so `bir-shell` cannot come back by name (except `agent::APP`).

Done when: no tracked file names `crates/bir-shell` or `app/shell`; CI is green on all three systems; the PR 1 release workflows still pass on a test tag from this branch.

## Only the owner can do

- Merge PRs. Tag the real release.
- Add `NSH_UPDATE_KEY` to GitHub secrets (I give the path of the secret file, never its content).
- Run the Windows and Linux handovers on their own machines.
- Upload to TestFlight.
- Say yes to deleting `rox-mini-app-round2`.

## Open risks

- The `$CARGO` feature wrapper has only been used with `nsh run`.
- Sandboxing the NSH app for the Mac App Store has never been tried; pick 7 makes PR 2 wait for it.
- Windows and Linux packaging can only be proven by CI from here.
- NSH desktop TLS trusts webpki roots only (nsh#914); GitHub release downloads should be fine, a custom host would not be.
- `nsh/AGENTS.md` and `docs/platforms/nsh.md` are stale in places (they still call the app menu, OS sign-in and Linux print missing). Trust the code.
- #260, #263 and #265 merged on 2026-11-22/23 and changed `app/forms/*.json`, `ci.yml`, `justfile`, `scripts/setup-*.sh`, the agent code and the NSH pin. Start from current `main`.
