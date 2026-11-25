> **Historical record:** this file describes a past state of the project, kept for reference.

# NSH (TSX) migration plan

Owner's picks: 2026-11-15. Feature slug `nsh-tsx`, branch `feat/nsh-tsx`, worktree `/Volumes/goldcoders/features/nsh-tsx/buwiz-forms`.
This file is the contract for every agent on the job. If it and a brief disagree, stop and report the conflict.

## Goal

Replace the gpui-shell UI (`app/` JavaScript hosted by the retired gpui shell crate) with TSX screens on Native Shell (NSH), mixed mode: TSX screens over the same Rust brain. Tonight's stop line is **all of desktop on macOS**. Every screen is built wide and narrow now, so iPhone and Android need no second rewrite. Phone builds are a later round.

## Hard rules (never break; if one blocks you, park the item and report)

1. Never send anything to BIR. The NSH app runs dry-run only; tests use the mock BIR server (`docs/mock-bir.md`).
2. Never open, copy, rename or modify the owner's original database (`~/Library/Group Containers/group.dev.goldcoders.bir/`) or `~/BuwizNsh/migration/`. Never read the macOS keychain.
3. `~/BuwizNsh/data/bir_data.db` is the owner's migrated plain copy. Do not run the app or tests against it. To use real data, first `cp` it to `~/BuwizNsh/agents/<your-track>.db` and point `BIR_DATABASE_PATH` there.
4. Do not change `app/` (the old screens) or anything under `/Applications`. The old app must keep building and passing its tests after every wave: `just shell-check`.
5. No signing, notarizing, store uploads, paid services, or new accounts.
6. No git writes: no add, commit, stash, checkout, reset, push. The orchestrator commits. Stay inside the paths your brief gives you.
7. Never weaken, skip or delete an existing test to make something pass. Never invent an NSH API: look it up in the NSH source, `.nsh/types`, or the NSH skill pages, and cite the file.
8. When stuck for more than about 20 minutes on one obstacle, park it: write what you tried and what is missing in your final report, and finish the rest.
9. Personal data stays out of the repo: tests and screenshots committed to the repo use only the demo taxpayer (Juana Santos, TIN 000-123-453-00000) or generated data. Screenshots of real data go under `~/BuwizNsh/evidence/`, never in the worktree.

## Decisions (from the picks)

| Area | Decision |
|---|---|
| Home of the code | New `nsh/` folder in buwiz-forms, beside the old app. |
| NSH version | NSH 0.5.1: the hexuria/nsh fork (`buwiz` line) with upstream `v0.5.1` merged and two Windows fixes, commit `87e0a4ea4`, the tip of `buwiz` since hexuria/nsh#13 (the checkout is the sibling `../nsh`). Read-only for agents: do not patch NSH to fit the app. |
| Door to Rust | The same host functions the JS calls today, moved into a UI-free crate `crates/bir-app-api`, exposed to TSX through one NSH native module. Existing tests move with the code, unedited. |
| Data | Plain SQLite copy of the owner's data (done). Scrubbed of PINs, mail credentials, send grants. |
| Encryption | Gone for the NSH app: plain SQLite, no DB key, no keychain read for it. The old shell keeps SQLCipher through a cargo feature until it is deleted. |
| Sign-in and locks | Build all of them (Buwiz account gate, PIN, authenticator, recovery key, Touch ID recovery, first-run setup). Dev builds skip them. |
| App identity | Name `eBIRForms`, id `dev.goldcoders.bir`, its own data folder. Replaces the old app without migrating its data. |
| Stop line | Desktop parity on macOS, all 43 forms. |
| Phone-ready | Every screen has a wide and a narrow layout and a test at 390x844. |
| Extras in this round | Print preview and PDF, app menu and shortcuts, tray and hide-to-tray, global hotkey, import/export, jobs and logs, Gmail/inbox connect, help-desk mode, agent remote control, auto-update (code only; signing is parked), Windows and Linux (code paths and CI files; real builds are parked if no machine). |
| Checks | Type check + build + headless tests; plan review; real-window screenshots light and dark; a verifier that uses each screen; a reviewer per wave; side-by-side shots with the old app. |

### UI decisions

Rebuild every screen to look and behave like today's app (source of truth: `app/main.js`, the old shell's `*.js`), except:

- **Look:** today's tokens (the old shell's `ui.js` PALETTES, TYPE, SPACE, RADIUS, SIZE, BREAK) with accent blue, compact 32 px controls, today's radii. Typeface **Inter**, bundled and registered. Fix the faint label color so small text meets 4.5:1. TINs and amounts use tabular figures.
- **Frame:** three columns like today (sidebar 248, main, profile/notifications panel 320 from the context pill). Sidebar: Home, Form library, Calendar, Filings, then Recent returns. Settings stays in the account menu and Cmd/Ctrl+comma.
- **Home:** dashboard like today.
- **Form page:** keep parts, the side list, and the pill that walks what needs you. Add: autosave (no Save draft button; "Saving…" then "✓ Saved"), TIN and money masks while typing, amounts right-aligned in tabular figures, part titles that stick while scrolling, errors shown only after leaving a box, schedule rows that open in a drawer (a sheet when narrow) instead of a sideways table. Do **not** hide parts that do not apply.
  Later-quarter 2551Q opening advice comes from the host's Item 13 applicability and annual-election validation,
  not a second TS predicate. Open/fill/use-profile return the missing profile context; links select the form's year,
  open the annual-election editor or focus the business start date, and return after a successful save. Reopening
  refreshes editable 2551Q profile snapshots from that return's year before deriving advice; in-form profile
  saves replace the advice from host results. Linked editors use native ref reveal before focus, including
  the selected (or first unset) annual-election choice. Queued/filed snapshots are not refreshed by this path.
  This adds no Queue requirement: a missing commencement date remains advisory.
  Verification: `session.prerequisites`, `FormPagePrerequisites`, `ui/focus`, taxpayer
  `yearEdit`, and the screen-form `fact_asks` Rust tests; native focus/scroll remains a device acceptance check.
- **Print preview:** a second window on desktop, like today.
- **Filings:** the board when wide (720 px and up), a list with status tabs when narrow.
- **Calendar:** today's filter bar with List/Month when wide; month grid with due dots plus an upcoming list when narrow.
- **Settings:** a normal page in the main area (sections on the left when wide; list then detail when narrow), not a whole-window takeover.
- **Narrow (under 600 px):** four bottom tabs (Home, Forms, Filings, Calendar); the avatar opens a sheet with taxpayers and Settings; long forms are one scroll with sticky part titles and one floating next-step bar. Phone sending will go through the Buwiz relay (later round; not built tonight).

## Target layout

```
crates/bir-host-shim/   UI-free stand-in for the gpui_shell host types (HostValue, HostArguments, HostError, HostResult,
                        HostModule with function/async_function/declarations/call/begin/has/is_async). bir-app-api imports it
                        under the name `gpui_shell` so the moved tests compile unedited.
crates/bir-app-api/     UI-free. A `Hooks` trait for what needs a UI (menu refresh, chrome, updater, hotkey, print preview,
                        agent queue, file pickers, OS sign-in), implemented once by bir-shell and once by the NSH door;
                        the moved modules (bir, tax_profile, dashboard, screen_form + its tree, cloud_account,
                        account_gate, auth_limiter, logging) and their tests. Depends on bir-core, bir-print, bir-rules only.
(retired gpui shell)    Old host, since deleted. Kept only gpui glue; adapts bir-app-api's modules to gpui_shell::HostModule.
crates/bir-core/        Gains cargo feature `sqlcipher` (default on). Off = plain SQLite: no key lookup, data folder for
                        `dev.goldcoders.bir`, a wrong-format file is an error and is never renamed.
                        bir-print and bir-app-api take bir-core with default-features = false and pass `sqlcipher` through;
                        bir-shell turns it on; `nsh/` leaves it off.
nsh/                    The new app. Its own cargo workspace (like the retired gpui shell crate).
  nsh.json  App.tsx  tsconfig.json  Cargo.toml  AGENTS.md  justfile
  src/                  Rust: #[native_module] door over bir-app-api, plus desktop glue we own (print, save dialog, auth).
  bir/                  Typed TS wrapper over the door. Screens import from here, never call the raw module.
  theme/                Tokens (shadcn cssVars file, light and dark), Inter font files.
  ui/                   The UI kit: every nsh/base control wrapped once and styled from tokens.
  shell/                Frame: title bar, sidebar, right panel, palette, toasts, overlays, tab bar when narrow.
  screens/<name>/       One folder per screen, with its *.test.tsx.
  lib/                  Pure helpers ported from the old shell (todo, periods, recent, facts, settings_model, dates, money).
  assets/icons/         The 55 Lucide SVGs from app/icons.
```

