# gpui-shell migration plan: JavaScript UI on the QuickJS shell

Tracking issue: [#135](https://github.com/hexuria/buwiz-forms/issues/135).
Tick items there and here in the same PR.

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

## Dependency pins (verified resolving; keep exact)

| Pin | Value | Why |
|---|---|---|
| gpui-shell, gpui-kit, gpui-component, gpui-base | `git = "https://github.com/longbridge/gpui-component"` → `rev = "a0230dbfc47c8a5c6d1f3f099529ff40b1d2e5d2"` | crates.io `gpui-shell 0.1.0` is an empty placeholder (`publish = false`); the real crate ships only via the monorepo git. Tag `v0.7.3` predates the `components` feature/catalog; master rev a0230db has it. |
| gpui / gpui_platform | `gpui-pre = "=0.3.9"`, `gpui-pre-platform = "=0.3.9"` (registry, features `font-kit`, `x11`, `wayland`, `runtime_shaders`) | What the shell family pins. Exact `=` so cargo tolerates bir-desktop's gpui-pre 0.3.4 in the same lockfile. |
| `[patch.crates-io] rquickjs` | `git = "https://github.com/longbridge/gpui-component", rev = "a0230dbf..."` | The monorepo ships `crates/shell/rquickjs-compat`, a facade named `rquickjs` that routes LLRT stdlib's crates.io `rquickjs` dependency onto the same quickjs-jit VM. Without the patch: `ModuleDef` trait conflict, two `rquickjs_core` versions. |
| Lockfile pins (via `cargo update -p <c> --precise`) | `rsa 0.10.0-rc.18`, `pkcs1 0.8.0-rc.4`, `spki 0.8.0`, `der 0.8.1`, `signature 3.0.0`, `ecdsa 0.17.0`, `elliptic-curve 0.14.1`, `ed25519-dalek 3.0.0` | crates.io RCs newer than upstream's lock break `llrt_crypto`/`rsa` inference. Matches upstream `Cargo.lock` at the pinned rev. |
| gpui-agent | unchanged (git rev dab85991) | No gpui deps (hmac/png/serde/sha2/thiserror); works on either graph. |

Two gpui-pre versions (0.3.4 for bir-desktop, 0.3.9 for bir-shell) coexist in
the workspace lockfile — verified via `cargo metadata`. bir-desktop's dep graph
stays untouched until the final cutover slice.

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
| S7 | Cutover | flip default `bir` binary target to bir-shell, retire `src/views/` + 0.6.1 dep graph, packaging/CI/README updates | `just check`/`just test` green; packaged artifacts build; parity checklist closed |

## Test oracle per slice

- `gpui-shell check app/` (or `runtime.check`) validates the JS bundle headlessly.
- `just check` / `just test` keep the Rust crates green.
- Visual: run bir-shell next to native bir-desktop on the same seeded
  `BIR_DATABASE_PATH`; compare navigation, field values, and validation output.
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
