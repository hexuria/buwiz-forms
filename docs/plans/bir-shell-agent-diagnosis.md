# bir-shell agent breakage on current main

Diagnosis only. No product code was changed. Run on `main` at `16160ec5` (22 Oct 2026).

Scratch databases only: `EBIR_TEST_ENV=1`, `BIR_SFTP_DRY_RUN=1`, `BIR_DATABASE_PATH` under `/tmp/scratch-bir/`. Nothing was submitted to the BIR. The taxpayer written on the headless scratch database is synthetic (`Scratch Taxpayer`, TIN `12345678800000`).

Clients used:

- `bir-shell` debug build, `GPUI_AGENT=1`, `GPUI_AGENT_TOKEN=dev-secret`, `GPUI_AGENT_ADDR=127.0.0.1:17423`, `BIR_SHELL_APP` pointed at `app/`.
- `gpui-agent` CLI built from the pin in the root `Cargo.toml`: `hexuria/gpui-agent` rev `dab85991d700b8f3410cbaacbc5d9f90568c7ee3`.
- `bir-mcp` from `cargo build --locked -p bir-mcp --bin bir-mcp`.
- `bir-headless` from `cargo build --locked -p bir-desktop --bin bir-headless --features agent`.

Effort below is scope, not a calendar estimate.

- **Small.** One module and its tests. No new protocol.
- **Medium.** Optional `agent` feature on the shell workspace, a listener, discovery, and a short command path into the JS app.
- **Large.** A new `AgentHost` for the painted invoke list, written against the JS screens. `BirAgentHost` cannot be moved: `crates/bir-shell` is its own Cargo workspace on gpui-pre 0.3.9, and `gpui-kit-assets` `links` blocks sharing a graph with bir-desktop (gpui-pre 0.3.4).

## What the run showed

`bir-shell` opens and holds the live-database owner lock. With `GPUI_AGENT=1` and a token it still binds nothing. The shell log has no `gpui-agent` line. Ports `17421` and `17423` refuse TCP (`errno 111`). The window stays on the first-run setup screen ("Who will use eBIRForms on this computer?"). `nav.go` does not run.

`bir-mcp` starts, speaks MCP, and lists 16 tools. Against the running shell, `bir_list_apps` returns `apps: []`. Every tool that needs a selected app returns `isError: true`.

`bir-headless` on its own scratch database still is an `AgentHost`. `gpui-agent` and `bir-mcp` complete profile create, `1701Q` open, fill, and validate there. Pointed at the shell's open database, `bir-headless serve` exits 1 with the owner-lock error. It does not attach to the shell window. `--connect bir-shell` still fails while that daemon is up.

The migration plan already records this as deferred work: `docs/plans/gpui-shell-migration.md` ("Agent tooling: rebuild `--features agent` parity for the shell host later in the series (agent plane stays in bir-desktop until then).") and the architecture note that the gpui-agent control plane, `bir-headless`, and `bir-mcp` stay Rust-owned and were not ported into the JS shell.

## Shared root cause for every missing shell op

`crates/bir-shell/Cargo.toml` has no `agent` feature and no `gpui-agent` dependency. `bir_shell::host::run` opens the database (`bir_module::open_database_at_launch`) and the main window. It never calls an equivalent of `bir_desktop::agent::bridge::maybe_start`, never `spawn_mailbox_named` / `spawn_host`, and never `discovery::publish_current` (painted path: `bridge::maybe_start` → `instance::publish`, app name `instance::APP` = `"bir-desktop"`).

`bir-mcp` would still ignore a future record whose `app` is not `"bir-desktop"` and whose executable basename is not `bir` or `bir-headless`. That filter is `bir_mcp::apps::is_bir` (`BIR_APP`, `BIR_EXECUTABLES`). The dev binary name is `bir-shell`. Packaged installs rename the binary to `bir` (`scripts/alias_desktop_bin.py`); that basename matches `BIR_EXECUTABLES` only after a discovery record exists.

Painted stable ids (`crates/bir-desktop/src/agent/ids.rs`, for example `global_dashboard_btn`, `profile-tin`, `page-form-1701q`) are not the shell ids (`app/shell/chrome.js`: `nav-home`, `nav-library`, `nav-calendar`, `nav-filings`). `app/shell/ui.js` documents stable ids for gpui-agent, and nothing in the shell builds a `UiTree`.

## Capability list

Status is against **bir-shell on current main**. "Works on bir-headless" means the old daemon, on its own scratch database, on this same commit.

### Control plane

