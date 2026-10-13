# Windows hardening plan: fixing #91 to #100 to the impeccable-rust bar

Tracking issue: #101. Tick items there and here in the same PR.

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
2. **Order inside each PR:** a regression test or gate that fails on `main` comes first, then the fix. The PR shows both runs: red before the fix, green after.
3. **Windows acceptance:** each PR re-runs the #90 steps it touches on a real Windows VM:
   - dry run only, `BIR_SFTP_*` unset;
   - test TIN 123-456-788-00000 or 00002;
   - results go in a PR comment with evidence.
4. **Verification impact block** in every PR description (template in §11).
5. **Repo hygiene:**
   - no generated artifacts, logs or DBs committed;
   - no session or org IDs in the repo;
   - dev runbooks go in `CONTRIBUTING.md` (new), not the README.

---

## 1. Guardrail first: a Windows runtime gate (owner for #96, #97, #98 and later regressions)

**Why:**
- CI already runs `cargo test`, clippy and fmt on `windows-latest`, and all of it was green while the app crashed in normal use.
- None of the Windows failures in #90 is a unit-test failure. They are runtime failures: nested message loops, stack depth, foreground rules and packaging.
- They need an owner that actually *runs the app*.

- [ ] **1.1 `windows-smoke` job** in `ci.yml` (windows-latest), driven by the agent control plane we already ship. In #90 step 29 it was verified working on Windows: `hello` and `snapshot` both work.
  - Build `bir-desktop --features dev-tools,agent`. Launch with:
    - `GPUI_AGENT=1`, `GPUI_AGENT_INSECURE_NO_TOKEN=1` (loopback only);
    - `BIR_DATABASE_PATH=$RUNNER_TEMP\smoke.db`;
    - `BIR_SFTP_DRY_RUN=1`.
  - Script (`scripts/windows-smoke.ps1`, also runnable locally):
    1. `hello`, then create profile 123-456-788-00000 and add a manual COR with 1601C plus one 2200 form.
    2. Open 1601C and use **Print Preview**. Assert the preview window exists, `print-preview-scroll` is present, and the app process is still alive.
    3. Close the preview, then open 2200M preview: same asserts.
    4. Hide to tray, then show: assert the process is alive and no panic is in stderr.
    5. Validate and Submit 1601C in dry run: assert status `Submitted` and the log line `SFTP dry-run PUT (no network)`.
  - Fail the job on any `panicked at`, `overflowed its stack`, or non-zero exit in the captured stderr.
  - Runs the **debug** build (where #96 bites) and the **release** build (what users get).
  - **Rollout:** lands first with `continue-on-error: true` (report-only, expected red on `main` because of #97). Flip it to required when §2 and §3 merge.
- [ ] **1.2 PE header check:** a CI step reads the `bir.exe` stack reserve (`dumpbin /headers`, or a tiny Rust test using the `object` crate). Fail if it's below the value chosen in §3.
- [ ] **1.3** Add `zizmor --persona=auditor` on `.github/workflows/**`. The repo has three workflows, and smoke jobs add secrets surface. This is the skill's CI-shape item 8.

**Oracle:** the user-visible behaviour in #90 (process alive, window present). The agent snapshot is the observation channel.
**Blind spots, written down:**
- GitHub runners have no GPU (WARP renderer).
- Foreground-window rules on a service desktop differ from a real session.
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
- [~] **2.1 Two-phase construction.**
  - `FrozenHtmlPreviewView` gets an explicit state: `enum PreviewState { Pending { html }, Ready(Entity<WebView>), Failed { error, html } }`.
  - `new()` only stores `Pending` and schedules creation with `cx.spawn_in(window, ...)`, yielding with a zero-duration background timer after `open_window` returns and the borrow is released.
  - The deferred step builds the WRY child and moves to `Ready` or `Failed`.
  - `render()` matches on the state, so "rendering a webview that doesn't exist yet" can't be written.
- [~] **2.2 Same pattern for every WebView2 operation that can pump messages** (`print()`, `evaluate_script` used by `request_js_metrics`): WebView-dependent methods now operate only on `PreviewState::Ready`; construction is moved outside the GPUI borrow.
- [~] **2.3 #90 step 17 polish:** map WebView2 errors to a retained failed-preview state and provide an **Open in browser** fallback that reuses `write_frozen_html_document`. Toolbar operation errors remain separate from construction errors.
- [~] **Tests:**
  - Unit: the state machine (`Pending→Ready`, `Pending→Failed`, no transition out of `Failed`).
  - Gate: §1.1 steps 2 and 3, with readiness exposed to the agent.
  - Manual: #90 step 16 on a VM (window about 1200×900; Print opens the WebView2 dialog with A4 and no scaled margins), plus step 17 by setting `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` to an empty folder.
- **Also verify** that macOS and Linux print preview are unchanged: run the existing `drain.rs` print-preview metrics tests, plus a manual macOS check by the owner.

**Changelog**

| Date | Result |
|---|---|
| 2026-10-12 | Implemented the Windows two-phase WebView construction and explicit pending/ready/failed state. The first main-branch smoke results were release: both previews panic with `RefCell already borrowed` (#97); debug: 1601C panics and 2551Q overflows its stack (`0xC00000FD`, #96); submit passes in both; `stack_reserve` is 1 MiB. |
| 2026-10-12 | Confirmed the second #97 rendering cause: DirectComposition hid the visible, correctly sized WebView2 child HWND. With `GPUI_DISABLE_DIRECT_COMPOSITION=1`, filled 1601C and 2551Q previews rendered; the main window continued to paint normally. |

---

## 3. #96: debug build overflows the 1 MiB main-thread stack (P0 for dev, unknown for release)

**Facts:**
- MSVC's default stack reserve is 1 MiB; macOS and Linux main threads get 8 MiB.
- `[profile.dev]` leaves workspace crates at opt-level 0 (only deps get 1), so GPUI render frames are large.
- An 8 MiB `editbin` copy runs cleanly.
- The release build (`opt-level="z"`, `panic="abort"`) didn't crash in a short session. That is not proof.

- [ ] **3.1 Measure before choosing a number.**
  - Bisect `editbin /STACK:` over 1, 2, 4 and 8 MiB with the #90 repro (tray Show, then click a profile; open a form; open the COR editor) on debug and release builds.
  - Record the smallest passing size for each, here.
- [ ] **3.2 Fix at the binary, not RUSTFLAGS.**
  - `ci.yml` relies on "no RUSTFLAGS anywhere" for cache sharing, so don't add `.cargo/config.toml` rustflags.
  - Instead, `crates/bir-desktop/build.rs` emits `cargo:rustc-link-arg-bin=bir=/STACK:<chosen>` only when `CARGO_CFG_TARGET_ENV == "msvc"`.
  - Default choice is 8 MiB, matching the other OSes, unless 3.1 says otherwise.
- [ ] **3.3 Find the deep frames** so the margin is known, not just moved:
  - capture the overflow backtrace under a debugger with a 1 MiB stack;
  - write down the top recursive or large-frame functions;
  - open follow-ups for any surprisingly large frame (big arrays on the stack in render paths).
- **Owners:** §1.2 (PE header) and §1.1 (debug and release runs).

---

## 4. #95: unreadable database is moved aside silently (P1, data trust)

**Facts:**
- `Database::open_or_recreate` quarantines to `bir_data.db.corrupt-<ts>.bak` and creates a fresh DB.
- `open_live_database()` returns `(Database, PathBuf, Option<PathBuf>)`.
- `gui.rs:288` only `eprintln!`s the `Option`.
- A DB from another machine is always unreadable, because the SQLCipher key lives in that machine's keychain.

- [ ] **4.1 Make ignoring it hard (type-enforced).**
  - Replace the tuple with `#[must_use] pub struct LiveDatabase { db, path, recovery: Option<Recovery> }`.
  - `Recovery { backup_path, reason: RecoveryReason }`, with `RecoveryReason` one of `WrongKeyOrForeign | NotADatabase | Truncated`. If SQLCipher's errors can't tell these apart, use one variant and write that down.
- [ ] **4.2 Make it visible and persistent.**
  - Write a high-severity alert row (`db/alerts.rs`) into the new DB, so the bell shows it after restarts.
  - Also show a first-frame banner: "We couldn't open your previous data (it may come from another computer or a different key). Nothing was deleted: it was moved to <path>." with **Open folder** and **How to move data between computers** (Export/Import).
- [ ] **4.3** Document in `docs/platforms/*.md` that a raw DB copy is not portable; use Export All Profiles and Import.
- **Decision needed:** keep auto-quarantine plus a loud notice (proposed), or refuse to start until the user chooses?
- **Tests (bir-core, every OS):**
  - **property-tested:** any random-byte file opens with `recovery: Some`, the backup's SHA-256 matches the input, and the fresh DB opens;
  - a DB under key A opened with key B gives `WrongKeyOrForeign`;
  - retention pruning keeps exactly N backups;
  - error-path litmus: a failed rename (read-only dir) must be `Err`, never a silent fresh DB.
  - Desktop: the alert row exists after a recovered open. #90 step 7 is the manual acceptance.

---

## 5. #98: second launch while hidden doesn't bring the window to the front (P2)

**Facts:**
- The second instance already calls `AllowSetForegroundWindow(ASFW_ANY)` (`ipc.rs:38`) before sending `SHOW`.
- The receiver's `show_in_dock()` runs `EnumWindows` over **every** window of the process (`platform/windows.rs` `show_window_callback`), calling `SW_RESTORE`, `SW_SHOW` and `SetForegroundWindow` on each one.
- That includes GPUI and tray helper windows, so the last window enumerated wins the foreground, and hidden helper windows get shown.
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
  pinned-revision update are merged upstream.
- [x] **7.2 `scripts/setup-windows.ps1`:** Root-manifest pin resolution, PATH
  ordering/deduplication, and 3010 handling passed PowerShell parsing and the
  focused setup-policy test.
- [x] **7.3 #92 `justfile`:** Python/ISCC prerequisites, source gating, MSIX
  path handling, and the real Windows packaging build passed locally.
- [ ] **7.4 #93 installer audit:** The shared audit script and parser checks
  are implemented. Local final-Setup execution safely refused because a real
  eBIRForms installation is registered; disposable CI must verify
  install/audit/uninstall. Never remove a pre-existing installation.
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
- [ ] **8.2** Add an early panic hook and startup breadcrumb (`%TEMP%\ebirforms-startup.log`) before anything else. This helps every platform.
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

- [ ] **9.1 Correct the record:** a comment on #99/#89 with the table above, so nobody "fixes" 2 and 3.
- [ ] **9.2 Upload-path oracle without Submit.** In a scratch copy of the HTA, a harness calls `saveEncryptedProfile`'s plaintext builder (or `saveXML(true)` with the final-copy writes redirected), and captures `allXML` before encryption and before any SFTP code.
  - It must never touch Submit, Final Copy, the network or `RenameAndSendFile`.
  - **The owner approves the harness before it runs**, since it goes beyond Validate and Save.
- [ ] **9.3 Real-capture corpus:**
  - for N representative forms (start with 1601C, 2551Q, 1702-RT, 1702Q, 0619E), capture real-IE plaintext on Windows with the #89 driver: synthetic TIN, Validate plus Save, plus the 9.2 harness;
  - store it as `*.ie.xml` fixtures with provenance (package version and sha, Windows build, recipe hash).
- [ ] **9.4 Fix the emulator (`tools/official-xml`):**
  - `setValue` refuses `disabled`/`readOnly` controls and reports the step as invalid;
  - fire IE's event order for typed input (keydown/keypress/keyup, then change, then blur; verify against the HTA handlers);
  - normalize the `xmlFormat` separator to CRLF the way IE does;
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

**Current verifiers:** `cargo test`, clippy and fmt on macOS, Linux and Windows; the sample-payload byte tests (78); frozen-HTML inventory checks; proptest-style crates in `bir-core`. Missing: `cargo deny`, nextest flaky gating, zizmor, and **any runtime check of the desktop app**.

| Failure class (this plan) | Owner | Verdict |
|---|---|---|
| GPUI vs Win32 re-entrancy (#97) | §1.1 smoke gate plus the §2 state-machine unit tests | REQUIRED |
| Stack exhaustion (#96) | §1.2 PE check plus the §1.1 debug and release runs | REQUIRED |
| Data-loss perception on recovery (#95) | bir-core property and error-path tests (§4) | REQUIRED |
| Wrong-window Win32 calls (#98) | trait-injected unit tests plus manual VM steps | REQUIRED |
| Dialog semantics differ by OS (#100) | pure mapping unit test, or the in-app dialog | REQUIRED |
| Official-XML parity (#99) | §9.5 differential conformance against real-IE captures | REQUIRED |
| Contributor setup drift (#91–#93) | clean-VM acceptance per PR; nightly `setup-windows` job | USEFUL |
| Vulnerable or unvetted deps | `cargo deny` (RUSTSEC) in CI | USEFUL |
| Compromised workflows | `zizmor --persona=auditor` | USEFUL |
| Flaky tests | nextest with `flaky-result = "fail"` | OPTIONAL |
| Win32 `unsafe` (~93 sites) | review, safe wrappers, smoke gate. Miri can't run FFI | Miri: NOT JUSTIFIED |
| Loom, TLA+, Kani, provers | no concurrency kernel or protocol in scope | NOT JUSTIFIED |

**Anti-drift:** add `CONTRIBUTING.md` holding the skill's anti-drift block, the Verification impact template and the Windows dev runbook from §7. The emulator (jsdom) is a second model of the official HTA, so it counts as evidence only through the §9.5 conformance link.

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
| 1 | `win-tooling` | §7 (#91 upstream first, then #92, #93) | none | clean-VM setup run |
| 2 | `win-smoke-gate` | §1.1–1.3 in report-only mode | 1 (CLI) | job runs, red on `main` as expected |
| 3 | `win-print-preview` | §2 | 2 | smoke steps 2–3 green |
| 4 | `win-stack` | §3 | 2 | PE check plus smoke on debug |
| 5 | flip §1.1 to required | none | 3, 4 | none |
| 6 | `db-recovery-notice` | §4 | owner decision | property tests plus #90 step 7 |
| 7 | `win-window-focus` | §5 and §6 | none | unit tests plus #90 steps 8, 9, 13, 18 |
| 8 | `official-xml-ie-parity` | §9 (separate track, needs Windows eBIRForms sessions) | owner approval of the 9.2 harness | §9.5 conformance |
| 9 | `msix-start` | §8 | 8.1 diagnosis | #90 step 4 |
| 10 | backlog | §10 | none | per item |

Each PR ends with the #90 steps it touches re-run on Windows. After PR 5, a full #90 pass gives the new Windows baseline.

---

## 13. Decisions

| Date | Decision | Status |
|---|---|---|
| | #95: auto-quarantine plus loud notice, or refuse to start? | **open (owner)** |
| | #99: approve the upload-path plaintext harness (9.2) that goes beyond Validate and Save? | **open (owner)** |
| | #99: does BIR's upload carry CRLF (parity target)? | open: settled by 9.2 |
| | #96: stack reserve size | open: settled by 3.1 |
| | #97: GPUI primitive for deferred WebView creation | open: settled in PR 3 |

## 14. Changelog
- Plan created from the #90/#89 Windows verification. Corrected the #99 assessment: diffs 2 and 3 come from different official save paths.
- 2026-10-13 | Replaced 7-Zip payload inspection with strict source validation and actual installed-Setup auditing; implementation is distinct from pending clean-VM and signed-release acceptance.