## The door

- One native module (name fixed by the base wave in `nsh/AGENTS.md`) exports `call(name: string, args: JsonValue[]): JsonValue` for sync host functions and `callAsync(name, args): Promise<JsonValue>` for async ones. It dispatches to bir-app-api's `HostModule`. A host error becomes a thrown JS error carrying the host message.
- `nsh/bir/` gives every used function a typed wrapper. Types come from the TypeScript declaration blocks already in the Rust modules. Skip the 19 functions the JS never calls (listed in the JS inventory).
- Sync calls run on the engine thread, as they run on the main thread today; the DB slot stays thread-local. Do not move DB work to other threads in this round.
- Refresh stays poll-based like today (30 s events, 5 s device filings, 1 s update status) behind one small `lib/poll.ts`; a push channel is a later improvement.
- **Send guard.** Normal desktop releases use bir-core's filing transport, which files live by default and honors explicit dry-run settings. Before the first DB open and worker start in a debug, test, agent or mobile launch, the door removes `BIR_SFTP_LIVE` and forces `BIR_SFTP_DRY_RUN=1` unless a local mock is selected. The worker is refused if the Dev Mode panel points at the test server or sending is neither dry nor directed at a guarded local mock (a listening loopback SFTP endpoint or a health-checked loopback relay).
- **Database guard.** The door refuses to open a database whose canonical path is inside `~/Library/Group Containers/group.dev.goldcoders.bir`, `~/BuwizNsh/data` or `~/BuwizNsh/migration`.
- **Keychain off-switch.** `BUWIZ_NO_KEYCHAIN=1` makes every keychain read in bir-core (cloud account, Google Calendar, dev SFTP password, DB key) return "not found" without calling the OS. Dev and agent runs always set it.
- **Dev run:** `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1 BIR_DATABASE_PATH=~/BuwizNsh/agents/<track>.db`. **Mock run:** unset `EBIR_TEST_ENV` and `BIR_SFTP_DRY_RUN`, then source `target/mock-bir/app.env` (`docs/mock-bir.md`).
- Export and import cannot use `sqlcipher_export` in the plain build; they need a plain path (for example `VACUUM INTO`).

## Android print session lifecycle

This is the Android print ownership/cancellation model for the anti-drift rule in
`CONTRIBUTING.md`. The desktop preview contract is unchanged. Implementation:
`nsh/screens/print/controller.ts`, `nsh/src/print.rs`, `nsh/src/print_android.rs`.

- **Open/loading:** the controller retains the Android key and its session/request
  identity even after the native open promise settles. Rust checks the exact
  request and captures its revocable token and the Android generation before
  reading the private snapshot. A single native slot owns the WebView, Activity and,
  after `PrintManager.print`, the returned PrintJob. Loading polls WebView progress;
  generation and request-liveness checks run on the UI thread, including
  immediately before `print`.
- **Revoke:** lock/sign-out reaches `closeAll` through the session-change hook
  (hidden chrome also closes previews). `closeAll` advances the controller's
  generation and closes known owners by their exact request identity. It also
  calls `Print.reset`, revoking every Rust preview, including requests whose open
  events have not arrived. `Print.current` rejects those stale events; late native
  callbacks cannot close a replacement request with the same form code.
  Synchronous revocation increments the Android generation, takes the native
  slot and forgets the snapshots before returning;
  WebView/Activity work is posted asynchronously to the Android UI thread. A queued
  open cannot revive a forgotten snapshot. The slot mutex covers the final print
  call and job retention, so cleanup owns a job that already started opening.
- **Dispose:** both replacement by another preview and revocation stop loading,
  destroy the old WebView and release its retained references. Only revocation
  checks the PrintJob state and, for `STATE_CREATED` (unsubmitted system preview),
  brings the existing app Activity forward with `CLEAR_TOP | SINGLE_TOP` to dismiss
  the sheet. No job or an already dismissed sheet requires no navigation.