| Capability | Status | Exact symptom | Root cause | Fix | Effort |
|---|---|---|---|---|---|
| Opt-in loopback bind, protocol v2 HMAC, `hello` | **missing** | `gpui-agent --token dev-secret hello` → `error: connect 127.0.0.1:17421 failed: Connection refused (os error 111)`, exit 1. Same for `--addr 127.0.0.1:17423` (the address passed to the shell). Python sockets: `ConnectionRefusedError` errno 111. Shell log never mentions gpui-agent. | `host::run` does not start a listener. Painted start is `bridge::maybe_start` (`GPUI_AGENT=1`, `from_env`, `spawn_mailbox_named`). | Optional `agent` feature, same gpui-agent rev. Start only when `GPUI_AGENT=1`, token required, loopback unless the existing remote flag is set. | Medium (feature + shell `Cargo.lock` + one bridge module) |
| Discovery / `--connect` | **missing** | With `GPUI_AGENT_ADDR` unset: `error: no AgentHost is running for 'bir-shell'`. Same string for `bir-desktop` and `bir` while only the shell is up. | No `discovery::publish_current`. Painted publish is `instance::publish` from `bridge::maybe_start`. | Publish a host record from the shell process. Choose `app`: `"bir-desktop"` matches `is_bir` as-is; `"bir-shell"` needs the `apps.rs` change below. | Small once the listener exists |
| `snapshot` | **missing** | Not reached. The CLI cannot connect, so no tree is returned. The setup screen never changes. | No `AgentHost`. Shell ids are not walked. | Implement `snapshot` from the JS tree (stable ids in `app/shell/ui.js`). TCP thread must not touch GPUI; a script timer should apply host commands. | Medium |
| `click`, `set-value`, `type`, `key` | **missing** | Not reached. No field on the setup screen changed. | No `AgentHost`. Painted semantic delivery is `BirAgentHost`; virtual HID returns `virtual_unavailable` (`AGENT.md`). | Semantic ops on shell ids. One filled field goes through `app/shell/form_page.js` `FormPageActions.choose` (or the host equivalent), then a UI refresh. | Medium |
| `keybindings` | **missing** | Not reached. | Painted dispatch is GPUI Actions on the desktop window (`AGENT.md`). The shell has no agent mailbox drain. | Map shell commands to the same confirm rules (`app.quit` needs `confirm=true`). | Medium, inside the host above |
| `screenshot`, including macOS scrolled capture | **missing** | Not reached. | Painted capture is the macOS mailbox drain (`screencapture`). Linux, Windows, and headless `spawn_host` already return `screenshot_unavailable`. | Keep that honest error until a window capture exists. macOS scrolled tiles are part of the large host, tied to painted scroll targets (`form-1601c-scroll`). | Large for macOS parity. Small to return `screenshot_unavailable` on day one. |
| Agent `shutdown` / `wait` of the shell | **missing** | Nothing is listening to shut down. | No mailbox. | Same listener. `bir-headless serve --wait` stays the no-window handoff; it does not become a shell attach. | Small, with the listener |

### Invoke ops (`BirAgentHost::invoke`, `crates/bir-desktop/src/agent/host.rs`)

Every row below was unreachable on bir-shell: the client dies in connect or discovery before `invoke`. `nav.go` was sent explicitly and failed the same way. The others are missing for that same reason; they were not given a separate error string because no host accepted the op.

