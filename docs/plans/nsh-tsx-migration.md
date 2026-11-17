# NSH (TSX) migration plan

Owner's picks: 2026-11-15. Feature slug `nsh-tsx`, branch `feat/nsh-tsx`, worktree `/Volumes/goldcoders/features/nsh-tsx/buwiz-forms`.
This file is the contract for every agent on the job. If it and a brief disagree, stop and report the conflict.

## Goal

Replace the gpui-shell UI (`app/` JavaScript hosted by `crates/bir-shell`) with TSX screens on Native Shell (NSH), mixed mode: TSX screens over the same Rust brain. Tonight's stop line is **all of desktop on macOS**. Every screen is built wide and narrow now, so iPhone and Android need no second rewrite. Phone builds are a later round.

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
| App identity | Name `eBIRForms`, id `dev.goldcoders.bir.nsh`, its own data folder. Runs side by side with 0.2.0. |
| Stop line | Desktop parity on macOS, all 43 forms. |
| Phone-ready | Every screen has a wide and a narrow layout and a test at 390x844. |
| Extras in this round | Print preview and PDF, app menu and shortcuts, tray and hide-to-tray, global hotkey, import/export, jobs and logs, Gmail/inbox connect, help-desk mode, agent remote control, auto-update (code only; signing is parked), Windows and Linux (code paths and CI files; real builds are parked if no machine). |
| Checks | Type check + build + headless tests; plan review; real-window screenshots light and dark; a verifier that uses each screen; a reviewer per wave; side-by-side shots with the old app. |

### UI decisions

Rebuild every screen to look and behave like today's app (source of truth: `app/main.js`, `app/shell/*.js`), except:

- **Look:** today's tokens (`app/shell/ui.js` PALETTES, TYPE, SPACE, RADIUS, SIZE, BREAK) with accent blue, compact 32 px controls, today's radii. Typeface **Inter**, bundled and registered. Fix the faint label color so small text meets 4.5:1. TINs and amounts use tabular figures.
- **Frame:** three columns like today (sidebar 248, main, profile/notifications panel 320 from the context pill). Sidebar: Home, Form library, Calendar, Filings, then Recent returns. Settings stays in the account menu and Cmd/Ctrl+comma.
- **Home:** dashboard like today.
- **Form page:** keep parts, the side list, and the pill that walks what needs you. Add: autosave (no Save draft button; "Saving…" then "✓ Saved"), TIN and money masks while typing, amounts right-aligned in tabular figures, part titles that stick while scrolling, errors shown only after leaving a box, schedule rows that open in a drawer (a sheet when narrow) instead of a sideways table. Do **not** hide parts that do not apply.
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
crates/bir-shell/       Old host. Keeps only gpui glue; adapts bir-app-api's modules to gpui_shell::HostModule.
crates/bir-core/        Gains cargo feature `sqlcipher` (default on). Off = plain SQLite: no key lookup, data folder for
                        `dev.goldcoders.bir.nsh`, a wrong-format file is an error and is never renamed.
                        bir-print and bir-app-api take bir-core with default-features = false and pass `sqlcipher` through;
                        bir-shell turns it on; `nsh/` leaves it off.
nsh/                    The new app. Its own cargo workspace (like crates/bir-shell).
  nsh.json  App.tsx  tsconfig.json  Cargo.toml  AGENTS.md  justfile
  src/                  Rust: #[native_module] door over bir-app-api, plus desktop glue we own (print, save dialog, auth).
  bir/                  Typed TS wrapper over the door. Screens import from here, never call the raw module.
  theme/                Tokens (shadcn cssVars file, light and dark), Inter font files.
  ui/                   The UI kit: every nsh/base control wrapped once and styled from tokens.
  shell/                Frame: title bar, sidebar, right panel, palette, toasts, overlays, tab bar when narrow.
  screens/<name>/       One folder per screen, with its *.test.tsx.
  lib/                  Pure helpers ported from app/shell (todo, periods, recent, facts, settings_model, dates, money).
  assets/icons/         The 55 Lucide SVGs from app/icons.
```

## The door

- One native module (name fixed by the base wave in `nsh/AGENTS.md`) exports `call(name: string, args: JsonValue[]): JsonValue` for sync host functions and `callAsync(name, args): Promise<JsonValue>` for async ones. It dispatches to bir-app-api's `HostModule`. A host error becomes a thrown JS error carrying the host message.
- `nsh/bir/` gives every used function a typed wrapper. Types come from the TypeScript declaration blocks already in the Rust modules. Skip the 19 functions the JS never calls (listed in the JS inventory).
- Sync calls run on the engine thread, as they run on the main thread today; the DB slot stays thread-local. Do not move DB work to other threads in this round.
- Refresh stays poll-based like today (30 s events, 5 s device filings, 1 s update status) behind one small `lib/poll.ts`; a push channel is a later improvement.
- **Send guard.** Before the first DB open and before the submission worker starts, the door sets `BIR_SFTP_DRY_RUN=1`, removes `BIR_SFTP_LIVE`, then requires `bir_core::transport::is_sftp_dry_run()` to be true and the Dev Mode panel's submission mode not to be the test server. If either fails, the worker does not start. The only exception is mock mode: `BIR_SFTP_HOST` is `127.0.0.1` or `::1` and its port is listening (same check as the `mock-bir-app` just recipe).
- **Database guard.** The door refuses to open a database whose canonical path is inside `~/Library/Group Containers/group.dev.goldcoders.bir`, `~/BuwizNsh/data` or `~/BuwizNsh/migration`.
- **Keychain off-switch.** `BUWIZ_NO_KEYCHAIN=1` makes every keychain read in bir-core (cloud account, Google Calendar, dev SFTP password, DB key) return "not found" without calling the OS. Dev and agent runs always set it.
- **Dev run:** `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1 BIR_DATABASE_PATH=~/BuwizNsh/agents/<track>.db`. **Mock run:** unset `EBIR_TEST_ENV` and `BIR_SFTP_DRY_RUN`, then source `target/mock-bir/app.env` (`docs/mock-bir.md`).
- Export and import cannot use `sqlcipher_export` in the plain build; they need a plain path (for example `VACUUM INTO`).

## Waves

**Parallel work in one worktree.** Agents share this worktree and one cargo build cache, so builds queue on a lock: that is normal, wait for it. A compile error in a file you do not own is probably another agent's edit in flight: wait a few minutes and retry before reporting it. Ownership in wave 1a: A3 alone edits root `Cargo.toml` (except the `members` line), `crates/bir-core/**` and `crates/bir-print/Cargo.toml`. A1 owns `crates/bir-host-shim/**`, `crates/bir-app-api/**`, `crates/bir-shell/**` with its `Cargo.lock`, the root `members` line and the root `Cargo.lock`. A2 owns `nsh/**` and `justfile`, and builds with `CARGO_TARGET_DIR=~/BuwizNsh/target-nsh`.

**Fast gate (each agent, for the old shell):** `cargo check --locked --manifest-path crates/bir-shell/Cargo.toml --all-targets`, then `EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1 cargo test --locked --manifest-path crates/bir-shell/Cargo.toml --lib`. The full `just shell-check` runs once per wave.

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
