# Windows hardening plan: findings, fixes and acceptance

Tracking issue: [#101](https://github.com/hexuria/buwiz-forms/issues/101). Tick items there and here in the same PR.

## Current evidence and scope (2026-10-13)

This is a plan, not authorization to merge, publish, submit to BIR or run the
official application's upload path. Code baseline: `56d7e2eb` on main, including
the all-form agent integration. Latest **visual** evidence was collected on
`47584c85` before that integration; do not claim that newer main was visually tested.
Reproduce on the implementation branch before choosing or declaring any fix.

Evidence: [original Windows checklist](https://github.com/hexuria/buwiz-forms/issues/90#issuecomment-6119512665),
[official comparison correction](https://github.com/hexuria/buwiz-forms/issues/99#issuecomment-6119692230),
[latest visual test](https://github.com/hexuria/buwiz-forms/pull/105#issuecomment-6143307647),
[unsigned installer CI](https://github.com/hexuria/buwiz-forms/actions/runs/38360442158),
[runtime CI](https://github.com/hexuria/buwiz-forms/actions/runs/38374863887).

| Work ID | Finding / evidence classification | State and owning section |
|---|---|---|
| W01 | Debug main-thread stack overflow, confirmed by stderr and visible navigation crash; exact local exit `0xC000041D`; CDB shows bounded request_layout recursion | Implemented on branch, §3 |
| W02 | 2551Q page-two ATC/rate/tax cells blank despite populated editor; incomplete writer joins confirmed in source | Open, tracked here and in #101, §15 |
| W03 | 2551Q printed identity incomplete with a seed lacking confirmed effective profile history | Coverage gap, **not** a confirmed renderer defect, §15.1 |
| W04 | Preview close logs `Invalid window handle` / `window not found`, but release remains usable | Observed warning; cause/severity unconfirmed, §16 |
| W05 | Smoke gate exists in open #105, but debug still fails; visible paint alone cannot prove box correctness | Implemented report-only on branch, promotion pending §1 |
| W06 | Setup, packaging prerequisites and MSIX installation | Implementation merged #103; #91–#93 closed; clean-VM/signed-release acceptance still open §7/§17 |
| W07 | Preview re-entrancy crash and blank WebView | Fix merged #107; #97 closed; release visibly verified; preserve regressions §2 |
| W08 | Duplicate/full-width validation alerts | Fix merged #109; compact inline/summary behavior visually verified; retain wider-form acceptance §17 |
| W09 | Unreadable database silently quarantined | Open #95; product decision D1 before behavior change, §4 |
| W10 | Second launch does not foreground main window | Open #98; mechanism to discriminate on real desktop, §5 |
| W11 | Export confirmation cannot open folder | Open #100, §6 |
| W12 | Installed MSIX fails to launch | Open #94; loader/package-context cause unconfirmed, §8 |
| W13 | Official XML comparison/emulator discrepancies | Open #99; Save/upload distinction corrected; upload oracle needs approval D2, §9 |
| W14 | Hotkey feedback, calendar handoff, existing-profile setup, COR Confirm and manual-entry icon | Individually tracked backlog rows §10 |
| W15 | Receipt printing, Windows Hello, calendar integration, clean host and real foreground behavior | Explicit acceptance gaps, §17 |

W01/W02 are separate from W07: a painted preview host can work while a debug
navigation path crashes or a printed tax box remains empty. One green result
must not close the others. W02–W04 use stable tracking IDs here and in #101;
create individual implementation issues if they need separate ownership.

This plan changes as we work. Update it in the same PR as each change: tick the box, add the PR link, record what was measured, and move anything learned into **Decisions** or **Changelog**. If a step turns out wrong, rewrite it here first, then change the code.

- **Source:** Windows verification of `main` @ `b34bc68` on Server 2022, results in #90 / #89.
- **Skill bar:** `impeccable-rust`:
  - every fix names its oracle;
  - error paths are tested, not just the happy path;
  - each failure class gets an owner that runs in CI;
  - every PR carries a Verification impact block.

Status legend: `[ ]` todo · `[~]` in progress · `[x]` done (with PR link) · `[-]` dropped (say why)

---

## 0. Ground rules for every PR in this plan

1. **Branch naming:** one branch per workstream: `devin/<ts>-win-<slug>`. Commit author is `hexuria <295542929+hexuria@users.noreply.github.com>`. PRs stay open for owner review; nothing is merged without explicit approval.
2. **Order inside each PR:** record the exact base/head; reproduce locally; add a regression that fails before the fix; implement; run the scoped tests, lint/type checks and affected Windows flows locally; inspect the rendered app; only then push and update the PR. CI confirms local evidence, it does not replace it. Documentation-only changes use document/link checks, not an unnecessary app rebuild.
3. **Windows acceptance:** each PR re-runs the #90 steps it touches on a real Windows VM:
   - remove all inherited `BIR_SFTP_*`, then set only `BIR_SFTP_DRY_RUN=1`; never set `BIR_SFTP_LIVE`;
   - test TIN 123-456-788-00000 (branch 00002 may be used for a second profile), scratch DB and isolated LOCALAPPDATA;
   - results go in a PR comment with evidence.
4. **Verification impact block** in every PR description (template in §11).
5. **Repo hygiene:**
   - no generated artifacts, logs or DBs committed;
   - no session or org IDs in the repo;
   - dev runbooks go in `CONTRIBUTING.md` (new), not the README.
6. **No false-green testing:** use a freshly built exact-head binary; compatible Cargo cache reuse is allowed, stale binaries are not. No stack patches or composition overrides in acceptance runs. Experimental stack copies are confined to §3 measurement and never used as proof of the shipped fix. Keep an existing human app/database untouched.
7. **Evidence record per work item:** base/head, OS/toolchain, fixture period/history, command or click sequence, expected/actual result, negative-control result, artifact link, skips with reasons, PR and issue links. Mark implementation complete separately from platform acceptance. In the official eBIRForms app click only Validate and Save; a plan does not approve D2.

---

## 1. Guardrail first: a Windows runtime gate (owner for #96, #97, #98 and later regressions)

**Why:**
- CI already runs `cargo test`, clippy and fmt on `windows-latest`, and all of it was green while the app crashed in normal use.
- None of the Windows failures in #90 is a unit-test failure. They are runtime failures: nested message loops, stack depth, foreground rules and packaging.
- They need an owner that actually *runs the app*.

- [~] **1.1 Integrate the existing report-only gate** from [#105](https://github.com/hexuria/buwiz-forms/pull/105), not a second runner. It uses `scripts/windows_smoke.py`, `file_tax_smoke.AgentClient`, random token authentication and an explicit loopback address. Each phase owns a fresh process/DB. Existing phases: filled 1601C and 2551Q previews, tray hide and dry-run submit. Reconcile with newly merged all-form agent operations before extending it.
  - Correct its fixture using §15.1; select a fixed historical period with an explicitly confirmed profile segment. Keep intentional invalid-period checks in a separate fixture.
  - Record **profile-sidebar navigation** as its own reached step before filing.start so W01 is not mislabeled as a preview-only failure.
  - Keep process/agent liveness, semantic readiness, and painted-content checks distinct. A failed capture is SKIPPED, not a passed visual check. Add value correctness at the Rust HTML boundary (§15), not only a dark-pixel threshold.
  - Retain diagnostic exit code and stderr independently. `0xC000041D` is a callback exception; only the stack-overflow log/debugger establishes that cause. Do not rename it `0xC00000FD`.
  - Local commands after integration: `just smoke-windows debug`, then `just smoke-windows release`. Both must pass after W01, plus a visible filled-form run on this Windows VM before pushing.
- [ ] **1.2 Turn the existing PE reader into a regression assertion** after §3 chooses the measured reserve. Check both Cargo's `bir-desktop.exe` and its shipping `bir.exe` alias; fail a reserve below policy. Test truncated/non-PE/wrong optional-header input and prove a 1 MiB fixture fails the new policy.
- [ ] **1.3 Workflow audit:** scope `zizmor --persona=auditor` to changed workflows, triage findings without weakening repository security. This is separate from fixing the application crash.
- [ ] **1.4 Promote only after evidence:** remove report-only behavior only after debug/release local runs and matching CI succeed, with fixture/paint coverage reported. Require three consecutive clean executions per profile on the same revision (a retry does not erase a failure). Setting branch-protection required checks is an **owner action**, not authorized here. Never disable a failing phase to turn CI green.

**Oracle:** the user-visible behaviour in #90 (process alive, window present). The agent snapshot is the observation channel.
**Blind spots, written down:**
- GitHub runners have no GPU (WARP renderer).
- Foreground-window rules on a service desktop differ from a real session.
- The mailbox drains while `AppState::render` runs; a hidden app cannot reliably receive agent operations. Tray show/graceful exit stay explicit manual acceptance, not claims made from the hide-only phase.
- So §5's foreground check stays a manual VM step.

---

## 2. #97: Print Preview crashes the app (P0)

**Root cause (confirmed by backtrace):**
- `FrozenHtmlPreviewView::new` runs inside `cx.open_window(...)`'s build closure (`form_html_preview_launcher.rs:138`), while GPUI's `App` RefCell is mutably borrowed.
- `wry::WebViewBuilder::build_as_child` (`frozen_html_preview.rs:189`) calls WebView2 `create_controller`, which pumps a nested Win32 message loop.
- GPUI's foreground executor runs `BlinkCursor::blink` inside that pump, and `AsyncApp::update_entity` tries to borrow `App` again, so it panics.
- macOS creates WKWebView synchronously with no pump, which is why only Windows breaks.
- It's the only `build_as_child` call site in the repo.

- **Implementation note (2026-10-12):** The confirmed chain is `launch_frozen_html` → `cx.open_window` → `FrozenHtmlPreviewView::new` → `build_as_child` → WebView2 `create_controller` → nested `wait_with_pump` → `BlinkCursor::blink` → `AsyncApp::update_entity` → `RefCell already borrowed` at `gpui/async_context.rs:65`. Windows now defers construction with `cx.spawn_in(window, ...)`, yields once, copies the raw handle in a short `update_in`, and builds WRY after that borrow returns. The second update installs `Ready` or `Failed`; macOS remains synchronous.
- **Second root cause (2026-10-12):** The WebView2 child was visible and correctly sized according to `EnumChildWindows` (`WRY_WEBVIEW` and `Chrome_WidgetWin_*`), and JS metrics loaded, but the embedded area remained white. Setting `GPUI_DISABLE_DIRECT_COMPOSITION=1` made the filled 1601C and 2551Q documents render. gpui-pre-windows' DirectComposition visual was covering the WRY child HWND; the startup fallback now prefers the HWND swap-chain path while honoring an existing override.
- [x] **2.1 Two-phase construction** merged in [#107](https://github.com/hexuria/buwiz-forms/pull/107).
  - `FrozenHtmlPreviewView` uses explicit Pending, Ready and retained Failed states; use the actual types in `frozen_html_preview.rs`, not a parallel state model.
  - `new()` only stores `Pending` and schedules creation with `cx.spawn_in(window, ...)`, yielding with a zero-duration background timer after `open_window` returns and the borrow is released.
  - The deferred step builds the WRY child and moves to `Ready` or `Failed`.
  - `render()` matches on the state, so "rendering a webview that doesn't exist yet" can't be written.
- [ ] **2.2 Audit remaining operations, not a claimed completed fix:** construction is deferred and operations require Ready, but this does not prove every print/evaluate callback is re-entrancy-safe. Include close/late-callback investigation under §16; retain missing-runtime and print-dialog checks under §17.
- [x] **2.3 #90 step 17 fallback implementation** merged in #107: retained failed-preview state and **Open in browser** reuse `write_frozen_html_document`. Toolbar operation errors remain separate from construction errors. Runtime-absent coverage used simulation, not a clean machine without WebView2.
- [~] **Tests:**
  - Unit: the state machine (`Pending→Ready`, `Pending→Failed`, no transition out of `Failed`).
  - Gate: §1.1 steps 2 and 3, with readiness exposed to the agent.
  - Manual: #90 step 16 on a VM (window about 1200×900; Print opens the WebView2 dialog with A4 and no scaled margins), plus step 17 by setting `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` to an empty folder.
- **Also verify** that macOS and Linux print preview are unchanged: run the existing metrics-parsing/state tests in `frozen_html_preview.rs` and test the affected agent dispatch when it changes. `agent/drain.rs::print_preview_metrics` is a method, not an existing dedicated metrics test. A visible macOS preview and Linux browser-fallback check remain platform acceptance; neither is certified by the existing drain tests.

**Changelog**

| Date | Result |
|---|---|
| 2026-10-12 | Implemented the Windows two-phase WebView construction and explicit pending/ready/failed state. The first main-branch smoke results were release: both previews panic with `RefCell already borrowed` (#97); debug: 1601C panics and 2551Q overflows its stack (`0xC00000FD`, #96); submit passes in both; `stack_reserve` is 1 MiB. |
| 2026-10-13 | W01/#96 measured and fixed on branch: CDB proves bounded layout recursion in debug (`__chkstk` in `dashboard::render`, ~150 GPUI request_layout frames), 1 MiB crashes 4/4 at taxpayer-sidebar, 2 MiB survives; shipped binary gets `/STACK:8388608` via `crates/bir-desktop/build.rs` plus a CI PE-header regression (`scripts/check_pe_stack_reserve.py`). |
| 2026-10-12 | Confirmed the second #97 rendering cause: DirectComposition hid the visible, correctly sized WebView2 child HWND. With `GPUI_DISABLE_DIRECT_COMPOSITION=1`, filled 1601C and 2551Q previews rendered; the main window continued to paint normally. |

---

## 3. W01 / #96: debug stack overflow (P0 for development)

**Facts:**
- The measured current Windows debug PE reserve is 1 MiB. Do not infer another OS's actual reserve from a conventional default.
- `[profile.dev]` leaves workspace crates at opt-level 0 (only deps get 1), so GPUI render frames are large.
- Earlier 8 MiB experimental copies ran the exercised paths; that is supporting evidence, not a measurement on newer main or a fix in the shipped binary.
- Latest local release previews pass. Unpatched debug maximizes successfully, then crashes on taxpayer-sidebar navigation before 2551Q opens. Stderr says main-thread stack overflow; exit is `0xC000041D`.
- **Cause diagnosed (2026-10-13):** CDB on the unpatched 1 MiB debug binary catches `c00000fd` at `bir_desktop!__chkstk` entering `bir_desktop::views::dashboard::impl$2::render` (dashboard.rs). ~150 bounded frames of `gpui` element `Interactivity::request_layout` + `smallvec` `Extend` + `with_element_state`/`with_text_style`/`with_image_cache` wrappers — deep-but-bounded layout recursion per nesting level, inflated by unoptimized debug frames. Not unbounded recursion, so a PE reserve policy is a legitimate fix; no single surprising frame to follow up on.

- [x] **3.1 Measure before choosing a number.**
  - Built exact-head debug (`3169105b`, sha256 `096daea8...`) and release once; originals untouched; diagnostic copies patched to 1/2/4/8 MiB via PE edit.
  - Flow exercised: maximize → taxpayer sidebar → dashboard forms tab → filled 1601C + 2551Q → close/reopen print preview ×2 each → COR editor → hide/tray restore, three times per configuration.
  - Debug matrix: **1 MiB crashes at taxpayer-sidebar 4/4** (3 patched + original baseline, `0xC000041D`, stderr main-thread overflow). **2 MiB survives every run** (3/3 reached filled-form previews; 1 full end-to-end pass including COR editor and hide/restore). 4/8 MiB: no crashes in any run.
  - Release matrix: full flow **passes 3/3 at every reserve** — 1, 2, 4 and 8 MiB — confirming optimized frames shrink the same recursion below 1 MiB.
  - Debugger stack captured on the unpatched failure (see facts above); recursion is bounded.
- [x] **3.2 Fix at the binary, not RUSTFLAGS.**
  - `crates/bir-desktop/build.rs` emits `cargo:rustc-link-arg-bin=bir-desktop=/STACK:8388608` only when `CARGO_CFG_TARGET_OS=windows` and `CARGO_CFG_TARGET_ENV=msvc`. Per-bin flag: `bir-headless`, tests and other crates keep platform defaults.
  - Chosen reserve **8 MiB** — smallest consistently passing debug reserve is 2 MiB; 8 MiB is ~4× margin over observed need, matching the earlier experimental evidence, with headroom for deeper element trees.
  - Regression: `scripts/check_pe_stack_reserve.py` + `scripts/tests/test_pe_stack_reserve.py`; CI `check` job verifies the release `bir-desktop.exe` after `just exe` and a debug `bir-desktop.exe`, both on Windows only. Reverting `build.rs` fails the PE check.
- [x] **3.3 Find the deep frames:** `c00000fd` at `__chkstk` inside `dashboard::render`; the depth is ~150 small GPUI layout frames (request_layout/smallvec/element-state wrappers), not a big-array frame — no follow-up issue needed.
- **Owners:** §1.2 (PE header) and §1.1 (debug and release runs).
- **Acceptance:** rebuild without editbin/environment overrides; both executable names meet PE policy; all navigation and preview steps pass three consecutive times in debug/release; macOS/Linux compilation unchanged. Run `cargo fmt --all -- --check`, focused build-script/policy tests, agent-feature desktop Clippy, then the normal workspace tests once. Publish the local visible recording before claiming the runtime fixed. Reverting the linker policy must fail the PE regression.

---

## 4. #95: unreadable database is moved aside silently (P1, data trust)

**Facts:**
- `Database::open_or_recreate` quarantines to `bir_data.db.corrupt-<ts>.bak` and creates a fresh DB.
- `open_live_database()` returns `(Database, PathBuf, Option<PathBuf>)`.
- `gui.rs:288` only `eprintln!`s the `Option`.
- A raw encrypted DB from another machine may be unreadable without the matching key; do not label every unreadable file as corruption or infer the cause from a generic SQLCipher error.

- [ ] **4.1 Make ignoring it hard (type-enforced).**
  - Replace the tuple with `#[must_use] pub struct LiveDatabase { db, path, recovery: Option<Recovery> }` and route desktop initialization through a helper that persists/reports recovery before exposing the database. `#[must_use]` on the struct alone cannot enforce consumption of its recovery field; test the initialization boundary explicitly.
  - `Recovery { backup_path, reason: RecoveryReason }`, with `RecoveryReason` one of `WrongKeyOrForeign | NotADatabase | Truncated`. If SQLCipher's errors can't tell these apart, use one variant and write that down.
- [ ] **4.2 Make it visible and persistent.**
  - Write a high-severity alert row (`db/alerts.rs`) into the new DB, so the bell shows it after restarts.
  - Also show a first-frame banner: "We couldn't open your previous data (it may come from another computer or a different key). Nothing was deleted: it was moved to <path>." with **Open folder** and **How to move data between computers** (Export/Import).
- [ ] **4.3** Document in `docs/platforms/*.md` that a raw DB copy is not portable; use Export All Profiles and Import.
- **Decision needed:** keep auto-quarantine plus a loud notice (proposed), or refuse to start until the user chooses?
- **Tests (bir-core, every OS):**
  - **property-tested:** any random-byte file opens with `recovery: Some`, the backup's SHA-256 matches the input, and the fresh DB opens;
  - a DB under key A opened with key B reports recovery without claiming corruption (use a generic unreadable reason if the engine cannot discriminate);
  - retention pruning keeps exactly N backups;
  - error-path litmus: a failed rename (read-only dir) must be `Err`, never a silent fresh DB.
  - Desktop: the alert row exists after a recovered open. #90 step 7 is the manual acceptance.

---

## 5. #98: second launch while hidden doesn't bring the window to the front (P2)

**Facts:**
- The second instance already calls `AllowSetForegroundWindow(ASFW_ANY)` (`ipc.rs:38`) before sending `SHOW`.
- The receiver's `show_in_dock()` runs `EnumWindows` over **every** window of the process (`platform/windows.rs` `show_window_callback`), calling `SW_RESTORE`, `SW_SHOW` and `SetForegroundWindow` on each one.
- This can include GPUI and tray helper windows. Wrong-window selection is a leading mechanism, not a proven explanation of every failed foreground request; Windows foreground policy may also reject the request. Log selected HWND/return values without taxpayer data to discriminate.
- `toggle_window_callback` has the same pattern.

- [ ] **5.1** Keep the main window's `HWND` (from `raw-window-handle`) in a typed `MainWindow` handle. Show, hide and toggle act on it alone, and the `EnumWindows` callbacks are deleted.
- [ ] **5.2** `SW_RESTORE` then `SetForegroundWindow`. If `GetForegroundWindow() != hwnd`, call `FlashWindowEx` so it at least flashes in the taskbar. No `AttachThreadInput` hacks.
- [ ] **5.3** Wrap each Win32 call in a small safe fn with one `SAFETY:` note. Miri can't run FFI, so review plus the smoke gate own this.
- **Tests:**
  - put the Win32 calls behind a trait, and unit-test that show/hide/toggle only ever touch the main `HWND`;
  - #90 steps 8, 9 and 13 on a VM, since the runner desktop can't be trusted for foreground checks.

---

## 6. #100: export confirmation has no "Open folder" (P2)

**Facts:**
- `views/settings.rs:998` uses `rfd::MessageButtons::OkCancelCustom("Open folder", "OK")`.
- On Windows rfd shows a plain OK/Cancel MessageBox and returns `Ok`/`Cancel`, never `Custom("Open folder")`. So Open folder can never fire.
- It's the only `*Custom` button use in the repo.

- [ ] **6.1** Replace the native box with an in-app dialog (gpui-component), the same on every OS: "Saved to <path>" with **Open folder** / **Close**.
- [ ] **6.2** If any native box stays, route its result through one pure `fn export_choice(MessageDialogResult) -> ExportChoice` and unit-test every variant.
- **Tests:** the mapping unit test, and #90 step 18 on a VM.

---

## 7. Contributor tooling: #91, #92, #93 (P1, every new Windows contributor hits these)

Implementation for this tooling slice is complete, but acceptance remains
pending a clean VM and a signed release run. 7-Zip extraction is prohibited:
it does not model Inno Setup installation behavior. The package path is staged
once and gated by strict `[Files]` source validation before ISCC. The shared
audit is implemented to install the actual Setup.exe into a disposable
per-user directory and audit its installed tree before uninstall; local
execution was safely refused because a real installation is registered.

- [x] **7.1 #91, upstream in `hexuria/gpui-agent`.** The bcrypt linkage fix and
  pinned-revision update are merged upstream ([gpui-agent PR #50](https://github.com/hexuria/gpui-agent/pull/50)).
- [x] **7.2 #91 `scripts/setup-windows.ps1`:** Root-manifest pin resolution, PATH
  ordering/deduplication, and 3010 handling passed PowerShell parsing and the
  focused setup-policy test ([PR #103](https://github.com/hexuria/buwiz-forms/pull/103)).
- [x] **7.3 #92 `justfile`:** Python/ISCC prerequisites and source gating were
  verified with the real Windows packaging build ([PR #103](https://github.com/hexuria/buwiz-forms/pull/103)).
- [x] **7.4 #93 MSIX installation:** `just install msix` invokes
  `Add-AppxPackage` through Windows PowerShell 5.1, which installed the package
  in the Server 2022 VM verification. The evidence records that the installed
  app still failed to start under the separate #94 issue ([#90 evidence](https://github.com/hexuria/buwiz-forms/issues/90#issuecomment-6119512665);
  [PR #103](https://github.com/hexuria/buwiz-forms/pull/103)).
- [x] **7.5 Unsigned final Setup audit:** The shared script and parser checks
  passed disposable Windows CI's actual build/install/audit/uninstall path
  ([run](https://github.com/hexuria/buwiz-forms/actions/runs/38360442158/job/115173162450)).
  Local execution safely refused the existing eBIRForms installation.
  Never remove a pre-existing installation.
- **Acceptance:** a clean VM must run setup, `just run`, `just exe`, `just msix`,
  `just sign-dev`, `just install msix`, and `cargo install gpui-agent-cli`.
  A signed release run remains a separate acceptance item. Local verification
  on the existing VM is partial evidence, not clean-VM certification.

---

## 8. #94: installed MSIX doesn't start (P3: MSIX is Store-only and already marked BLOCKED in `justfile`)

**Facts:**
- Activation is logged as successful, but no process is ever seen and no app log is written.
- `bir.exe` imports `VCRUNTIME140.dll` and `VCRUNTIME140_1.dll`. The package ships `vcruntime140.dll` and `msvcp140.dll` copied from System32, but not `vcruntime140_1.dll`.
- The release build uses `panic = "abort"` and has no console.

**Hypotheses (unconfirmed):**
- (a) incomplete or mixed packaged VC runtime;
- (b) package-context startup (CWD `System32`, virtualized AppData) aborts before tracing starts;
- (c) the Server 2022 SKU.

- [ ] **8.1 Diagnose:** loader snaps (`gflags +sls`) or Procmon on activation, `Invoke-CommandInDesktopPackage` with a console, and a Windows 11 retest. Record the result here.
- [ ] **8.2** If existing diagnostics are insufficient, add an opt-in early startup breadcrumb before DB/UI initialization. Log only startup stage/error category, never taxpayer fields, DB contents or credentials. A panic hook alone cannot diagnose a loader failure before Rust starts; preserve loader/activation evidence from 8.1.
- [ ] **8.3** If (a): depend on the `Microsoft.VCLibs.140.00.UWPDesktop` framework in `assets/windows/AppxManifest.xml` instead of copying DLLs. If (b): fix the startup path.
- **Acceptance:** #90 step 4 launch on Windows 11 and Server 2022.

---

## 9. #99: official-XML parity, 1702-RT and the emulator behind all 78 samples (P0 for filing correctness)

**What the HTA source shows** (`BIR-Form1702RTv2018C.hta`, 7.9.6.2.1). The #89 comparison mixed two different official save paths:

| Path | Trigger | Name/LOB/Address | Trailer | Separator |
|---|---|---|---|---|
| `saveXML(false)` → `C:\eBIRForms\savefile` | **Save** button (lines 90/289/3731) | `escape()`d (10323–10332) | `BIR 2014.` (no `0` unless final copy, 10414–10418) | `xmlFormat.innerHTML` |
| `saveEncryptedProfile(true)` → SFTP upload | Submit (line 9225) | plain `elem.value` | `BIR 2014.0` (9993) | `xmlFormat.innerHTML` |

`*.official.xml` models the **upload** path (sample README). So, of the five #89 differences:
- **2 (trailer) and 3 (`escape()`) are expected path differences, not bugs.**
- **1 (CRLF):**
  - the HTA's `<xmp id='xmlFormat'>` holds `\t\n` plus spaces (LF bytes), and real IE wrote CRLF, so IE normalizes line breaks in `innerHTML`;
  - both paths use the same `tab`, so the upload almost certainly carries CRLF too, while jsdom keeps LF. Likely real, and it would affect **every** sample and Rust writer;
  - this is inferred, not observed, because Submit is off-limits.
- **4 (disabled debit-memo controls):** a real emulator gap. `official.js setValue()` writes `disabled` controls, and the recipe's steps 174–177 are impossible in the real app.
- **5 (`txtPg4Sc3I2TotalDeductions` 0 vs 2,181,125):** a real gap. The emulator doesn't fire the handler sequence IE fires, so one recompute never runs.

- [x] **9.1 Correct the record:** [#99 correction](https://github.com/hexuria/buwiz-forms/issues/99#issuecomment-6119692230) and [#89 correction](https://github.com/hexuria/buwiz-forms/issues/89#issuecomment-6119692413); do not "fix" expected Save/upload differences.
- [ ] **9.2 Upload-path oracle without Submit.** In a scratch copy of the HTA, a harness calls `saveEncryptedProfile`'s plaintext builder (or `saveXML(true)` with the final-copy writes redirected), and captures `allXML` before encryption and before any SFTP code.
  - It must never touch Submit, Final Copy, the network or `RenameAndSendFile`.
  - **The owner approves the harness before it runs**, since it goes beyond Validate and Save.
- [ ] **9.3 Real-capture corpus:**
  - for N representative forms (start with 1601C, 2551Q, 1702-RT, 1702Q, 0619E), capture real-IE plaintext on Windows with the #89 driver: synthetic TIN, Validate plus Save, plus the 9.2 harness;
  - store it as `*.ie.xml` fixtures with provenance (package version and sha, Windows build, recipe hash).
- [ ] **9.4 Fix the emulator (`tools/official-xml`):**
  - `setValue` refuses `disabled`/`readOnly` controls and reports the step as invalid;
  - distinguish user-input recipe steps from the official handlers' programmatic writes to computed fields; do not block valid derived-value updates while enforcing user-editability;
  - fire IE's event order for typed input (keydown/keypress/keyup, then change, then blur; verify against the HTA handlers);
  - normalize the `xmlFormat` separator only after the approved upload capture establishes its bytes; do not regenerate every sample on the CRLF hypothesis alone;
  - keep the `+ '0'` trailer, which matches upload line 9993.
- [ ] **9.5 Conformance test (differential):**
  - `node runtime.js` output must equal the `*.ie.xml` capture byte for byte for every captured recipe;
  - the real HTA under IE is the oracle and jsdom is the model, so this is the conformance link the skill requires before the emulator's samples count as evidence;
  - runs on any OS from the stored captures, with Windows needed only to refresh them.
- [ ] **9.6** Regenerate the 78 samples (`UPDATE_SAMPLE_PAYLOADS=1`). Update the Rust writers (`official_xml.rs` and the form writers) until `sample_payloads_test` is green again. Fix the 1702-RT recipe (drop steps 174–177, or enable item 23 the way the real form does).
- **Decision needed:** is CRLF what BIR's upload actually carries? Byte parity says yes; 9.2 settles it.

---

## 10. Smaller gaps found in #90 (P3 backlog: file an issue when picked up)

- [ ] Step 13: a hotkey that fails to register only logs WARN (`gui.rs:576`). Show the error inline in Settings; the recorder should say when a combo is taken.
- [ ] Step 25: the toast says "handed to your calendar app" even when no app handles `.ics`. Fall back to "Saved to … / Open folder".
- [ ] "Set up forms for this year" on an existing profile opens a blank create-profile editor, and saving gives a duplicate-TIN error. Not Windows-specific.
- [ ] COR **Confirm** silently does nothing while the editor has unapplied changes. Disable it, or say why.
- [ ] `Failed to load asset: Embedded resource not found: manual_entry` when a manual COR entry is opened.
- [ ] Step 19 is untested on Windows: the email receipt Print path (2551Q only). Cover it in the smoke script once a 2551Q profile fixture exists.

---

## 11. Verification architecture (impeccable-rust audit summary)

**Current verifiers:** workspace tests, Clippy and fmt on macOS/Linux/Windows; sample-payload byte comparisons; frozen-HTML inventory/stamp checks; bir-core property tests; Python script tests and actual `just --summary` on all OSes; disposable unsigned Windows installer audit. Runtime smoke exists on #105, still report-only, not yet on this plan's main baseline. Missing coverage is tracked explicitly below rather than treating green unit tests as runtime evidence.

| Failure class (this plan) | Owner | Verdict |
|---|---|---|
| GPUI vs Win32 re-entrancy (#97) | §1.1 smoke gate plus the §2 state-machine unit tests | REQUIRED |
| Stack exhaustion (#96) | §1.2 PE check plus the §1.1 debug and release runs | REQUIRED |
| Data-loss perception on recovery (#95) | bir-core property and error-path tests (§4) | REQUIRED |
| Wrong-window Win32 calls (#98) | trait-injected unit tests plus manual VM steps | REQUIRED |
| Dialog semantics differ by OS (#100) | pure mapping unit test, or the in-app dialog | REQUIRED |
| Official-XML parity (#99) | §9.5 differential conformance against real-IE captures | REQUIRED |
| Missing printed fields (W02) | §15 writer-map-to-rendered-cell assertions plus visible official-layout comparison | REQUIRED |
| Invalid smoke fixture (W03) | §15.1 confirmed history invariant and unresolved-profile negative case | REQUIRED |
| Late preview-close work (W04) | §16 lifecycle regressions and close/reopen Windows acceptance | REQUIRED after diagnosis |
| Contributor setup drift (#91–#93) | clean-VM acceptance per PR; nightly `setup-windows` job | USEFUL |
| Vulnerable or unvetted deps | `cargo deny` (RUSTSEC) in CI | USEFUL |
| Compromised workflows | `zizmor --persona=auditor` | USEFUL |
| Flaky tests | nextest with `flaky-result = "fail"` | OPTIONAL |
| Win32 `unsafe` (~93 sites) | review, safe wrappers, smoke gate. Miri can't run FFI | Miri: NOT JUSTIFIED |
| Loom, TLA+, Kani, provers | no concurrency kernel or protocol in scope | NOT JUSTIFIED |

**Anti-drift:** keep the existing `CONTRIBUTING.md` verification rules and dev runbook current. Do not add a second smoke protocol client or duplicate state machine. The emulator (jsdom) counts as an official oracle only through §9.5 conformance. For new pure Python fixture/PE policy logic, use focused unittest error-path/negative-control tests and available lint/type checking; record unavailable tools, never imply they ran. Rust unsafe/FFI changes need boundary review plus real platform acceptance; Miri cannot execute WebView2/Win32. New formal models are not justified for these deterministic mappings and scoped UI fixes.

**Verification impact template** (copy into each PR):
```text
Verification impact
[ ] Pure Rust deterministic behavior
[ ] Concurrency / interleaving behavior      (GPUI executor vs Win32 pump: §2)
[ ] Crash / recovery / replay behavior       (DB quarantine: §4)
[ ] Persistence semantics                    (alert row, DB path: §4)
[ ] Unsafe / memory behavior                 (Win32 FFI: §5)
[ ] Workflow / DSL semantics                 (official-XML emulator: §9)
[ ] Property-test / fuzz surface
[ ] No verification architecture impact
Reason:
Affected invariants:
Tests or proofs updated:
Windows acceptance (#90 steps re-run, VM evidence):
```

---

## 12. Order of work and PR slicing

| # | PR | Covers | Depends on | Gate |
|---|---|---|---|---|
| Done | tooling / preview host / validation summary | #103 / #107 / #109 | merged | Preserve regressions; outstanding acceptance remains §17 |
| A | reconcile existing smoke branch and fix its profile fixture | W03/W05, §1/§15.1 | current main includes all-form agent; #105 is open | Local script tests; seeded profile resolves full filing period; release visual baseline |
| B | `win-stack` | W01, §3 | A's reproducible fixture (or equivalent valid manual fixture) | Measured reserve, PE regression, unpatched debug/release navigation and filled previews |
| C | `2551q-print-fields` | W02, §15.2–15.4 | A; can develop independently of B using release | Cell-value regression red→green, official layout oracle, all sample bytes unchanged |
| D | `preview-close-lifecycle` | W04, §16 | A's release baseline | Diagnose log owner first; close/reopen stress without stale-window work |
| E | promote smoke gate | W05, §1.4 | A+B; C's field assertions included before claiming content completeness | Three clean local/CI runs per profile; no disabled phases; owner controls branch protection |
| F | `db-recovery-notice` | W09 / #95, §4 | D1 | Backup preservation and failure-path tests + visible persistent notice |
| G1 | `win-window-focus` | W10 / #98, §5 | real Windows desktop | Only main HWND targeted; foreground or explicit flash fallback |
| G2 | `export-open-folder` | W11 / #100, §6 | none | In-app action opens correct folder; cancel/no-handler errors tested |
| H | `official-xml-ie-parity` | W13 / #99, §9 | D2 before capture | Real capture → emulator conformance → reviewed fixture/Rust writer changes |
| I | `msix-start` | W12 / #94, §8 | diagnose on disposable Windows client/server | Activation + persistent process + first painted window, not activation event alone |
| J | backlog and acceptance | W14/W15, §10/§17 | per item | One focused fix/acceptance slice each, no invented PASS for unavailable prerequisites |

Start A/B next, then C. H is an independent correctness track once approved;
do not block diagnosis on unrelated UI polish. Each PR contains only its slice,
plus its regressions and plan update. No application fixes are part of this
documentation PR. After E, rerun the full #90 checklist and publish a new
baseline that names remaining manual/credential/hardware skips.

---

## 13. Decisions

| Date | Decision | Status |
|---|---|---|
| | #95: auto-quarantine plus loud notice, or refuse to start? | **open (owner)** |
| | #99: approve the upload-path plaintext harness (9.2) that goes beyond Validate and Save? | **open (owner)** |
| | #99: does BIR's upload carry CRLF (parity target)? | open: settled by 9.2 |
| 2026-10-13 | #96: stack reserve size | **8 MiB** — smallest passing debug reserve 2 MiB, ~4× margin; §3.1-3.3 |
| 2026-10-13 | #97: deferred WebView construction + production DirectComposition fallback | implemented #107; latest release previews visibly paint |
| 2026-10-13 | Keep W02 print mapping separate from W03 unresolved profile identity | missing schedule mappings are confirmed; identity must be retested with valid history |
| | Smoke branch-protection requirement | owner action after §1.4 evidence; no settings change authorized by this plan |

## 14. Changelog
- Plan created from the #90/#89 Windows verification. Corrected the #99 assessment: diffs 2 and 3 come from different official save paths.
- **2026-10-12** — Added the Python Windows smoke design and report-only
  `stack_reserve` measurement. Initial `main` results: release previews both
  panic with `RefCell already borrowed` (#97); debug 1601C panics and 2551Q
  overflows its stack (`0xC00000FD`, #96); submit passes in both profiles;
  `stack_reserve` is 1 MiB.
- 2026-10-13 | Replaced 7-Zip payload inspection with strict source validation and actual installed-Setup auditing; implementation is distinct from pending clean-VM and signed-release acceptance.
- 2026-10-13 | Re-review follow-up: CI parses the justfile on all three OSes, the source auditor rejects unsupported ISPP directives including StageDir overrides, and AGENT.md follows the repository-root gpui-agent pin. Clean-VM and signed-release acceptance remain unverified.
- 2026-10-13 | Restricted inline ISPP expressions across the entire installer to the exact MyAppVersion and StageDir tokens; assignment and Define expressions outside the Files section now fail the source gate.
- 2026-10-13 | Reconciled merged fixes and latest visual findings; corrected Cargo linker target and callback-exception attribution; added print mapping, fixture history, lifecycle diagnostics and a local-before-push acceptance matrix. Latest main includes #110 but has not been visually retested by this plan.

---

## 15. W02/W03: 2551Q printed values and valid profile fixtures (P1)

**Observed:** release editor displays PT010 / 3% / 60.00 for taxable amount
2,000.00; printed page two displays the taxable amount but leaves ATC/rate/tax
cells blank. Page-one total tax displays 60.00. This is independent of WebView
painting and must not be repaired by changing the tax calculation to fit a
preview snapshot.

**Confirmed code boundary:**
- `crates/bir-core/src/forms/form_2551q_xml.rs` writes schedule keys including
  `txtATCAmt1`, `txtATCRate1` and `txtATCDue1`; the ATC key and row loop are in
  that same writer. Its existing rate tests expect the XML form `3.0`, not a
  percentage label invented by the UI.
- `html-frozen/2551q-2018/writer-cells.json` joins `txtATCAmt1` and page-one
  `frm2551Qv2018:txt14`, but explicitly lists `txtATCDue1` as not joined.
- `crates/bir-print/src/frozen_html.rs::{fill_by_name_with_cells,fill_2551q,filled_2551q_document}`
  ignores unmatched keys and only fills known direct names/approved joins.
  An emitted value is therefore not evidence that its physical print box is filled.
- `Form2551QView::{preview_draft_snapshot,preview_pdf}` in
  `crates/bir-desktop/src/views/form_2551q_view.rs` uses in-memory Draft data,
  but reloads persisted non-Draft data. Both semantics must be preserved.

### 15.1 Repair the fixture before diagnosing identity (W03)

- [ ] Replace #105's flat profile/forms-set-only `seed_profile` with confirmed
  effective profile/COR history covering the **entire** selected quarter, via
  existing profile/COR APIs (no direct production-table SQL shortcuts).
- [ ] Assert before preview: `effective_profile_version_id` present,
  `profile_resolution_error` absent, expected taxpayer name/address resolved.
  `Form2551QDraft::{new_from_effective_profile,reconcile_with_effective_profile}`
  in `form_2551q.rs` is the authority. The unresolved branch intentionally
  retains stable TIN rather than silently falling back to flat profile values.
- [ ] Keep a separate negative test: missing, overlapping or insufficient
  effective history remains unresolved and blocks filing; never loosen that
  check merely to make the smoke fixture pass.
- **Oracle:** confirmed profile segment → draft snapshot → writer map → printed
  name/address. Existing `filled_document_2551q_fills_taxpayer_name_and_address`
  covers a filled draft, not successful profile resolution in the application.
- **Acceptance:** local visible run with valid history and a historical filing
  period. If printed identity is then still missing, capture the four boundary
  values and open a focused bug; do not assert a root cause before that check.

### 15.2 Complete the missing cell mappings (W02)

- [ ] Inventory each Schedule 1 row against the official printed sheet and
  current `writer-cells.json`/HTML inputs; record source writer key, physical
  cell IDs, text/rate vs peso+cent type, row and expected display.
  Do not guess box IDs by visual proximity. Verify how all six first-sheet
  rows and any continuation sheet are produced.
- [ ] Add deterministic Rust regressions at `filled_2551q_document`: PT010,
  2,000.00 at the period-correct 3% gives 60.00 in its actual row tax cells
  and page-one total. Assert exact destination values/comb slots, not global
  substring presence (page-one 60.00 could otherwise hide page-two omission).
- [ ] Cover one distinct second ATC row, cents, empty unused rows and a
  continuation case if supported by this rendering path. Derive expected
  values from the official rate/domain and existing writer, not from the
  renderer under test. Preserve the rule that >6 ATCs cannot be submitted
  without a verified official attachment protocol.
- [ ] Fill text/rate cells through vetted joins and tax amounts through
  `money_joins`' peso/cent splitting; never stamp synthetic official field
  names or put a dotted amount into a peso comb. Update provenance in the
  manifest and remove only now-covered `not_joined` entries.
- [ ] Locate the corresponding maintained generator/catalog in
  `hexuria/buwiz-forms-html` before editing generated bundles. Carry the source
  change and regenerated join artifacts together so regeneration cannot erase
  the fix. No unreviewed broad regeneration of tax payloads.

### 15.3 Verification commands and acceptance

- Focused existing checks first: `cargo test --locked -p bir-print filled_document_2551q`
  and the new destination-cell regressions; `cargo test --locked -p bir-core form_2551q`.
- Then `cargo test --locked -p bir-core --test sample_payloads_test` (must retain
  XML/IAF bytes for a print-only change), `cargo test --locked -p bir-print`,
  scoped Clippy, rustfmt, and the existing frozen-inventory/name-stamp checks.
- For any changed agent-derived definition, run its derivation regression in
  isolated and workspace-unified feature modes. Preserve canonical object-key
  order (`serde_json/preserve_order` must not make fixtures appear stale).
- Locally rebuild release and debug after W01. Enter values visibly, preview
  both pages, compare the populated editor with the specific physical cells,
  close/reopen and inspect printer preview without printing/submitting.
- A non-Draft persisted-snapshot regression must show that later profile or
  editor changes do not rewrite the historical printed return.

### 15.4 Earlier 1601C print observations

- [x] Item 11 category mapping has a dedicated merged fix (#108); latest
  filled release preview paints. Preserve that regression rather than redoing it.
- [ ] Recheck earlier items 15–19 blank-field observations with valid profile
  history and an intentionally populated fixture; latest 1,000/100 demo did
  not establish completeness for every box. Trace through
  `Form1601CDraft::to_bir_field_map`, `frozen_html::filled_document` and
  `html-frozen/1601c-2018/writer-cells.json`. If missing joins are reproduced,
  apply §15.2's per-cell oracle in a separate focused print-mapping commit.

---

## 16. W04: preview-close warnings (P2 diagnosis, not a proven crash)

**Observed:** release logs include `Invalid window handle` / `window not found`
when previews close, but the app remained usable. Do not suppress the messages
or call them harmless without identifying the producer.

- [ ] Attribute the log with span/timestamp/backtrace in a diagnostic build;
  determine application callback vs GPUI/WRY cleanup, close-before-ready vs
  ready-state close, and whether work targets an expired preview generation.
- **Source seams:** `frozen_html_preview.rs` deferred `cx.spawn_in` construction,
  `request_js_metrics`, `Drop`, generation-keyed `PreviewSlot`; and
  `form_html_preview_launcher.rs::HtmlPreviewLaunchKind::observe_close`. Do not assume that
  holding the metrics Arc proves the native window is still valid.
- [ ] If application-owned: cancel/ignore only demonstrably stale work using
  weak window/entity ownership and the current generation, never a global
  catch-and-ignore. If dependency-owned: capture minimal reproduction and
  record an upstream issue/fix pin rather than changing unrelated lifecycle code.
- **Regressions:** close while Pending, close while Ready, immediately open a
  replacement, then deliver the old callback; old completion cannot mutate or
  clear the new preview. Keep existing generation-slot unit tests as the seam.
- **Local acceptance:** ten open/close cycles for each filled form in release
  and debug after W01, including rapid early close; no crash, no leaked live
  child window, no application work against a closed handle. Remaining known
  dependency warnings require an explicit disposition and evidence, not silence.
- **Done:** root cause + owning layer recorded, regression demonstrates the
  fix, and the same sequence has clean or explicitly accounted-for logs.

---

## 17. Platform acceptance ledger and completion criteria

Implementation, unit tests, hosted-runner tests and a real desktop are separate
columns of evidence. The owner of each item is the implementer of its PR;
product/security approvals and branch-protection changes remain with the owner.

| Surface / gap | Exact next acceptance | Done / blocked condition |
|---|---|---|
| Clean Windows contributor machine | Run documented setup twice (idempotence), `just run`, named/token agent handshake, `just exe`, MSIX build/sign/install on a disposable VM | Evidence includes OS/tool versions and exact revision; existing development VM is not clean-VM proof |
| Signed Windows release | Obtain explicit owner approval for a non-publishing validation run or real release; run existing signing, final EXE audit and MSIX gates | No fake tag, weakened signing, secret exposure or automatic release publication |
| WebView2 unavailable | Keep empty-runtime-folder simulation; add a genuinely runtime-absent disposable machine when available | Clear failure state + browser fallback; native/browser print layout and temp-file handling documented |
| Preview close/print | §16 plus open OS print dialog with filled forms, close/cancel; no physical print or submission | App stays alive, data and status unchanged |
| Validation banners | 1601C invalid month and representative wide-column generic form; dismiss, change error contents without changing count, expand many errors | One detailed inline error per field; summary matches its column; dismissal and capped scrolling remain correct |
| Tray/second launch/hotkeys | Hide, launch second instance, inspect foreground HWND, restore by tray and hotkey on actual desktop | §5 foreground/flash contract, no exposed helper windows; mailbox silence not mistaken for process death |
| Database recovery | Unreadable/wrong-key scratch DB, failed rename, reopen after restart | D1 behavior; original bytes preserved, persistent notice, permission failure returns error |
| Export/calendar | Export, click Open folder, then `.ics` on a machine with and without a handler | Folder really opens; no false calendar-handoff success (§6/§10) |
| MSIX activation | Windows 11 plus Server 2022 diagnostic comparison; inspect loader and early startup trace | §8 demonstrated startup, or explicit unsupported-platform disposition; activation event alone is not PASS |
| Receipt print | Synthetic stored receipt in a scratch 2551Q fixture; verify receipt content and preview without contacting email/BIR services | Fake fixture tests layout only; live receipt retrieval remains separately unverified |
| Windows Hello | Owner-provided supported hardware; test authenticate and Cancel→PIN | SKIPPED with reason on hardware without Hello, never mark success from Cancel alone |
| Google Calendar | Owner-approved test account and reversible event; no credentials means skip | Authorization/data-flow evidence, not an inferred integration pass |
| Other desktop platforms | Matching Linux/macOS checks for affected shared code; real macOS preview acceptance on an approved macOS machine | Windows evidence cannot certify WKWebView; record unavailable platform coverage |
| §10 smaller gaps | Hotkey conflict feedback; existing-profile setup should not duplicate TIN; COR Confirm explains unapplied edits; manual_entry asset resolves | Focused regression + corresponding visible action per item; create issue when assigning a slice |

**Closure rule:** do not close a finding until its oracle/regression, scoped
local checks and affected platform acceptance have evidence links. A skip
remains unchecked in #101 with its reason. Completed issues #91–#93/#97 do not
imply signed-release certification or resolve #94/#96. At each merge, update
this ledger, #101's matching work ID and the decisions/changelog; retain older
evidence as historical, not as a claim about a new head.