| Capability | Status | Exact symptom | Root cause | Fix | Effort |
|---|---|---|---|---|---|
| `nav.go` (`home`, `library`, `calendar`, `filings`, form pages) | **missing** | `gpui-agent --connect bir-shell invoke nav.go --arg page=library` → `error: no AgentHost is running for 'bir-shell'`, exit 1. Window stayed on setup. | `invoke` match arm `"nav.go"` calls `ids::view_from_slug` and `navigate` on the painted `ActiveView`. Shell navigation is `app/main.js` `go` / `runStartRoute` (`BIR_SHELL_START`), with no agent caller. | Host op that asks the JS app to `go()`. Shell route ids differ from painted slugs. | Medium |
| `app.info` | **missing** | Not reached. | `invoke` arm `"app.info"` → `BirAgentHost::app_info`. | Return `{app, mode, database, profiles}` from the shell's open database. | Small, inside the new host |
| `profile.create`, `profile.save`, `profile.list`, `profile.search`, `profile.set`, `profile.edit`, `profile.tab`, `profile.html`, `profile.forms_set.get`, `profile.forms_set` | **missing** | Not reached on the shell. On bir-headless these profile ops succeed (see below). | Painted editor widgets and `ids.rs` (`profile-tin`, `profile-name`, `save_profile`, …). Shell taxpayers go through `tax_profile_module` (`list_taxpayers`, `create_taxpayer`) and are `TaxpayerWithBranches`. | Call those host functions and sync the JS profile screen. Keep `profile.forms_set` behind `confirm=true`. `profile.ensure` stays unimplemented (painted host already errors). `profile.calendar_sync` stays the painted refusal (needs a linked Google Calendar). | Large for full editor parity. Medium for `profile.list` / `profile.set` / `profile.save` on the scratch DB. |
| `dues.list`, `dues.html`, `calendar.html`, `tax-dues.refresh`, `calendar.add` | **missing** | Not reached on the shell. | `invoke` arms in `BirAgentHost::invoke`. | Read dues from bir-core for the selected profile. `calendar.add` opens the painted add flow; the shell needs the same confirm behavior. | Medium for `dues.list`. Large for calendar UI parity. |
| `jobs.list`, `alerts.list`, `alerts.dismiss`, `submissions.list` | **missing** | Not reached on the shell. | Same `invoke` arms. The shell already runs `background_cron` via `bir_module::start_submission_worker` on the owner connection. | Expose the same lists through the host. `alerts.dismiss` needs `args.id`. | Medium |
| `search.open`, `palette.open`, `palette.search`, `dashboard.set_forms`, `dashboard.filter` | **missing** | Not reached on the shell. | Painted command palette and dashboard filters. | Reimplement against `app/shell` search and dashboard state. | Large |
| `filing.start` / `form.open` | **missing** | Not reached on the shell. bir-mcp `bir_form_open` never got this far (selection error below). On bir-headless, `filing.start` code `1701Q` year `2026` period `1` returns ok. | Painted open path is `host_generic` (`open_generic`, `page_period`, `load_2550q`). Shell equivalent `screen_form_module::load_draft` is private. | Make `load_draft` callable from the agent host and tell the JS app to show that draft. | Medium |
| `form.fields`, `form.context`, `form.fill`, `form.needs_you` | **missing** | Not reached on the shell. On bir-headless, `form.fields` returned 64 boxes for `1701Q`; `form.fill` of `frm1701q:txtSheets=2` applied that key. A bogus key `note` was refused: `` `note` is not a box on Form 1701Q ``. | `form_fields` / `form_fill` on `BirAgentHost`, backed by `bir_core::forms::agent` (`fill`, `field_map`, `needs_you`). | Call `bir_core::forms::agent` from the shell host and push the edit through `FormPageActions.choose` so the screen matches. | Medium |
| `filing.validate` / `form.validate` | **missing** | Not reached on the shell. On bir-headless, `form.validate` returned `ok: false` with 4 official errors (birth date item 11, filer type item 7, ATC item 8, tax rate item 16). That is the real rules path: `bir_core::forms::agent::validate` recomputes, then checks. | `invoke` arm `"filing.validate" \| "form.validate"` → `validate_form`. | Same call from the shell host. | Medium, with form open |
| `form.preview`, `form.pdf` | **missing** | Not reached on the shell. | `form_preview` on the painted host. The shell already has print preview (`print_preview_window`, `print_sheet`) for the human UI. | Point the op at that preview. Headless must keep an honest "no window" error for `form.print`. | Medium |
| `form.card` | **missing** | Not reached on the shell. | `form_card` / `bir_core::forms::agent` card boxes. | Return the same card JSON. | Small, once a form is open in the host |
| `form.review_in_app` | **missing** | Not reached on the shell. | Desktop-only: highlights Submit in the painted window. Headless refuses it. | Implement only when the shell window is up. A headless-style host keeps the refusal. | Medium |
| `form.save_draft`, `form.dismiss` | **missing** | Not reached on the shell. | `save_form_draft` / `form_dismiss`. Core helper is `bir_core::forms::agent::save_draft`. Shell drafts already persist through `bir_module` `get_form_draft` / `save_form_draft`. | Call those and refresh the JS store (`app/shell/store.js` `bump`). | Medium |
| `form.print` | **missing** | Not reached on the shell. | `form_print`. Headless already errors: `form.print needs the desktop window's frozen HTML preview; headless cannot print`. | Shell window can call the existing print preview. Do not pretend headless can print. | Medium on the shell window |
| `form.revert_draft`, `form.release_abandoned_claim`, `form.mark_paid`, `form.upload_receipt` | **missing** | Not reached on the shell. | Those `invoke` arms. Several are 1601C/2551Q only and confirm-gated. | Port the confirm gates onto the shell's draft screens. | Large |
| `filing.submit` | **missing** | Not reached on the shell. | `request_submit_confirm`. It opens the confirm gate. It does not queue or file by itself. | Same gate in the shell. Do not add an agent path that submits to the BIR. | Medium for the confirm highlight. The dry-run queue stays a separate, explicit op. |
| `form.queue`, `filing.queue`, `form.submit` | **missing** | Not reached on the shell. | `queue_open_return`. | If exposed, keep `BIR_SFTP_DRY_RUN=1` in tests. This diagnosis did not queue anything. | Medium, and it must stay confirm-gated |
| `form.file`, `form.submit_external`, `filing.file`, `profile.ensure` | **missing** as a success path on purpose | Painted `invoke` already returns an error for these names. | Existing refusals in `BirAgentHost::invoke`. | Keep the refusals on the shell host. | Small (copy the error strings) |

