# gpui-shell migration plan: JavaScript UI on the QuickJS shell

Tracking issue: [#135](https://github.com/hexuria/buwiz-forms/issues/135).
Tick items there and here in the same PR.

## Cutover gate: failed headed parity comparison

**Do not retire the painted application's views or dependencies.** The shell
workspace builds on all three CI platforms, but compilation and generic
field-spec coverage do not establish product parity. The earlier S1–S6
completion claims are superseded by this comparison. `just run` currently
selects the development shell; `just run-painted` remains the reference. This
development default is not approval to call the product cutover complete.

*S7 packaging leg:* release CI now ships the shell as `eBIRForms` — the DMG,
Windows installer/MSIX and Linux packages carry `bir-shell` under the shipped
`bir` name plus the `app/` and `print-overrides/` resources it resolves from
the bundle layout (`crates/bir-shell/src/packaged.rs`). `bir-desktop` stays
buildable for development and is still what `just app`/`testflight` packages
for MAS: the shell lacks a `mas_build` equivalent and its QuickJS JIT needs a
sandboxed executable-memory entitlement story first. The blocking parity
checklist below remains open; it gates the `src/views/` retirement and the
0.6.1 dependency graph, not the artifact layout.

Both Linux applications were rebuilt and compared at equal window geometry,
using separate synthetic scratch databases. All inherited `BIR_SFTP_*`
variables were removed before setting `BIR_SFTP_DRY_RUN=1`. No live submission
was attempted. Queue creation is not completed transport.

### Blocking repair checklist

Each item remains open until its reference interaction and persistence
acceptance pass. These are required migration work, not a visual redesign.

- [ ] **P1 — Replace unsafe generated form bindings.**
  `tools/extract_specs.py` flattens field arrays and falls back to raw UI keys;
  `app/shell/specs.js` gives both 1700 employer name and taxpayer name the key
  `name`. `app/main.js::openFormView` stores inputs by that key. Headed
  verification showed both labels changing together and retaining the shared
  value after save/reopen. The native `form_1700_view.rs` instead binds
  `taxpayer_name` separately from indexed employer rows. Use explicit typed
  paths and indexed schedule bindings, with unique-input-key assertions and
  roundtrip tests. Acceptance: independently edit taxpayer and two employers;
  each value must remain distinct after reopening and in print output.
- [ ] **P1 — Align period controls, typed data and storage keys.**
  The extracted 1700 `year` field is not `taxable_year`, while
  `app/main.js::saveFormDraft` indexes by `formYear`. The UI allowed Item 1 =
  2025 while the header and saved row stayed 2026. Add reference-equivalent
  period selection with one canonical period; do not silently migrate existing
  drafts by editing an identity field. Acceptance: create/open two years and
  verify header, database summary, typed draft and print agree independently.
- [ ] **P1 — Initialize and validate typed drafts before reporting Save success.**
  `bir_module.rs::save_form_draft` builds typed defaults only for 2551Q/1601C;
  the generic branch persists arbitrary JSON. Headed 1604C Save succeeded but
  Queue failed on missing `amended_return`; 1604E Queue/Print failed on missing
  `taxable_year`. Use the same typed draft constructors and persistence contracts
  as native views, preserving existing state. Acceptance: new and reopened
  annual drafts deserialize successfully before Save reports success; invalid
  fields produce actionable validation rather than deferred serialization errors.
- [ ] **P1 — Port schedules and required filing controls.**
  1604C/E expose scalar identity fields but omit the native schedules and
  repeating rows. 2551Q lacks ATC rows and election controls required by queue
  validation. Flat specs are partial editors, not completed form ports.
  Acceptance: repeat the native row add/edit/remove flow, save/reopen, validate
  and print with identical synthetic data; verify calculations and row order.
- [ ] **P1 — Preserve fresh 1601C defaults without preventing intentional edits.**
  `openFormView` receives null for a new draft and starts from `{}`.
  `saveFormDraft` writes empty text when a field is undefined, so the host
  overlay replaces constructor defaults. Untouched ATC/category still failed
  queue validation; explicitly entering WW010/P permitted queueing. Initialize
  the UI from canonical defaults and distinguish untouched from cleared fields.
  Acceptance: fresh untouched defaults survive save/reopen; intentional changes
  are preserved and invalid values remain rejected.
- [ ] **P1 — Restore reachable taxpayer management and the period library.**
  `app/shell/chrome.js` profile rows only call `selectProfile`; the reference
  profile editing/history/branch/archive flow is not reachable through them.
  The profile dashboard also lacks the reference Forms Set/period grid and
  year navigation. Acceptance: create, edit, reopen, branch, archive/unarchive
  and unlock a synthetic taxpayer; select form periods through the library.
- [ ] **P1 — Repair admin PIN input and prove authentication roundtrips.**
  Headed Settings remained “Admin PIN not set”; paste, numeric typing and
  individual-cell entry left the PIN cells empty. The attempted save said
  “Enter the new 4-digit PIN first.” Input-level root cause remains unconfirmed.
  Logout only deselected a taxpayer and is not authentication evidence.
  Acceptance: set PIN, restart, reject a wrong PIN, accept the correct PIN,
  change/remove PIN; independently test per-profile credentials and TOTP.
- [ ] **P2 — Complete remaining page controls and theme/keyboard parity.**
  Global dashboard compliance/news content, calendar override editing, richer
  settings and job/log controls are missing or reduced; Dev Mode is an explicit
  stub. Privacy checkbox labels disappear in dark mode. Ctrl+F did not focus
  profile search. Acceptance: compare each reference action in both themes,
  including restart persistence and keyboard navigation.
- [ ] **P1 — Demonstrate the complete dry-run filing lifecycle.**
  The comparison proved queue insertion and reopening only. Host queue APIs
  are not evidence that a worker dispatched a payload. Verify worker integration,
  dry-run artifacts and state transitions independently; confirmation/payment
  require their own reference acceptance and must not be inferred from Queued.

### Shell UI redesign (2026-10-15) — separate from the parity checklist

The JS shell now follows the buwiz UI redesign (hexuria/buwiz-ui-redesign
`DESIGN_SYSTEM.md` and prototype, commit 3cfc037). This is a visual and
tax-profile redesign. **It closes none of the P1/P2 items above**; each stays
open until its own reference acceptance passes.

Covered:

- `app/shell/ui.js`: the design tokens (light and dark), type, 4 px grid,
  radii, and the ten wrappers (IconButton, AppButton, NavRow, ContextPill,
  Avatar, Disclosure, Field, StatusChip, SidePanel, Segmented). The
  Components page (command palette → Components) renders them.
- Shell: 40 px title bar drawn by the app (system bar hidden; macOS traffic
  lights at y 14), context pill, sidebar with Recent and the account menu,
  tax-profile and notifications panels closed by their own trigger or Esc,
  command palette, start-a-return dialog, toasts, overlays below 860 px and
  the compact pill below 600 px.
- Screens: Home (with the fresh-install welcome), Form library, Calendar,
  Filings, Settings, the taxpayer page (Profile, Forms, Templates tabs), New
  taxpayer, New branch.
- Tax-profile data layer exposed to JS (`crates/bir-shell/src/tax_profile_module.rs`);
  the tax-year bound and the one-transaction TIN move (and branch-code move)
  live in bir-core with unit tests.
- Form page rendered from `app/forms/forms-schema.json` for all 43 forms,
  bound by rules-corpus key through bir-core's agent seam
  (`crates/bir-shell/src/screen_form_module.rs`); open and validation run off
  the UI thread. Headed check: 0619E, 1601C, 1601EQ and 2550Q opened, took
  input, saved and reopened intact across an app restart, with validation in
  place; one 0619E return was queued (queue insertion only, no transport).

Files the redesign touches that the P1 items also touch: `app/main.js`
(`openFormView`/`saveFormDraft` are gone; the form page uses
`screen_form_*`), `app/shell/specs.js` and `form_view.js` (removed),
`app/shell/periods.js` (period choice in the start-a-return dialog),
`crates/bir-shell/src/screen_form_module.rs` (typed draft construction per
form, including 1601C's unanswered constructor), `app/shell/form_page.js`
(schedule tables, Save/Queue/Print), `app/shell/taxpayer.js` and the retained
`profile_manager.js` (taxpayer management), `app/shell/settings.js` (admin
PIN, the same `OtpInput` as before), and the key bindings in `app/main.js`.
`bir_module.rs` `save_form_draft`/`get_form_draft` are unchanged.

Not done, or different from the prototype, and why:

- 1601EQ's ATC schedule cells show read-only: bir-core's pinned agent
  definition marks them "derived by the app" (the painted page has its own
  ATC row editor). This is the open P1 schedules item.
- Templates snapshot only 1601C and 2551Q returns; other forms have no field
  classification in bir-core yet, and the toast says so.
- Template mode, guided stops and the "Update template?" prompt (D10) are
  not built, as `DESIGN_SYSTEM.md` notes; the Guided mode switch explains this.
- Settings rows the prototype shows for features this build lacks (lock after
  inactivity, back up now, update-template prompt, filing confirmation
  emails) are left out rather than drawn as dead controls.
- The app account row reads "Local account": there is no account record.
- Archive, delete and the per-taxpayer PIN stay on the pre-redesign profile
  editor, linked from Profile › "Email, security and export", so they remain
  reachable.
- GPUI exposes no tabular-figures feature, no `sticky` position and no way
  for a script to focus an Input. TIN and form codes use the UI face with
  default figures; the part navigator opens the chosen part and folds the
  others instead of scrolling to it; the palette routes keys typed at the
  root into its field until the field is clicked. Editable amount fields stay
  left-aligned (the base Input ignores text alignment).
- gpui-shell re-renders only on state change and has no resize event, so the
  app polls the viewport every 200 ms to keep pixel widths current.
- Checked on macOS only (light and dark, 1440 and 800 px). Windows caption
  buttons are drawn but not run on Windows.

### Verified behaviors and coverage limits

Profile creation and visible input frames passed. A reopened monthly 1601C
correctly remains Queued without an unsafe duplicate Queue action, and appears
in Background Tasks. Numeric validation rejected `abc`; a 2551Q amount of 300
survived save/reopen. Sidebar hide/mini and Settings shortcuts, scrolling,
notifications opening, and import chooser opening/cancellation worked.

Six painted forms were inspected and five shell forms exercised: 1601C,
2551Q, 1700, 1604C and 1604E. Shell 2550Q, restart persistence, full
branch/archive/authentication flows, complete import/export and completed
submission transport remain unverified. Testing stopped after establishing the
failed gate; this is not certification of the remaining catalog or of Windows
and macOS runtime behavior.

## Goal and evidence classification (2026-10-14)

Migrate `bir-desktop`'s native GPUI UI (~95k LOC under `src/views/` plus the app
shell in `src/app.rs`, `sidebar.rs`, `top_bar.rs`, `gui.rs`) onto
[gpui-shell](https://gpui-kit.com/shell/): a QuickJS runtime from the
longbridge/gpui-component family where JavaScript describes the interface and
Rust keeps rendering, layout, focus, and all system capabilities. Target: full
feature parity with the native app, with the UI layer living in a scriptable
`app/` directory instead of compiled Rust views.

Evidence so far: a scratch host (`/tmp/shell-probe`) builds against the
monorepo pin below and renders an interactive JS `View` with a working Rust
host-module call on this VM (llvmpipe, `VK_ICD_FILENAMES=/dev/null`). This is
proof of toolchain only, not product parity.

**This plan is not authorization** to merge, publish, submit to BIR, or run the
official application's upload path. All test runs use a fresh
`BIR_DATABASE_PATH`, test TIN `123-456-788-00000`, and `BIR_SFTP_DRY_RUN=1`.
Also set `EBIR_TEST_ENV=1`: without it a scratch database still asks the
macOS keychain for the live master key (`com.ebir.rust`), which must not be
granted to a test run.

## Dependency pins (verified resolving; keep exact)

| Pin | Value | Why |
|---|---|---|
| gpui-shell, gpui-kit, gpui-component, gpui-base | `git = "https://github.com/longbridge/gpui-component"` → `rev = "a0230dbfc47c8a5c6d1f3f099529ff40b1d2e5d2"` | crates.io `gpui-shell 0.1.0` is an empty placeholder (`publish = false`); the real crate ships only via the monorepo git. Tag `v0.7.3` predates the `components` feature/catalog; master rev a0230db has it. |
| gpui / gpui_platform | `gpui-pre = "=0.3.9"`, `gpui-pre-platform = "=0.3.9"` (registry, features `font-kit`, `x11`, `wayland`, `runtime_shaders`) | What the shell family pins. Exact `=` so cargo tolerates bir-desktop's gpui-pre 0.3.4 in the same lockfile. |
| `[patch.crates-io] rquickjs` | `git = "https://github.com/longbridge/gpui-component", rev = "a0230dbf..."` | The monorepo ships `crates/shell/rquickjs-compat`, a facade named `rquickjs` that routes LLRT stdlib's crates.io `rquickjs` dependency onto the same quickjs-jit VM. Without the patch: `ModuleDef` trait conflict, two `rquickjs_core` versions. |
| Lockfile pins (via `cargo update -p <c> --precise`) | `rsa 0.10.0-rc.18`, `pkcs1 0.8.0-rc.4`, `spki 0.8.0`, `der 0.8.1`, `signature 3.0.0`, `ecdsa 0.17.0`, `elliptic-curve 0.14.1`, `ed25519-dalek 3.0.0` | crates.io RCs newer than upstream's lock break `llrt_crypto`/`rsa` inference. Matches upstream `Cargo.lock` at the pinned rev. |
| gpui-agent | unchanged (git rev dab85991) | No gpui deps (hmac/png/serde/sha2/thiserror); works on either graph. |

The two gpui-pre generations (0.3.4 for bir-desktop, 0.3.9 for bir-shell) use
separate workspace lockfiles: `gpui-kit-assets` declares `links`, which prevents
resolving both generations in one graph. The shell has a standalone workspace;
the painted dependency graph remains until the cutover gate passes.

## Architecture

- New crate `crates/bir-shell`: a thin Rust host binary. It owns the window,
  a `ShellRuntime`, the capability policy, and every host module. It contains
  **no business views** — those live in JavaScript.
- New `app/` directory (repo root): `gpui-shell.json` manifest, `main.js`
  (default-exported `class extends View`), and JS modules mirroring today's
  `src/views/` tree (`app/shell/`, `app/forms/`, `app/views/`).
- `bir` host module (`import { ... } from "bir"`): a JSON boundary
  (`HostArguments` → `HostValue`; `async_function` for I/O futures) over
  bir-core/bir-print/bir-rules. Heavy logic stays Rust; JS stays presentational.
- Stays Rust-owned (script cannot reach): SQLCipher db, russh SFTP, IAF crypto,
  bir-print PDF/HTML, `gpui_wry` HTML print preview windows, secondary windows,
  keyring, the gpui-agent control plane, `bir-headless` daemon, `bir-mcp`,
  platform/keychain integration, updater, drag/drop, multi-window.

## Capability policy

Host-installed `Policy` for application `bir` granting: `storage`,
`clipboard_read`, `clipboard_write`, `exit`, `env_variables` (config vars
only), `read_roots`/`write_roots` for the app data dir + user-picked
import/export paths, `execute` for the platform opener only, `http_requests`
limited to BIR endpoints. The JS manifest may declare narrower caps but cannot
widen them.

## Slices (each lands as its own PR; verify before moving on)

| ID | Slice | Contents | Acceptance |
|---|---|---|---|
| S0 | Host skeleton + plan | `crates/bir-shell` binary, `app/` skeleton, policy, `bir` module v0 (`system_info`, `list_profiles`, `forms_catalog`), this doc | `cargo build -p bir-shell`; binary opens a window rendering profiles + form count from real bir-core on a scratch `BIR_DATABASE_PATH` |
| S1 | App shell | sidebar (profiles/nav/dev-mode chip), top bar, theme tokens matching `theme.rs` palette incl. tint helpers, routed content area, profile picker gate | Side-by-side vs native: same routes render; agent snapshot diff reviewed |
| S2 | Auth + settings | lock screen, unlock flow, settings view, import/export | Lock/unlock round-trip on scratch profile; settings persist via `storage`/bir-core |
| S3 | Form kit | comb/currency/date inputs, field validation display, form header, shared widgets (`form_controls`, `form_header`, `event_form_kit`) | Kit drives one ported form end-to-end; validation badges match native output |
| S4 | Form slices A–E | 45 `form_*_view` ports grouped by family (061x/160x/170x/180x/2000x/2200x/255x) — sub-PRs per family | Each form: load spec, edit fields, validate, save draft, print-preview handoff parity vs native on identical seed |
| S5 | Remaining views | dashboard, global_dashboard, profile_manager, queueable_forms, notifications, import_export polish, admin/cron/debug/dev_mode views | Full nav sweep works; no missing-route panics |
| S6 | Print/preview/queue | `bir.print*` module + host-opened `gpui_wry` preview window, filing queue actions, SFTP dry-run path | Print preview of a seeded draft opens; dry-run queue submission produces expected artifacts |
| S7 | Cutover | flip default `bir` binary target to bir-shell, retire `src/views/` + 0.6.1 dep graph, packaging/CI/README updates. *Packaging leg done:* release artifacts ship bir-shell as `bir` + `app/`/`print-overrides/`; MAS still packages bir-desktop until the shell gains a `mas_build`-equivalent entitlement story | `just check`/`just test` green; packaged artifacts build; parity checklist closed |

## Test oracle per slice

- `gpui-shell check app/` (or `runtime.check`) validates the JS bundle headlessly.
- `just check` / `just test` keep the Rust crates green.
- Visual: run bir-shell next to native bir-desktop using separate scratch
  `BIR_DATABASE_PATH` values with equivalent synthetic data. Never open one
  live database concurrently in both applications (owner lock). Compare
  navigation, field values, validation output and persisted state.
- Agent tooling: rebuild `--features agent` parity for the shell host later in
  the series (agent plane stays in bir-desktop until then).

## Risks / open questions

- `rev = a0230db` is an untagged master commit (newest tag v0.7.3 lacks the
  components catalog). Bump to the next tag when published.
- Some views may need host-module additions (e.g. webview preview is a second
  window — script can't open windows, host does it on request).
- JS typecheck: generated `gpui-kit.d.ts` + `jsconfig.json` `checkJs` gives
  static checking; wire into `just check` in S0.
- Performance expectation: cached `RenderSnapshot` means JS render runs only on
  state change; verify form views with heavy grids (e.g. 1700-series schedules)
  stay smooth before S4 sign-off.