- **Submitted:** queued, started, blocked, failed, completed and cancelled jobs
  belong to the system spooler, not the app's preview session. Neither replacement
  nor lock/sign-out calls `PrintJob.cancel()` or dismisses these jobs. In particular,
  printing a second return must not cancel the first return's submitted print job.

**Verification boundary:** host Rust tests in `nsh/src/print_android_policy.rs`
cover dismissal versus leaving a job alone for replacement/revocation across all
job states, including missing/unknown states. Cancellation is not a policy action.
The Android snapshot test awaits cleanup after asserting synchronous revocation
and snapshot deletion. Controller tests cover closed/hidden requests and
revocation before and after native opening settles, queued open events after a
session reset, and late callbacks for replaced same-form requests; Android-target compilation
checks JNI calls. These do not prove system spooler or Activity behavior. Device
acceptance must cover lock during loading (no late sheet), lock while previewing
(sheet closes), and submitting a job followed by either a new preview or lock
(submitted job continues). No separate formal Android print model exists here;
these transitions document this boundary without duplicating the native state
machine in an unrelated model.

## Android development command mailbox

For emulator access-revocation checks, `Door.status().devCommandEnabled` is true
only on Android with the shared `dev_env::ENABLED` compile-time gate (debug
assertions or `BUWIZ_ANDROID_DEV=1`) and the exact in-process configuration value
`EBIR_TEST_ENV=1`. Desktop and non-opted-in release builds return false and never
consume a mailbox. This is development control, not a production IPC surface.

Shell owns one 500 ms poll, separate from ordinary polls and independent of
focus, lifecycle, visible chrome and sign-in state. Unmount stops the poll.
`Door.devCommand()` rechecks the gate on every read, reads at most 65 bytes from
`<engine data_dir>/dev-command` and removes it before returning an allowed
command. A failed read/removal does not dispatch. Unknown, oversized and invalid
UTF-8 commands are consumed and rejected; tracing records the receipt time in
Unix milliseconds and warns on rejection. Producers should atomically rename
a completed file into place rather than race a partially written command.

The shell dispatches only `lock` to `app.lockApp()` and `logout` to
`app.logOut()`: the same handlers as Ctrl+L and local Settings Sign out. The
mailbox adds no Activity/focus operations. Existing print revocation/disposal
above is unchanged, including its Activity navigation to dismiss an unsubmitted
system preview; the mailbox itself does not first bring the app forward.

Rust `dev_command` tests cover the complete gate matrix, parsing, private-root
consumption, no replay, disabled reads, invalid commands and I/O failure.
`shell/devCommand.test.ts` covers disabled scheduling, the 500 ms task and real
session-handler effects, including sign-out after lock. These do not prove
Android background timer execution or system-preview dismissal on a device.

## Android agent token hand-off

This is the ownership/cancellation model of the development launcher's token
writer, for the anti-drift rule in `CONTRIBUTING.md`. Implementation: the
`nsh-run-agent` recipes in the `justfile`, `nsh/agent/android-handoff.sh` (Unix)
and `take_token`/`LaunchToken` in `nsh/src/agent.rs`. It exists only in agent
builds (cargo feature `agent`) started by that recipe.