### bir-mcp (`crates/bir-mcp`)

The adapter does not embed bir-core. It discovers a running host and invokes gpui-agent ops. Closed set in `tools.rs` `ALLOWED_OPS`: `hello`, `app.info`, `profile.list`, `dues.list`, `form.open`, `form.fields`, `form.context`, `form.fill`, `form.needs_you`, `form.validate`, `form.preview`, `form.card`, `form.review_in_app`, `form.save_draft`, `form.dismiss`.

| Capability | Status | Exact symptom | Root cause | Fix | Effort |
|---|---|---|---|---|---|
| Process, `initialize`, `tools/list` (16 tools) | **works** | Exit 0. Tools: `bir_list_apps`, `bir_connect`, `bir_status`, `bir_profiles_list`, `bir_dues_list`, `bir_form_open`, `bir_form_fields`, `bir_form_context`, `bir_form_fill`, `bir_form_needs_you`, `bir_form_validate`, `bir_form_preview`, `bir_form_card`, `bir_form_review`, `bir_form_save_draft`, `bir_form_dismiss`. | None. This process does not need the shell. | None for listing. | — |
| `bir_list_apps` while bir-shell is running with `GPUI_AGENT=1` | **broken** | `isError: false`, `"apps": []`, `selected: null`, next text: `No BIR app is running. Ask the user to open BIR with the agent enabled (GPUI_AGENT=1), then call bir_list_apps again.` | No discovery record, and `apps::is_bir` would drop a `bir-shell` record anyway. | Shell publishes a record `is_bir` accepts. Extend `BIR_EXECUTABLES` / `BIR_APP` if the app name or dev binary is `bir-shell`. Update the tests in `apps.rs`. | Small, after the listener |
| `bir_connect` | **broken** against the shell | No id is ever returned to connect to. | Same empty list. | Same publish. | Small |
| `bir_profiles_list`, `bir_form_open` (`1701Q` / `2026` / `1`, TIN `12345678800000`), `bir_form_validate` | **broken** against the shell | Each call: `isError: true`, `no BIR app selected — call bir_list_apps, ask the user which one, then bir_connect`. | `tools` refuses before any host op when `selected` is null. The refusal is the adapter's empty-selection path. The shell never becomes selectable. | Publish, then implement the matching `ALLOWED_OPS` on the shell host. | The selection check is small. The ops are the Medium host work above. |
| `bir_status`, `bir_dues_list`, `bir_form_fields`, `bir_form_context`, `bir_form_fill`, `bir_form_needs_you`, `bir_form_preview`, `bir_form_card`, `bir_form_review`, `bir_form_save_draft`, `bir_form_dismiss` | **missing** against the shell | Not given their own host error. With no selection they take the same `no BIR app selected` refusal. | Same as the row above. | Same host ops. | Medium-large for the whole `ALLOWED_OPS` set on the shell |
| Those same tools against bir-headless on its own scratch DB | **works** | `bir_list_apps` returned one app: id `pid:56765`, mode `headless`, database `test`, token `known`, taxpayer Scratch Taxpayer. `bir_connect`, `bir_profiles_list`, `bir_form_open` (`already_open: true`), `bir_form_fill` of `frm1701q:txtSheets` to `3`, and `bir_form_validate` all returned `isError: false`. The shell was still absent from `apps`. | Headless publishes `app=bir-desktop`, executable `bir-headless`, which `is_bir` accepts. | None for the daemon. It is not a shell session. | — |

### bir-headless

