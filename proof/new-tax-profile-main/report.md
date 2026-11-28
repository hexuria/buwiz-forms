# PR #492 UI evidence test report

## Scope and result

Captured local runtime UI evidence for the New tax profile entries at:

- AFTER `8f38cedacd21d05708e7c6e576421965e224517e`
- BEFORE `af4348cfbaf95ac00aeec84b274359706a5d7ba8`

Evidence index: `/Users/devin/repos/newtaxproof492/evidence/CAPTURES.tsv`.

macOS BEFORE captures were **completed** for the three gpui-visible entries: side panel, narrow picker, and command palette. iPhone BEFORE captures were also completed for the picker and Home empty state. The native macOS menu is a documented `known_gap` in both directions because gpui-agent does not expose menu rows/actions for this app.

One desktop AFTER mp4 exists and is valid MP4:

- `/Users/devin/repos/newtaxproof492/evidence/8f38ceda-macos-desktop-entry-proof.mp4`
- duration 18.266667s, size 1,499,816 bytes, ISO Media MP4

## Safety and constraints observed

Every app launch used:

```sh
BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1
unset BIR_SFTP_LIVE GOOGLE_CLIENT_ID GOOGLE_CLIENT_SECRET
CARGO_TARGET_DIR=/Users/devin/repos/newtaxproof492/target
```

`BIR_SFTP_LIVE` was never set. Only scratch databases under `/Users/devin/repos/newtaxproof492/scratch/` were used. No Queue/Send/Submit control was pressed. No real BIR, production relay, production account server, or `api.buwiz.com` was intentionally contacted. UI input was through `/Users/devin/.cargo/bin/gpui-agent` only. `simctl` was used only for simulator/container infrastructure, never input. Nothing was pushed or committed.

The generated first-run recovery key for the disposable helpdesk scratch database was not copied into evidence; the saved refs file pattern was checked and no recovery-key-shaped value remains in evidence files.

## Exact build/run commands

### AFTER macOS

Existing launcher:

```sh
/Users/devin/repos/newtaxproof492/run-after-mac.sh
```

That script runs, from `/Users/devin/repos/newtaxproof492/app`:

```sh
/Users/devin/.cargo/bin/just nsh-run-agent macos /Users/devin/repos/newtaxproof492/scratch/after-owner.db
```

Run header in `/Users/devin/repos/newtaxproof492/after-mac-run.log`:

```text
build: app 8f38ceda, nsh 8c78cfd6 beside it, tool nsh 0.6.1 (8c78cfd), cargo 1.99.0 (5f94df478 2026-08-27) (/Users/devin/.rustup/toolchains/1.99.0-aarch64-apple-darwin/bin/cargo), folder /Users/devin/repos/newtaxproof492/target
```

Session: `proofmac`, window plane `127.0.0.1:17421`.

### AFTER iPhone simulator

Launcher created for this job:

```sh
/Users/devin/repos/newtaxproof492/run-ios.sh after /Users/devin/repos/newtaxproof492/scratch/after-ios-owner.db 17431 proofios
/Users/devin/repos/newtaxproof492/run-ios.sh afterhdempty /Users/devin/repos/newtaxproof492/scratch/after-ios-helpdeskempty.db 17431 proofios
```

The launcher runs, from `/Users/devin/repos/newtaxproof492/app`:

```sh
/Users/devin/.cargo/bin/just nsh-run-agent ios <scratch-db> --device E0224A22-F874-4B8C-83FB-EB084492C493
```

with:

```sh
RUSTUP_TOOLCHAIN=1.99.0
CARGO_INCREMENTAL=0
CARGO_PROFILE_DEV_DEBUG=0
GPUI_AGENT_ADDR=127.0.0.1:17431
```

Run header in the iOS logs:

```text
build: app 8f38ceda, nsh 8c78cfd6 beside it, tool nsh 0.6.1 (8c78cfd), cargo 1.99.0 (5f94df478 2026-08-27) (/Users/devin/.rustup/toolchains/1.99.0-aarch64-apple-darwin/bin/cargo), folder /Users/devin/repos/newtaxproof492/target
nsh-run-agent: window plane on 127.0.0.1:17431, data plane on 127.0.0.1:17432
```

### BEFORE iPhone simulator

Commands:

```sh
/Users/devin/repos/newtaxproof492/run-ios.sh before /Users/devin/repos/newtaxproof492/scratch/before-owner.db 17431 proofios
/Users/devin/repos/newtaxproof492/run-ios.sh beforehdempty2 /Users/devin/repos/newtaxproof492/scratch/before-ios-helpdeskempty.db 17431 proofios
```