- **Owner:** the launcher (the recipe's shell, which becomes `nsh run` on Unix)
  owns the writer. The writer is started only after the SDK's `adb` and the one
  device nsh launches on are resolved, and names that serial in every `adb` call.
  A launch refused there (no SDK, no `adb`, no online device, a different
  `ANDROID_SERIAL`) starts no writer and contacts no device.
- **Running:** the writer polls `run-as` until the host is installed, then writes
  the token once over standard input to `agent-token.new` and renames it to
  `agent-token`. It then ends. It is bounded (1800 tries).
- **Launch failure or cancellation:** before each poll the writer checks that its
  launcher is still the same live process (Unix: the PID; Windows: the PID and its
  start time, so a reused PID does not count) and ends without writing when it is
  not. Windows also stops the job in the recipe's `finally`, which runs for a
  failed build, a failed install and Ctrl+C, and removes `agent-token` and
  `agent-token.new` from the host's files. On Unix a token already written when
  the launch fails stays in the host's private files until the next start of the
  app reads and deletes it; it is the token of that developer's own launch.
- **Reader:** the app reads the file once and deletes it (`take_token`); the
  launch keeps the token in memory for a replacement listener after a full
  reload (`LaunchToken`). A file that arrives late is waited for (10 s).

**Verification boundary:** `nsh/agent/android-handoff.test.sh` (fake `adb`,
`just nsh-agent-handoff-test`) covers the Unix writer: the serial in every call, the
token in no argument, a launcher gone before the first poll, a launcher that
dies while the writer waits, and the bounded wait.
`scripts/tests/test_nsh_run_agent_windows.py` runs the Windows recipe against a
native `adb.exe` and a fake `nsh`. `test_a_failed_launch_stops_the_token_writer`
and `test_a_launch_that_fails_after_the_token_was_stored_removes_it` cover a
launch that returns an error. `test_a_cancelled_launch_stops_the_token_writer`
ends the recipe's PowerShell with `taskkill` while the writer polls: no `finally`
runs, and the writer ends by its own launcher check, before any token is stored.
`test_ctrl_c_after_the_token_was_stored_removes_it_and_stops_the_writer` covers
Ctrl+C: the recipe runs in a console of its own, the fake `nsh` holds the stored
token unread, a console Ctrl+C event (`GenerateConsoleCtrlEvent`) is sent to
that console, and the `finally` must remove `agent-token` and `agent-token.new`
with `-s <serial>` and leave no writer polling. Closing the console window
(`CTRL_CLOSE_EVENT`) and Ctrl+Break are not tested. The file needs Windows, `pwsh`,
`just` and `rustc`, and is skipped elsewhere; the `scripts` CI job runs on Linux,
so it is run by hand on Windows. `nsh/tests/agent.rs` covers the reader (late
arrival, deletion, the kept token). None of these runs a real device: `run-as`,
`adb shell -T` standard input and the app's restart on a device are not proved
here. No separate formal model exists; the two launchers are small enough that
these tests are the model.

## Worker-driven filing refresh

Filings, Home and the form page share the cached `store.returns` lists. Worker
completion posts `bir_core::ipc::post_db_changed`, but the NSH UI does not consume
that signal; invalidation is owned by the shell polls, not by screen navigation.
`startQueuedWatch` runs independently of the ordinary events poll, including in
development builds where that poll is disabled. While unlocked and a cached return
is Queued, it invalidates `WORKER_KEYS` every 3 seconds for the first 30 seconds
after the set/queue action changes, then every 30 seconds until no return is Queued.
A long relay retry must not outlive all cache invalidation. Lock stops reads and
unlock restarts the fast phase. No queued returns means no periodic invalidation
from this watch. `shell/queuedWatch.test.ts` verifies the pacing and a late worker
submission with no ordinary events poll; this is cache/timer verification, not a
device or relay transport test.

## Android profile export

`export_profile` keeps its PIN/rate-limit gate and existing Settings/taxpayer card.
Android's `NshHooks::export_profile_download` writes the authorized ZIP synchronously
on the database-owning engine thread, through `export_profile_to` →
`export_profile_data` → `write_profile_zip`. It never opens the unsupported save
panel. Other platforms retain that panel unchanged.

Staging is a unique 0700 directory beneath `bir_core::platform::temp_dir()` (Android:
the injected app-private data directory's `tmp`), removed on drop, including writer,
JNI and copy failures and dropped futures. A running blocking copy owns the stage
until it completes even if its caller goes away. Process abort/kill can leave a
private stage; this is not a public Downloads copy. Publication uses MediaStore
Downloads on API 29+: `relative_path=Download/eBIRForms`, `is_pending=1`, close the
output stream, then `is_pending=0`. Errors delete the pending row; cleanup failures
are logged. The provider chooses collision suffixes; its queried `_display_name`
is returned as `Downloads/eBIRForms/<name>` and shown in the success toast.

iOS has no save panel either: `nsh/src/export_ios.rs` stages the same private archive, copies it
to `<tmp>/profile-exports/<name>` and presents `UIDocumentPickerViewController` in export mode
(as a copy: Files, iCloud Drive or another provider). The call resolves when the user saves
(`Files: <name>`) or dismisses the sheet (an error, "Export cancelled: nothing was saved.");
the staged copy is deleted after either answer.

NSH's share API exists but Android's outcome only reports selection of a target,
not completion of its file read. It cannot establish a saved Downloads path or a
safe point to delete this private ZIP, so this flow uses MediaStore, not sharing.
No framework or manifest changes, storage permission or Activity result are needed.
The existing print JNI VM/UI/foreground-Activity helpers are reused.

Verification boundary: host-compilable Rust tests cover staging isolation,
writer-failure cleanup and returned-name validation. `taxpayer/export.test.tsx`
mocks the host download result to cover no-dialog UI success, collision copy,
failure and cancellation; it does not exercise MediaStore. Android compilation
and actual device export/opening of the ZIP are separate native acceptance checks.

## Waves

**Parallel work in one worktree.** Agents share this worktree and one cargo build cache, so builds queue on a lock: that is normal, wait for it. A compile error in a file you do not own is probably another agent's edit in flight: wait a few minutes and retry before reporting it. Ownership in wave 1a: A3 alone edits root `Cargo.toml` (except the `members` line), `crates/bir-core/**` and `crates/bir-print/Cargo.toml`. A1 owns `crates/bir-host-shim/**`, `crates/bir-app-api/**`, the retired gpui shell's `**` with its `Cargo.lock`, the root `members` line and the root `Cargo.lock`. A2 owns `nsh/**` and `justfile`, and builds with `CARGO_TARGET_DIR=~/BuwizNsh/target-nsh`.

**Fast gate (each agent, for the old shell):** `cargo check --locked --manifest-path <the retired gpui shell's Cargo.toml> --all-targets`, then `EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1 cargo test --locked --manifest-path <the retired gpui shell's Cargo.toml> --lib`. The full `just shell-check` runs once per wave.

Each wave ends with: old shell still green (`just shell-check`), `nsh check && nsh build && nsh test` green, real-window screenshots (light, dark, wide, narrow), a verifier pass, a reviewer pass, then one commit by the orchestrator.

| Wave | Tracks (model) |
|---|---|
| 1a Base | A1 bir-app-api extraction (Opus) · A2 NSH toolchain, `nsh/` scaffold, theme, `nsh/AGENTS.md`, just recipes (Sonnet) · A3 bir-core `sqlcipher` feature, plain open, wrong-format guard (Sonnet) |
| 1b Base | A4 UI kit over nsh/base (Opus) · A5 pure helpers and icons (Haiku) · A6 the door and `nsh/bir` typed wrapper (Sonnet) |
| 2 One form | B1 form engine: open, fill, check, autosave, pill, to-dos (Opus) · B2 field controls, masks, schedule drawer (Sonnet) · B3 frame: title bar, sidebar, panel, palette, toasts, navigation, narrow tab bar (Sonnet) |
| 3 Everyday | C1 Home (Sonnet) · C2 Form library and New return dialog (Haiku) · C3 Calendar (Sonnet) · C4 Filings (Sonnet) · C5 Taxpayer profile, tax years, facts, setups, templates, new taxpayer/branch (Opus) |
| 4 All desktop | D1 Settings, jobs and logs, import/export (Sonnet) · D2 sign-in, locks, first-run setup, help-desk sessions (Sonnet) · D3 print preview window and PDF (Sonnet) · D4 app menu, tray, hotkey, updater, single-instance (Sonnet) · D5 inbox connect and send grants UI (Sonnet) · D6 agent remote control (Sonnet) · D7 Windows/Linux code paths and CI (Sonnet) · D8 open all 43 forms, screenshot each (Haiku) |

## Definition of done for a screen

1. It reads and writes only through `nsh/bir`; it is built only from `nsh/ui` and `nsh/shell` parts and theme tokens (no raw colors, no restyled base controls).
2. Wide and narrow layouts exist. A `*.test.tsx` covers the main interaction at 390x844 and at a desktop size.
3. `nsh check`, `nsh build` (no warnings left unexplained) and `nsh test` pass.
4. Real-window screenshots exist for light and dark.
5. Behavior matches the old screen for every interaction in the old source, or the difference is one listed under UI decisions.

## Parked for the owner

Signing and notarizing, store ids, real Windows/Linux machines, live sending, phone builds, moving to upstream NSH.