| Capability | Status | Exact symptom | Root cause | Fix | Effort |
|---|---|---|---|---|---|
| `serve` on a free scratch DB | **works** | Log line: `gpui-agent listening on 127.0.0.1:17421 (platform=headless, app=bir-desktop)`. | `headless::run` → `spawn_host` + `open_serve_database`. Unchanged on this commit. | Keep this binary as the no-window host. | — |
| `gpui-agent --connect bir-headless` hello, profile create, set-value, `save_profile`, `profile.list`, `filing.start` 1701Q, `form.fields`, `form.fill`, `form.validate` | **works** | Hello `app: bir-desktop`, `platform: headless`, `ready: true`. Fill and validate results are the ones cited above. | `BirAgentHost::invoke` in the desktop crate, feature `agent`. | None on this daemon. | — |
| `serve` on the database bir-shell already has open | **broken** | Exit 1. `live database /tmp/scratch-bir/shell.db is already open (painted bir or bir-headless). Single live-DB owner: quit the other process. There is no protocol Op::Yield.` | `headless::open_serve_database` maps `db::DbError::LiveDatabaseInUse` through `format_serve_open_error` (`error.to_string()`). The shell already took the lock in `bir_module::database_handle` → `db::open_live_database`. The error text is `DbError::LiveDatabaseInUse` in `crates/bir-core/src/db/mod.rs`. | Leave the lock. The shell has to be the agent host so a second writer is unnecessary. Do not add an attach mode that opens the same live file. | The lock stays. The effort is the shell host, not a lock change. |
| `--connect bir-shell` while bir-headless is serving | **missing** | `error: no AgentHost is running for 'bir-shell'`. bir-mcp lists only the headless app. | Headless discovery app name is `bir-desktop`. It never drives the shell window. | Shell publishes its own record. | Small, with the listener |
| `status`, `logs`, `shutdown`, `detach`, `wait` | **works** as bir-headless CLI behavior on main; **not re-run** in this pass | Not pointed at bir-shell. They manage the daemon process, not the shell window. | `crates/bir-desktop/src/agent/headless.rs` / `crates/bir/src/main.rs`. | No change so they can manage the daemon. They will not gain a shell attach. | — |

## What to build after this report

1. Optional `agent` feature on `crates/bir-shell`, gpui-agent rev `dab85991`. `host::run` starts it only when `GPUI_AGENT=1`. Token required. Loopback.
2. Publish discovery. Either `app=bir-desktop` (drop-in for `is_bir`) or `app=bir-shell` plus `apps::is_bir` / `BIR_EXECUTABLES`.
3. `AgentHost` for `hello`, `snapshot`, semantic `click` / `set-value`, and `invoke`, using `bir_core::forms::agent` and `screen_form_module::load_draft` (make that function visible to the host). Sync the JS view with a polled command (`go`, open form, `FormPageActions.choose`). The gpui-agent TCP thread must not touch the GPUI window.
4. Desktop-only ops stay honest errors when there is no window (`screenshot_unavailable`, `form.print`, `form.review_in_app`).
5. `just shell-check` and `.github/workflows/ci.yml` `shell-check` gain `cargo check --features agent` on Linux, macOS, and Windows. Toolchain stays Rust 1.99.0. Tests stay as strict as they are.
6. `bir-headless` stays the no-window host on its own database. It does not share the shell's live file.

First slice that unblocks the product demo: listener + discovery + `is_bir` + `profile.list` + `form.open` / `form.fill` / `form.validate` on a scratch DB, with the JS screen following. Full painted parity (palette, dashboard filters, keybinding Actions, macOS scrolled capture, receipt upload, abandoned-claim confirms) is the large follow-on.

## Out of scope for this run

buwiz-server OAuth and license checks were not run. `bir-mcp` today authenticates with `GPUI_AGENT_TOKEN`, `BIR_MCP_TOKENS`, and `BIR_MCP_HTTP_TOKEN` (`crates/bir-mcp/src/tokens.rs`, `http.rs`). It does not check a buwiz-server access token. That wiring is a later change. It does not explain the connection refused / empty `apps` results above.

## Evidence

- Shell + agent + bir-mcp refusal, setup screen still up: `bir-shell-agent-refused.png`, recording `bir-shell-agent-mcp-refused.mp4` (readable failure text at the start and again just after 01:07; the terminal is blank from about 00:12 to 01:06; the last frames are a cube logo).
- Headless refused the shell's open database, shell still on the setup screen: `headless-locked-out-of-shell-db.png`.