Run header in `/Users/devin/repos/newtaxproof492/before-ios-run.log`:

```text
build: app af4348cf, nsh 8c78cfd6 beside it, tool nsh 0.6.1 (8c78cfd), cargo 1.99.0 (5f94df478 2026-08-27) (/Users/devin/.rustup/toolchains/1.99.0-aarch64-apple-darwin/bin/cargo), folder /Users/devin/repos/newtaxproof492/target
```

### BEFORE macOS

Launcher:

```sh
/Users/devin/repos/newtaxproof492/run-mac.sh before /Users/devin/repos/newtaxproof492/scratch/before-owner.db 17421 proofmac
```

It runs:

```sh
/Users/devin/.cargo/bin/just nsh-run-agent macos /Users/devin/repos/newtaxproof492/scratch/before-owner.db
```

Run header in `/Users/devin/repos/newtaxproof492/before-mac-run.log`:

```text
build: app af4348cf, nsh 8c78cfd6 beside it, tool nsh 0.6.1 (8c78cfd), cargo 1.99.0 (5f94df478 2026-08-27) (/Users/devin/.rustup/toolchains/1.99.0-aarch64-apple-darwin/bin/cargo), folder /Users/devin/repos/newtaxproof492/target
```

The BEFORE build completed:

```text
✓ built the Rust crate nsh-app-ebirforms (21.1s, 7 warnings: -v shows them)
✓ built App.tsx (1.5 MB js, 5.6 MB bytecode, 173ms)
✓ built the development host for macOS (26.7s, 8 warnings: -v shows them)
```

After captures, the worktree was restored to `8f38cedacd21d05708e7c6e576421965e224517e`; `git status --short` was empty at report time.

## Evidence results

### PASS — AFTER macOS, owner with one tax profile

Owner profile used: TIN root `123456788`, display `123-456-788-00000`, name `E2E PROFILE PROOF`.

- `8f38ceda-macos-sidepanel-entry-visible.png` — `+ New tax profile` visible in side panel.
- `8f38ceda-macos-sidepanel-opened-new-profile.png` — clicking it opened the New tax profile form.
- `8f38ceda-macos-picker-addrow-visible.png` — `+ New tax profile` visible in narrow picker.
- `8f38ceda-macos-picker-opened-new-profile.png` — clicking it opened the New tax profile form.
- `8f38ceda-macos-palette-row-visible.png` — command palette query showed the `+ New tax profile` row.
- `8f38ceda-macos-palette-opened-new-profile.png` — clicking it opened the New tax profile form.
- `8f38ceda-macos-desktop-entry-proof.mp4` — one desktop AFTER recording.

Representative commands were of the form:

```sh
GPUI_AGENT_SESSION=proofmac /Users/devin/.cargo/bin/gpui-agent click context-pill
GPUI_AGENT_SESSION=proofmac /Users/devin/.cargo/bin/gpui-agent screenshot --inline --out <evidence.png>
GPUI_AGENT_SESSION=proofmac /Users/devin/.cargo/bin/gpui-agent viewport --width 390 --height 844
GPUI_AGENT_SESSION=proofmac /Users/devin/.cargo/bin/gpui-agent click nb-search
```

### PASS — BEFORE macOS, owner with one tax profile

- `af4348cf-macos-sidepanel-entry-absent.png` — side panel visible; no New taxpayer/New tax profile add entry. Semantic snapshot confirmed `panel-new-tp` and `panel-new-taxpayer` absent.
- `af4348cf-macos-picker-addrow-absent.png` — narrow picker shows only `E2E PROFILE PROOF`; no add row.
- `af4348cf-macos-palette-row-absent.png` — query `New tax profile` shows `No matches. Try a form code like 2550Q.`; semantic snapshot confirmed no exact row.

macOS BEFORE captures are therefore complete for every surface gpui-agent can expose.

### PASS — BEFORE iPhone, owner with one tax profile

- `af4348cf-ios-picker-addrow-absent.png` — picker shows only `E2E PROFILE PROOF`; no add row. Semantic snapshot confirmed no `New taxpayer`/`New tax profile` button.

### PASS WITH SETUP CAVEAT — BEFORE/AFTER iPhone Home empty state

BEFORE files:

- `af4348cf-ios-homeempty-add-first-taxpayer-button.png` — Home empty state shows the old label `Add your first taxpayer`.
- `af4348cf-ios-homeempty-opened-new-taxpayer.png` — clicking it opened the old `New taxpayer` page.

AFTER files:

- `8f38ceda-ios-homeempty-new-profile-button.png` — Home empty state shows the new label `New tax profile`.
- `8f38ceda-ios-homeempty-opened-new-profile.png` — clicking it opened the `New tax profile` page.

Caveat for both directions: these were captured in a helpdesk-admin/no-profiles scratch setup. A literal solo-owner/no-profiles state could not be produced through the real app UI: choosing “Just me or my business” goes directly into first-profile creation, and deleting the solo owner's only profile is not offered because `canRemove()` requires admin and non-self. Removing the profile rows while keeping `admin_tin_root` produced the lock message `No taxpayer with TIN 123-456-788`. The visual empty Home state is nevertheless proved, including the BEFORE-to-AFTER label change.

### PASS — AFTER iPhone, owner with one tax profile

- `8f38ceda-ios-picker-addrow-visible.png` — picker shows `E2E PROFILE PROOF` and `+ New tax profile`.
- `8f38ceda-ios-picker-opened-new-profile.png` — clicking the row opened the New tax profile form.

Semantic snapshot contained `palette-item-1 button 'New tax profile'`; after click it contained `new-tp`, `new-tp-name`, and `new-tp-cancel`.

### KNOWN GAP — native macOS menu, BEFORE and AFTER

macOS native menu rows are not exposed in the gpui semantic tree and no GPUI keybinding exists for them. OS input was forbidden, so the menu could not be driven directly.

Exact AFTER errors in `/Users/devin/repos/newtaxproof492/evidence/after-macos-native-menu-gpui-attempts.txt` include:

```text
error: node `run:File > New tax profile…` not found
error: node `run:Profiles > New tax profile…` not found
error: unknown binding `menu.File/New tax profile…`
error: unknown binding `menu.Profiles/New tax profile…`
```

Exact BEFORE errors in `/Users/devin/repos/newtaxproof492/evidence/before-macos-native-menu-gpui-attempts.txt` include both old and new labels:

```text
error: node `run:File > New taxpayer…` not found
error: node `run:Profiles > New taxpayer…` not found
error: unknown binding `menu.File/New taxpayer…`
error: unknown binding `menu.Profiles/New taxpayer…`
error: node `run:File > New tax profile…` not found
error: node `run:Profiles > New tax profile…` not found
error: unknown binding `menu.File/New tax profile…`
error: unknown binding `menu.Profiles/New tax profile…`
```

This is a gpui-agent capability limitation, not proof of whether the native menu item is enabled. The narrow hamburger is intentionally absent on macOS because macOS uses the native menu.

## Setup problems and exact errors

1. Initial iOS attach failed repeatedly:

```text
error: invalid automation token
```

Cause found: a stale `nsh-ios-host` process from the prior launcher was still listening on `127.0.0.1:17431`. Killing the stale host and relaunching with a fixed loopback-only job token fixed attachment. No token value is printed here or in evidence.

2. The first positional `gpui-agent keybinding` probes in BEFORE were CLI usage errors (`unexpected argument ... found`). They are not evidence; the correct `--id ... --scope focused` probes are preserved in the cleaned transcript.

3. Some gpui clicks/types initially reported `delivered_unverified` or `node ... not found` while refs were stale. Fresh refs were taken and the captures then succeeded.

## Files removed

After macOS BEFORE and AFTER captures were fully complete, I removed the job-created directory `/Users/devin/repos/newtaxproof492/target/debug` (about 15 GiB) to make room for the BEFORE iOS build, exactly under the lead's cleanup approval. No protected path was removed. `/Users/devin/repos/newtaxproof492/target` itself remains, with the iOS simulator build artifacts.

Temporary/runtime files removed only as part of scratch setup:

- stale simulator `Library/Caches/gpui-agent-token` (local automation token file; contents never read/printed/copied)
- scratch database files immediately before recreating same-job scratch databases

The lead had already removed `/Users/devin/repos/newtaxproof492/target/debug/incremental` before this continuation. Disk was about 16 GiB free immediately after my removal of `target/debug`.

## Remaining gaps

- Native macOS menu BEFORE/AFTER: not runtime-proven because gpui-agent cannot expose it and OS input is forbidden.
- Home empty state: visually proven BEFORE and AFTER, but the real-app session was helpdesk-admin/no-profiles rather than literal solo-owner/no-profiles for the reason above.
