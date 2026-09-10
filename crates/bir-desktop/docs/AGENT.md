# gpui-agent integration (bir-desktop)

Opt-in control plane so Grok Bot (or any MCP/CLI client) can drive **eBIRForms**
beside a human: navigate pages, create a tax profile, inspect dues, and prepare
a form draft. It speaks the generic gpui-agent protocol only. BIR-specific
verbs live in this host as `invoke` names. Start at the
[AI agent playbook](#ai-agent-playbook) and
[Authorization](#authorization) if you are driving painted `bir` or
`bir-headless` from CLI / MCP / Grok Bot.

Pinned crate: [`gpui-agent`](https://github.com/hexuria/gpui-agent) commit
`8857139af12fb033b4dd04eabd8d19b5bfc5ffc6` (`main` tip, Merge PR #32 / epic
children). Host GPUI is **gpui-pre** through gpui-kit 0.6.1. Cookbook:
[`docs/INTEGRATING.md`](https://github.com/hexuria/gpui-agent/blob/8857139af12fb033b4dd04eabd8d19b5bfc5ffc6/docs/INTEGRATING.md)
and [`docs/SDK.md`](https://github.com/hexuria/gpui-agent/blob/8857139af12fb033b4dd04eabd8d19b5bfc5ffc6/docs/SDK.md).
Do not fork the protocol. There is no crates.io release; git/path only.

## Contents

- [Locked protocol contract](#locked-protocol-contract)
- [Two hosts, one agent port](#two-hosts-one-agent-port) (`--wait` handoff)
- [Authorization](#authorization)
- [Security gates](#security-gates)
- [AI agent playbook](#ai-agent-playbook) (capabilities / limits, including Forms Set)
- [Mac: build and run beside Grok Bot](#mac-build-and-run-beside-grok-bot)
- [Mac: `bir-headless`](#mac-bir-headless)
- [Linux CLI smoke](#linux-cli-smoke-buwiz-box)
- [Recipes](#recipes)
- [Invoke allow-list](#invoke-allow-list-bir-host-only)
- [Alias table](#alias-table)
- [Stable IDs](#stable-ids)
- [Coverage](#coverage-this-slice)
- [Claimed queue without BIR outcome](#claimed-queue-without-bir-outcome-facts)

## Locked protocol contract

These are host constraints. They do not change protocol v1.

- Depend on git/path `gpui-agent` at that rev only (no crates.io).
- `bir-core` has no GPUI / gpui-agent dependency. Painted-window and virtual
  glue stay behind bir-desktop `--features agent`.
- Loopback default via `gpui_agent::from_env` / `authorize_bind`. Non-loopback
  needs `GPUI_AGENT_REMOTE=1` and a non-empty `GPUI_AGENT_TOKEN` (SDK, not a
  BIR-invented bind). This host does not add a second bind path.
- **Two AgentHosts must not share a bind or a live SQLCipher file.** Painted
  `bir` and `bir-headless serve` both default to `127.0.0.1:17421` and take
  `bir_data.db.owner.lock`. See [Two hosts, one agent port](#two-hosts-one-agent-port).
  There is **no** protocol `Op::Yield` / `Takeover`.
- ADR-001 ([daemon SoT, GUI as protocol client](https://github.com/hexuria/gpui-agent/blob/8857139af12fb033b4dd04eabd8d19b5bfc5ffc6/docs/ADR-001-daemon-sot.md))
  is the long-term shape. **This slice is shared persistence only:** the
  daemon opens `default_database_path()` (`platform::data_dir()/bir_data.db`
  + the same SQLCipher key). Smoke C reopen of the GUI is offline
  verification of that file, not live sync. The Mac GUI is **not** a
  protocol client of the daemon yet.
- Preferred BIR `invoke` names (app-only, not CLI/MCP verbs) include
  `nav.go`, `profile.list` / `profile.search` / `profile.set` / `profile.edit` /
  `profile.tab`, `dues.list`, `jobs.list`, `search.open`, `palette.search`,
  `form.fill` / `form.pdf`, plus the original `profile.create`,
  `tax-dues.refresh`, `filing.start`, `filing.validate`, `filing.submit`,
  and `form.release_abandoned_claim`.
  `filing.submit` maps to the existing confirmation gate; it does not queue or
  file. Match-arm synonyms (see the Alias table) are not a second allow-list.
  There is **no** `profile.ensure` auto-write; `profile.create` only opens the
  editor.
- Semantic delivery is the supported path. Virtual ops return
  `virtual_unavailable` rather than synthesizing OS HID or a half-wired
  in-window pointer. Protocol is unchanged.
- Screenshot is observe-only. The UI-thread mailbox drain intercepts
  `Op::Screenshot`: macOS writes **this** window via
  `capture_window_via_screencapture` (`screencapture -l`, Screen Recording).
  Linux, Windows, and headless `spawn_host` stay `screenshot_unavailable`.
  The semantic host has no `Window` and does not invent a PNG.
- Filing status SoT is the `form_drafts` row for that TIN/year/month (the same
  queued id `submissions.list` shows). Snapshot / `form.fields` /
  `form-1601c-status` overlay that row; a stale local Draft cannot mask
  Queued+claimed until the release CAS writes Draft.

## Two hosts, one agent port

Painted GUI `bir` (`--features …,agent`, in-process mailbox on the GPUI window)
and daemon `bir-headless serve` (no GPU) both speak the **same**
gpui-agent protocol on loopback **`127.0.0.1:17421`** (override with
`GPUI_AGENT_ADDR`). Clients (CLI / MCP / Grok Bot) talk to whoever currently
holds that bind. `hello.platform` is `desktop` vs `headless`.

**One owner at a time** of the TCP bind **and** the live SQLCipher file.
Painted `bir` and `bir-headless serve` both take an exclusive sidecar lock
(`bir_data.db.owner.lock`) around `default_database_path()` (or
`BIR_DATABASE_PATH` in CI). A second AgentHost on the same live DB is refused
(`LiveDatabaseInUse`). There is no silent two-writer bridge.

### Who should own the port

- **Day-to-day Mac:** the painted app is the host **when it is open**.
- **Headless:** when the GUI is closed, and on Linux/box VMs with no GUI.

Do **not** run GUI and headless concurrently as writers. After headless work,
`bir-headless shutdown` (or kill) then open painted `bir`. If Uriah later
wants GUI updates while the daemon runs, that is a protocol client — not this
slice.

### `--wait` handoff

There is **no** protocol `Op::Yield`. Handoff is process ownership, not a
semantic yield.

- `bir-headless serve --wait` (also `bir-headless --wait`) waits for bind
  **and** the live-DB owner lock. While the GUI holds them, headless prints
  `waiting for bind 127.0.0.1:17421 …` or `waiting for live DB owner lock …`.
  After GUI quit, headless binds and serves.
- **Without `--wait`**, a busy bind fails immediately (exit 2) and a busy
  live-DB lock fails immediately (`LiveDatabaseInUse`, exit 1).
- Clean GUI **Quit** (Cmd+Q / tray Quit / `gpui-agent shutdown`) releases bind
  + owner lock. Hiding or closing the window does **not**.

```bash
# GUI already up on 17421 with the live app-group DB
cargo run --locked --bin bir-headless --features agent -- serve --wait
# quit painted bir (Cmd+Q) → headless owns 17421
# after headless work:
cargo run --locked --bin bir-headless --features agent -- shutdown
# then open painted bir
```

## Authorization

This is the token model AI agents must follow. Never print or log the token.

| Gate | Rule |
| --- | --- |
| Feature | `agent` is **off** by default. Product/release builds must leave it off. |
| Opt-in | Runtime starts only when `GPUI_AGENT=1` (`true`/`yes`/`on`). |
| Release | Release binaries also need `GPUI_AGENT_ALLOW_RELEASE=1`. |
| Bind | Loopback `127.0.0.1:17421` unless `GPUI_AGENT_ADDR` is set. `from_env` calls `authorize_bind`. Non-loopback needs `GPUI_AGENT_REMOTE=1` **and** a non-empty token. Transport is still plaintext TCP. Do not invent a second bind path. |
| Shared secret | `GPUI_AGENT_TOKEN` (local smokes use `dev-secret`). CLI / MCP / Grok Bot must send the **same** token. When set, it is required on every request. |
| Live default DB | **`bir-headless serve` requires a token** against live `default_database_path()` (Mac app-group `~/Library/Group Containers/group.dev.goldcoders.bir/bir_data.db`, Linux `~/.taxman-ebir/bir_data.db`). Landed as “Require a token on live default_database_path for bir-headless”. |
| Path override | `BIR_DATABASE_PATH` is for CI / temp demos. A **non-empty** override may omit the token (`live_database_token_required` is false). Prefer still setting a token so recipe/MCP clients match. Do **not** set `BIR_DATABASE_PATH` for live Mac smokes. |
| Painted `bir` | Does **not** apply that live-path token refuse (mailbox starts from `from_env` alone). Recipes and MCP still need the same non-empty token on host and client when you set one. |
| `hello.auth` | `"required"` when a token is configured on the host, `"none"` otherwise. The TCP thread enforces the token; the mailbox drain (painted `bir` and `bir-headless`) passes it into `handle_request` so hello does not overwrite `auth` to `"none"`. `BirAgentHost::hello()` does not set `auth` by hand. |
| Logging | **Never log the token.** Opt-in `GPUI_AGENT_LOG_REQUESTS=1` (`true`/`yes`/`on`) emits one stderr line per request: `timestamp gpui-agent id=… op=hello\|invoke\|… name=profile.list ok=true`. Off by default. Not enabled by `RUST_LOG`. Invoke args, `set_value` values, typed text, screenshot paths, and tokens are never included. Same helper on the painted mailbox drain. |

## Security gates

These still apply to every agent, painted or headless:

- No auto `profile.ensure` (rejected). `profile.create` opens the editor only.
  `profile.save` is the explicit persist after human confirm for live
  taxpayers.
- Never queue or file externally. `filing.submit` is confirmation-only (exposes
  the confirmation node; does not queue or file). Confirming
  `form-1601c-submit-confirm` is refused. Complete filing in the BIR UI.
  `form.release_abandoned_claim` only returns a claimed Queued snapshot to Draft
  after a human confirmed nothing reached BIR; it does **not** file.
- Never skip the lock screen, profile PIN/TOTP, or administrator OTP. Unsaved
  profile compliance still blocks navigation.
- No virtual HID on this host. `hello.deliveries` is `["semantic"]`. Virtual
  `click` / `type` / `key` return `virtual_unavailable`. Do not point agents at
  `--delivery virtual`.
- Snapshots include names, last-4 TIN, dues, and editor fields needed to drive
  flows. They do not include PIN hashes, TOTP secrets, or keychain material.

## AI agent playbook

Use this section as the day-to-day recipe. Invoke names must match the
[allow-list](#invoke-allow-list-bir-host-only). Do not invent verbs
(`profile.forms_set`, `Op::Yield`, `filing.queue`, …).

### Client env

```bash
export GPUI_AGENT_ADDR=127.0.0.1:17421
export GPUI_AGENT_TOKEN=dev-secret   # must match the host
```

Host (whichever owns the port):

```bash
export GPUI_AGENT=1
export GPUI_AGENT_TOKEN=dev-secret
export GPUI_AGENT_ADDR=127.0.0.1:17421
# optional: export GPUI_AGENT_LOG_REQUESTS=1
# painted:
#   cargo run --locked --bin bir --features dev-tools,agent
# headless (GUI closed, or Linux/box):
#   cargo run --locked --bin bir-headless --features agent -- serve
#   cargo run --locked --bin bir-headless --features agent -- serve --wait
```

CLI `--arg`s are repeated **`KEY=VALUE`**. Values are JSON if they parse
(`true`, `2026`, `{"any_taxes_withheld":false}`), otherwise strings. Do **not**
pass a single JSON-object `--arg`. Invokes with no args omit `--arg`.
`set-value` is positional: `gpui-agent set-value <TARGET> <VALUE>` — not
`--id`. (`assert --id` is a different command.)

### Probe

```bash
gpui-agent hello
```

Check:

- `platform` — `desktop` (painted `bir`) vs `headless` (`bir-headless`)
- `ready`
- `auth` — `"required"` when the host has `GPUI_AGENT_TOKEN`
- `deliveries` — `["semantic"]` only

```bash
gpui-agent --addr 127.0.0.1:17421 --token dev-secret hello
```

### Profile

Never auto-create. 0 hits → `not_found`; 1 hit → set; many hits → `ambiguous`
plus candidate widgets for the human.

```bash
gpui-agent invoke profile.list
gpui-agent invoke profile.search --arg q=acme
gpui-agent invoke profile.set --arg q=acme --arg view=dashboard
```

`profile.create` only opens the editor. On a live taxpayer DB, ask the human
before `profile.save`. Do **not** run `recipes/profile-create.json` against a
Mac app-group DB.

### Forms workflow

`filing.start` → `form.fill` → `filing.validate` → `form.save_draft` →
`form.pdf`. `form.pdf` returns a **frozen HTML** absolute `path`
(`kind: "frozen-html"`). Convert to PDF **client-side**; the invoke does not
put file bytes on the result and does not run `filing.validate`.

Zero-tax 1601-C (`any_taxes_withheld=false`):

```bash
gpui-agent invoke filing.start --arg code=1601C --arg year=2026 --arg period=8
gpui-agent invoke form.fill --arg any_taxes_withheld=false
gpui-agent invoke filing.validate
gpui-agent invoke form.save_draft
gpui-agent invoke form.pdf
```

### Headless capabilities

Semantic invokes that do not need a GPU/window:

- Profiles: `profile.list` / `search` / `set` / `create` / `save` / `edit` / `tab`
- Dues / jobs: `dues.list`, `tax-dues.refresh`, `jobs.list`, `submissions.list`
- 1601-C / 2551Q: `filing.start`, `form.fill`, `form.fields`, `filing.validate`,
  `form.save_draft`, `form.pdf` (frozen HTML path)
- Daemon: `bir-headless serve` / `status` / `shutdown` (and `gpui-agent hello` /
  `shutdown`)

### Headless limits

- No GPU. Screenshot is `screenshot_unavailable`.
- `form.print` errors (`form.print needs the desktop window's frozen HTML preview`).
- No cron / FTP submission path on headless (painted `bir` starts in-process
  cron; headless does not).
- UI-only navigation/chrome may be thinner than painted (semantic tree, not
  pixels).
- **Forms Set editing is not an agent invoke today.** Agents cannot toggle
  1601C / 2551Q onto a profile via gpui-agent alone. There is **no**
  `profile.forms_set`. Use the painted Profile Manager Forms Set UI, or a
  deliberate offline DB helper. Do not confuse this with `dashboard.set_forms`,
  which only filters dashboard chips / host tree.

### Linux / box demo DB

Prefer a **fresh** `BIR_DATABASE_PATH` for demos. Set `EBIR_TEST_ENV` **only**
when you intentionally want the test zero SQLCipher key (same as unit tests).
The live default path uses the OS keyring / keychain key.

Mixing keyring vs `EBIR_TEST_ENV` keys on the **same file** makes the DB
unreadable (`file is not a database` / SQLCipher `NotADatabase`).
`bir-headless` uses `Database::open` and will **not** quarantine or recreate
that file. Painted `bir` uses `open_or_recreate` and may quarantine a bad
open — do not “fix” a live taxpayer file that way.

```bash
export GPUI_AGENT=1
export GPUI_AGENT_TOKEN=dev-secret
export GPUI_AGENT_ADDR=127.0.0.1:17421
export BIR_DATABASE_PATH=/tmp/bir-headless-demo.db   # fresh path
export EBIR_TEST_ENV=1                               # test zero key; demo only
cargo run --locked --bin bir-headless --features agent -- serve
```

## Mac: build and run beside Grok Bot

From the repo root, debug (the usual developer loop):

```bash
export GPUI_AGENT=1
export GPUI_AGENT_TOKEN='dev-secret'
export GPUI_AGENT_ADDR='127.0.0.1:17421'
cargo run --locked --bin bir --features dev-tools,agent
```

Release (only if you intentionally want the control plane in a release binary):

```bash
export GPUI_AGENT=1
export GPUI_AGENT_ALLOW_RELEASE=1
export GPUI_AGENT_TOKEN='dev-secret'
cargo run --release --locked --bin bir --features agent
```

Install the CLI from the pinned gpui-agent repo (separate checkout):

```bash
git clone https://github.com/hexuria/gpui-agent
cd gpui-agent
git checkout 8857139af12fb033b4dd04eabd8d19b5bfc5ffc6
cargo install --path crates/gpui-agent-cli --locked
```

Smoke:

```bash
export GPUI_AGENT_ADDR='127.0.0.1:17421'
export GPUI_AGENT_TOKEN='dev-secret'
gpui-agent hello
gpui-agent snapshot
gpui-agent click global_dashboard_btn
gpui-agent assert --id page-global-dashboard
gpui-agent invoke nav.go --arg page=profile-manager
```

### Mac: bir-headless

Clap: `serve` / `status` / `shutdown`, global `--wait`. Shell matches
gpui-agent `apps/todo-headless` on pin `8857139af12fb033b4dd04eabd8d19b5bfc5ffc6`:
`from_env` + mailbox host, `PlatformKind::Headless`, loop until shutdown.

#### Smoke matrix

Status is babysitting fact, not marketing. Commands below stay copy-pasteable.
SHA `f273eec7` is `--wait` + `GPUI_AGENT_LOG_REQUESTS` on this branch.

| Mode | Status | Notes |
| --- | --- | --- |
| **A** Mac painted + gpui-agent | **working** | Painted `bir --features …,agent`. Headless not required. |
| **B** Mac GUI then `serve --wait` then quit GUI → headless owns | **PASSED** around `f273eec7` | No `Op::Yield`. `--wait` while GUI holds bind+DB; quit GUI → headless owns `17421`. `GPUI_AGENT_LOG_REQUESTS` optional. launchd KeepAlive is an optional example, not a shipped LaunchAgent. |
| **C** Mac headless-only create on live app-group DB then GUI verify | **PASSED** | Recipe below. TIN `00000000000002` / Headless Live TIN. Do **not** set `BIR_DATABASE_PATH`. Token required. |
| **D** Linux box `bir-headless serve` with temp DB | **PASSED** | `BIR_DATABASE_PATH` temp/demo (+ `EBIR_TEST_ENV` only for the test zero key). See Linux smoke D. |

#### Smoke B (Mac) — GUI owns port → headless `--wait` → quit GUI → headless owns

Painted `bir` must already be running with the same KEY=VALUE (agent on
`127.0.0.1:17421`, live app-group DB). **Quit means Cmd+Q / tray Quit /
`gpui-agent shutdown`**, not hide or the red window-close button (those keep
bind + owner lock). CLI always talks to whoever currently holds the port.

```bash
export GPUI_AGENT=1
export GPUI_AGENT_TOKEN=dev-secret
export GPUI_AGENT_ADDR=127.0.0.1:17421
# both bins: cargo build --locked --bins --features agent
# painted bir already up with the same KEY=VALUE
# optional: one stderr line per request (off by default; never logs the token)
# export GPUI_AGENT_LOG_REQUESTS=1
cargo run --locked --bin bir-headless --features agent -- serve --wait
# stderr: waiting for bind 127.0.0.1:17421 …
#         (or waiting for live DB owner lock …)
```

Quit painted `bir` (Cmd+Q). Headless should print the usual listening lines
and become owner. Other terminal:

```bash
export GPUI_AGENT_ADDR=127.0.0.1:17421
export GPUI_AGENT_TOKEN=dev-secret
gpui-agent hello
# hello.platform=headless
gpui-agent invoke profile.list
```

Optional: start painted `bir` again while headless still owns bind+DB. The
GUI must **not** dual-write — bind/lock fail and the in-process mailbox stays
off. **Shutdown headless before opening GUI** if both are installed and you
are not using wait orchestration:

```bash
cargo run --locked --bin bir-headless --features agent -- shutdown
# then open painted bir with the same KEY=VALUE
```

Without `--wait`, a busy bind still exits 2 and a busy live-DB lock exits 1
(same refuse as smoke C).

launchd KeepAlive is **not** shipped. Optional supervisor later; example
`ProgramArguments` if you add a LaunchAgent yourself:

```xml
<key>ProgramArguments</key>
<array>
  <string>/path/to/bir-headless</string>
  <string>serve</string>
  <string>--wait</string>
</array>
<key>KeepAlive</key>
<true/>
```

That plist is documentation only. It does not install an agent, does not
set the token for you, and is not required for smoke B.

#### Smoke C (Mac) — quit GUI → serve → save → shutdown → open GUI

**Daemon-only.** Quit the painted GUI first (`17421` refused is expected). Do
not leave the mailbox AgentHost running. Headless opens
`default_database_path()` once (`~/Library/Group Containers/group.dev.goldcoders.bir/bir_data.db`)
plus the same keychain SQLCipher key. `BIR_DATABASE_PATH` is CI-only; do
**not** set it here. **`GPUI_AGENT_TOKEN` is required** for this live path.
After `shutdown`, opening painted `bir` is **offline verification of the shared
file**, not live sync. There is no silent two-writer bridge. Shut the
daemon before GUI reopen.

```bash
export GPUI_AGENT=1
export GPUI_AGENT_TOKEN=dev-secret
export GPUI_AGENT_ADDR=127.0.0.1:17421
# optional: export GPUI_AGENT_LOG_REQUESTS=1
cargo run --locked --bin bir-headless --features agent -- serve
```

Other terminal (CLI from pin `8857139af12fb033b4dd04eabd8d19b5bfc5ffc6`):

```bash
export GPUI_AGENT_ADDR=127.0.0.1:17421
export GPUI_AGENT_TOKEN=dev-secret
gpui-agent hello
gpui-agent invoke profile.create
gpui-agent set-value profile-tin 00000000000002
gpui-agent set-value profile-name 'Headless Live TIN'
gpui-agent set-value profile-rdo 018
gpui-agent set-value profile-lob Retail
gpui-agent set-value profile-address Manila
gpui-agent set-value profile-zip 1000
gpui-agent set-value profile-phone 09170000000
gpui-agent set-value profile-email headless@example.com
# Uriah confirms the live-DB write; profile.save stays the explicit persist
# (profile.create does not save; profile.ensure is rejected).
gpui-agent invoke profile.save
gpui-agent invoke profile.list
gpui-agent shutdown
# or: cargo run --locked --bin bir-headless --features agent -- shutdown
```

Then open painted `bir` (no `GPUI_AGENT` required). TIN `00000000000002` must
be visible. Do **not** run `recipes/profile-create.json` against this live DB.

If `bir-headless serve` prints bind-in-use: another AgentHost already owns
`17421` — quit GUI or the other daemon, or set `GPUI_AGENT_ADDR`. Use
`serve --wait` if you want this process to resume when the owner quits.

If it prints `already open` / `LiveDatabaseInUse`: painted `bir` still has the
app-group file. Quit the GUI, or pass `--wait` (headless does not auto-yield
via a protocol op).

```bash
export GPUI_AGENT_ADDR=127.0.0.1:17421
export GPUI_AGENT_TOKEN=dev-secret
cargo run --locked --bin bir-headless --features agent -- status
```

## Linux CLI smoke (Buwiz box)

Bind is `gpui_agent::from_env` → `authorize_bind` (loopback default).
Non-loopback needs `GPUI_AGENT_REMOTE=1` **and** `GPUI_AGENT_TOKEN`. Do not
invent a second bind. `hello.auth` is `"required"` when that token is set
because the mailbox drain (painted `bir` and `bir-headless serve`) pass it into
`handle_request` (never `None` when configured).

**Display-less Linux — smoke D (temp / demo DB, PASSED):**

```bash
# Prefer a fresh file. Do not point this at a keyring-encrypted taxpayer DB.
export GPUI_AGENT=1
export GPUI_AGENT_TOKEN=dev-secret
export GPUI_AGENT_ADDR=127.0.0.1:17421
export BIR_DATABASE_PATH=/tmp/bir-headless-demo.db
export EBIR_TEST_ENV=1   # test zero key; omit if this file was created with the OS keyring
# optional: export GPUI_AGENT_LOG_REQUESTS=1
cargo run --locked --bin bir-headless --features agent -- serve
# other terminal:
gpui-agent --addr 127.0.0.1:17421 --token dev-secret hello
# hello.platform=headless
gpui-agent --addr 127.0.0.1:17421 --token dev-secret invoke profile.list
```

Mixing `EBIR_TEST_ENV` vs keyring keys on the same file makes it unreadable
(`file is not a database`). Headless will not quarantine/recreate.

**Display-less Linux — live default path** (`~/.taxman-ebir/bir_data.db`):

```bash
# Quit painted bir first so 17421 is free (connection refused today is expected
# when the GUI is not running).
export GPUI_AGENT=1
export GPUI_AGENT_TOKEN=dev-secret
export GPUI_AGENT_ADDR=127.0.0.1:17421
# optional: export GPUI_AGENT_LOG_REQUESTS=1
# default path = default_database_path() = platform::data_dir()/bir_data.db
# Token is required on this live path (same refuse as Mac smoke C).
# Do not set BIR_DATABASE_PATH. Do not set EBIR_TEST_ENV against a keyring file.
cargo run --locked --bin bir-headless --features agent -- serve
# other terminal:
gpui-agent --addr 127.0.0.1:17421 --token dev-secret hello
gpui-agent --addr 127.0.0.1:17421 --token dev-secret invoke profile.list
```

`bir-headless` opens with `Database::open` (same key as painted `bir`) and
will **not** quarantine/recreate a taxpayer file on a bad open. Checkpoint
WAL on shutdown. Headless does **not** start background cron (no FTP / no
auto-file). `screenshot_unavailable`, `virtual_unavailable`, `form.print`
errors as today. Exclusive owner lock + bind probe refuse a second process
without `--wait`. `serve --wait` polls until both are free.

**Fixture-host unit test** (ephemeral SQLite, not the taxpayer DB):

```bash
cargo test --locked -p bir-desktop --features agent \
  agent::host::tests::headless_tcp_host_serves_hello_and_nav
cargo test --locked -p bir-desktop --features agent agent::
```

**Linux with a painted window** (`DISPLAY` / Wayland). Same KEY=VALUE as Mac.
Do not run this at the same time as `bir-headless serve` on the same addr:

```bash
export GPUI_AGENT=1
export GPUI_AGENT_TOKEN=dev-secret
export GPUI_AGENT_ADDR=127.0.0.1:17421
cargo run --locked --bin bir --features agent
# other terminal:
gpui-agent --addr 127.0.0.1:17421 --token dev-secret hello
gpui-agent snapshot
```

macOS observe-only PNG of **this** window (Screen Recording), not Linux:

```bash
gpui-agent screenshot --out /tmp/bir-window.png
```

Claude Code / MCP (same token as the host):

```json
{
  "mcpServers": {
    "gpui-agent": {
      "command": "gpui-agent",
      "args": ["mcp"],
      "env": {
        "GPUI_AGENT_ADDR": "127.0.0.1:17421",
        "GPUI_AGENT_TOKEN": "dev-secret"
      }
    }
  }
}
```

## Recipes

Day-to-day AI flow (env, hello probe, profile, 1601-C, headless limits):
[AI agent playbook](#ai-agent-playbook). CLI `invoke` `--arg`s are `KEY=VALUE`;
`set-value` is positional (`<TARGET> <VALUE>`), not `--id`.

Pinned `gpui-agent` CLI `recipe validate` / `recipe run` uses a baked **todo**
schema registry. Host `invoke` names such as `nav.go` fail that registry, so
checked-in recipes use generic protocol ops only (`wait` / `click` / `set_value`
/ `assert`). BIR verbs stay available via `gpui-agent invoke`.

```bash
gpui-agent recipe run crates/bir-desktop/recipes/nav.json
gpui-agent recipe run crates/bir-desktop/recipes/dues.json
gpui-agent recipe run crates/bir-desktop/recipes/dues-upcoming.json
gpui-agent recipe run crates/bir-desktop/recipes/dues-overdue.json
gpui-agent recipe run crates/bir-desktop/recipes/jobs-list.json
gpui-agent recipe run crates/bir-desktop/recipes/profile-search-set.json \
  --set profile_row=profile-12345678900000
```

CLI `invoke` takes **repeated** `--arg KEY=VALUE`. Values are JSON if they parse
(`true`, `2026`, `{"any_taxes_withheld":false}`), otherwise strings. Do **not**
pass a single JSON-object `--arg`. `confirm=true` is boolean true, not
`"true"`. Invokes with no args omit `--arg`.

Natural-language → invoke examples (CLI, not recipe JSON):

```bash
# "open the Acme profile"
gpui-agent invoke profile.set --arg q=acme --arg view=dashboard
# 0 hits → result.status=not_found (does not create)
# many hits → result.status=ambiguous + candidates; prompt the human with widgets

# "edit this taxpayer's COR tab"
gpui-agent invoke profile.edit
gpui-agent invoke profile.tab --arg tab=cor

# "what is due this month / what is overdue"
gpui-agent invoke dues.list --arg filter=upcoming
gpui-agent invoke dues.list --arg filter=overdue --arg scope=global

# "show background jobs"
gpui-agent invoke jobs.list
gpui-agent invoke submissions.list

# "fill 1601-C from the profile then export a print document"
gpui-agent invoke filing.start --arg code=1601C --arg year=2026 --arg period=8
gpui-agent invoke form.fill --arg any_taxes_withheld=false
gpui-agent invoke form.pdf
# HTML: p1c9=08, p1c10=2026, withheld No xbox p1c20=X, amended No xbox p1c22=X

# "return a claimed queued 1601-C to editable Draft after a human confirmed nothing reached BIR"
# Unclaimed Queued: form.revert_draft with no extra args.
# Claimed Queued: same confirm gate as form.release_abandoned_claim (either invoke).
# Never call this unless Uriah confirmed no BIR filing. Still never form.file / filing.queue.
gpui-agent --addr 127.0.0.1:17421 --token dev-secret invoke form.revert_draft \
  --arg confirm=true \
  --arg reason=abandoned_no_bir_filing
# equivalent:
gpui-agent --addr 127.0.0.1:17421 --token dev-secret invoke form.release_abandoned_claim \
  --arg q=Juan \
  --arg form=1601-C \
  --arg year=2026 \
  --arg period=8 \
  --arg confirm=true \
  --arg reason=abandoned_no_bir_filing

# zero-tax 1601-C after release: set Any Taxes Withheld=No (does not file)
# Fill goes through Agent1601CHostPatch so the next snapshot / form.fields is No.
gpui-agent --addr 127.0.0.1:17421 --token dev-secret invoke form.fill \
  --arg any_taxes_withheld=false
gpui-agent invoke form.fields
gpui-agent invoke form.save_draft
gpui-agent invoke filing.validate

# "open command palette" (Cmd+K overlay). palette.search stays the query invoke.
gpui-agent invoke search.open
gpui-agent invoke palette.search --arg q=acme
```

`profile-create.json` **writes a taxpayer profile into whichever DB the host
opened**. Do **not** run it against a real Mac app-group DB. Linux CI uses a
temp file (`BIR_DATABASE_PATH`) or `Database::open` in unit tests. Host fixture
tests still use `Database::open_ephemeral()`. Live Mac smoke is the
`bir-headless serve` recipe above (TIN `00000000000002`), not this recipe.

```bash
# throwaway / empty DB only
gpui-agent recipe run crates/bir-desktop/recipes/profile-create.json
```

`form-1601c-draft.json` validates and saves a **draft**, then asserts the submit
control and confirmation node. It **does not** queue or file. Pass the due-row
id for a selected 1601-C obligation (year/period are not list indices).
Prefer `form.fill` / `form.save_draft` / `form.pdf` over scraping once a form
is open:

```bash
gpui-agent recipe run crates/bir-desktop/recipes/form-1601c-draft.json \
  --set due_id=due-1601C-2026-1
gpui-agent invoke form.fill --arg tax_14=1000.00 --arg tax_25=100.00
gpui-agent invoke form.save_draft
gpui-agent invoke form.pdf
```

Linux/cloud agents can compile and run the headless host tests. Mac painted +
`--wait` + live app-group smokes **A/B/C** are **PASSED** (see the smoke
matrix). Day-to-day AI flow: [AI agent playbook](#ai-agent-playbook).

## Invoke allow-list (BIR host only)

One canonical name per row. Match-arm synonyms are listed only in the Alias
table; do not use them in recipes.

| Name | Args | Effect |
| --- | --- | --- |
| `nav.go` | `page` | Navigate to an `ActiveView` slug (`global-dashboard`, `dashboard`, `profile-manager`, `settings`, `form-1601c`, …) |
| `profile.create` | — | Open Create Profile (does not save) |
| `profile.save` | — | Validate + `Database::save_profile` (same checks as the Save Profile button) |
| `profile.list` | — | `[{tin, name, last4, selected, archived}]` for every listed profile |
| `profile.search` | `q` | Same shape as list. TIN substring or name, case-insensitive. Empty `q` lists all. Does not create |
| `profile.set` | `tin` **or** `q`; optional `view`=`dashboard` (default) or `profile-manager` | 0 hits → `{status:"not_found"}` (does **not** create). 1 hit → select (same lock/PIN/dirty gates as `profile-*`). Many hits → `{status:"ambiguous", candidates}` and no auto-select |
| `profile.edit` | optional `tin` | Select current or given TIN and stay on Profile Manager with editor fields loaded |
| `profile.tab` | `tab`=`tax`\|`cor`\|`email`\|`export`\|`calendar`\|`security` | Switch Profile Manager tab. Drain writes `active_tab`. Calendar errors if Google Calendar is not linked |
| `dues.list` | `filter`=`upcoming` (default) \| `overdue` \| `all`; `scope`=`profile` \| `global` (default: profile if a TIN is selected, else global) | Profile: selected taxpayer obligations. Global: BIR tax calendar deadlines whose **final date falls in the current local calendar month**. Date basis: `chrono::Local::now().date_naive()`. Status is date vs today, not weekend/holiday `DeadlineStatus` |
| `tax-dues.refresh` | — | Reload unfiltered dues for the selected profile |
| `jobs.list` | optional `status` | Read-only `Database::list_jobs` |
| `submissions.list` | optional `tin`, `status` | Queued/submitted draft summaries plus `list_submissions_for_tin` |
| `search.open` | — | Open the Command Palette overlay (`overlay-command-palette`). Same as Cmd+K / Ctrl+K. Does **not** select, create, or run a query |
| `palette.search` | `q` | Same ranking as Command Palette (shared `search_profiles_for_palette`). `{matches, can_create, create_query?}`. Does **not** create and does **not** require the overlay to be open |
| `dashboard.set_forms` | `forms`=`all` or codes | Agent-side form filter + `dashboard-form-filter` / `dashboard-form-chip-*`. **Global Dashboard UI has no form combobox**; this is host/tree + profile `FilterBar` chips |
| `dashboard.filter` | `q` | Text filter (`dashboard-filter-query`). Profile dashboard FilterBar search; Global Dashboard has no search box |
| `filing.start` | `code`, `year`, `period` | Open a form for the selected profile |
| `filing.validate` | — | Run `FormValidator` for open 1601-C or 2551Q |
| `form.fields` | — | Required/optional fields, current values, `profile_defaulted` / `fillable` for the open 1601-C or 2551Q. `status` / `claimed` / `id` come from the `form_drafts` row (same id as `submissions.list`), not a stale in-memory Draft |
| `form.fill` | `fields` object and/or fillable KEY=VALUE args | Set only provided fillable keys; refuse unknown. 1601-C: `tax_14`, `tax_25`, `sheets`, **`any_taxes_withheld`** (boolean `true`/`false` or `Yes`/`No`; aliases `withheld_btn`, `form-1601c-withheld`). Live window drain applies withheld through `Agent1601CHostPatch` so the painted Yes/No and the next snapshot/`form.fields`/`filing.validate` match. 2551Q: `creditable_tax_withheld`, `other_tax_credit`, `taxable_amount` — 2551Q has **no** Any Taxes Withheld Yes/No control. Does not queue or file |
| `form.save_draft` | — | Persist a 1601-C or 2551Q **draft** |
| `form.pdf` | — | Real `bir_print::frozen_html::filled_document` pipeline to a temp `index.html` (TIN stamps, writer-cell identity including email, 1601-C For the Month `txtMonth`/`txtYear` on `p1c9`/`p1c10`, Amended/Withheld `xbox_joins`, 2551Q header period, demo tax `money_joins`). Writer-cell letter combs ASCII-uppercase for BIR CAPITAL LETTERS; money/digits/xbox and profile DB values are unchanged. Does **not** run `filing.validate` and does not refuse on validation errors. Returns `{path, kind:"frozen-html"}` with an **absolute** `path`. Does **not** put file bytes on the invoke result |
| `form.print` | optional `copies` (ignored; preview has no copies API) | Desktop: flags the existing frozen HTML preview. Headless: error. Never queues filing |
| `form.revert_draft` | Unclaimed: no args. Claimed Queued: `confirm` boolean `true` + `reason`=`abandoned_no_bir_filing` (same gate as `form.release_abandoned_claim`) | Unclaimed **Queued** → **Draft** via cancel CAS. Claimed **Queued** with unresolved outcome → **Draft** via the abandoned-claim CAS (does not file). Snapshot `form-1601c-status` stays `Queued` + `claimed` until that CAS succeeds. Submitted/Confirmed/Paid refuse. Agent click on `form-1601c-return-draft` does **not** skip the confirm args |
| `form.release_abandoned_claim` | `confirm` must be boolean `true`; `reason`=`abandoned_no_bir_filing`; `tin` **or** `q` (same as `profile.set`, refuse ambiguous); `form` or `code` (`1601C` / `1601-C` / `2551Q`); `year` + `period` as `filing.start` (1601-C month, 2551Q quarter). Open 1601C/2551Q can supply form/period if omitted | Same claimed **Queued** → **Draft** CAS as confirmed `form.revert_draft`. Clears claim token/`claimed_at` and the pending-retry error; stores the release reason on the draft. Does **not** queue or file. Unclaimed queues stay on `form.revert_draft` without confirm. Submitted/Confirmed/Paid refuse. Already-Draft / no row is idempotent `{released:false}`. **Never** use unless a human confirmed nothing reached BIR |
| `form.mark_paid` | — | 1601-C: `{status:"unsupported"}` (UI does not actually mark paid). 2551Q: only from Confirmed via `save_paid_2551q_draft` |
| `form.upload_receipt` | — | `{status:"needs_file", path:null}` — file picker required. A later success must return an absolute `path`, never file bytes |
| `calendar.add` | — | Writes a native `.ics` via `build_desired_events` + `write_profile_calendar_ics` to a temp path. Does not open a calendar app |
| `profile.calendar_sync` | — | **Error**: Google push needs a linked account and the Profile Manager calendar tab |
| `filing.submit` | — | Validate and expose confirmation; **does not queue or file** |
| `form.submit` / `form.queue` / `form.file` / `filing.queue` / `filing.file` | — | **Rejected** |
| `profile.ensure` | — | **Rejected**. The host will not auto-write a taxpayer. `profile.create` only opens the editor; the outer agent asks Uriah before `profile.save` |

## Alias table

Match-arm synonyms for the same handler. Allow-list and recipes use the
**Canonical** name only so spellings do not drift.

| Alias (match-arm only) | Canonical |
| --- | --- |
| `form.preview_pdf` | `form.pdf` |
| `draft.revert` | `form.revert_draft` |
| `payment.mark_paid` | `form.mark_paid` |
| `receipt.upload` | `form.upload_receipt` |
| `palette.open` | `search.open` |
| `profile.new` | `profile.create` |
| `form.open` | `filing.start` |
| `form.validate` | `filing.validate` |

`palette.search` is **not** an alias of `search.open`. Open is Cmd+K / Ctrl+K;
search is the query invoke.

Not implemented (on purpose):

- `profile.ensure` — no auto-create / upsert. Opening the editor is `profile.create`. Persisting is `profile.save` after human confirmation.
- `profile.forms_set` — **does not exist.** Yearly Forms Set include/exclude is the painted Profile Manager UI (or a deliberate offline DB helper), not a gpui-agent invoke. `dashboard.set_forms` only filters dashboard chips.

## Stable IDs

See `src/agent/ids.rs`. Page roots are `page-*`. Sidebar nav IDs match existing
widget ids (`global_dashboard_btn`, `settings_sidebar_btn`, …). Profile rows are
`profile-{tin}` (TIN digits only; `profile-tab-*` is never parsed as a TIN).
Selected profile is the checked `profile-{tin}` listitem plus
`context.selected_tin` (full TIN in `value`). Command Palette overlay is
`overlay-command-palette`. Dues rows are `due-{form}-{year}-{period}`
with overdue/upcoming in `states` and the row name. Jobs are `job-{id}` and
submissions `submission-{id}` under `page-cron-tasks`. Profile Manager tabs:
`profile-tab-tax` … `profile-tab-calendar` and `profile-section-*`. Dashboard
filter: `dashboard-form-filter`, `dashboard-filter-query`,
`dashboard-form-chip-{code}`. 1601-C: `page-form-1601c`, `save_draft_btn`,
`submit_btn`, `form-1601c-tax-14`, `form-1601c-tax-25`,
`form-1601c-sheets`, **`withheld_btn`** (Any Taxes Withheld Yes/No; role
`checkbox`, `checked` plus `value` `Yes`/`No`; Draft-only),
`form-1601c-submit-confirm`, `cancel_queue_btn` (unclaimed Queued),
`form-1601c-return-draft` / `form-1601c-release-claim-confirm` (claimed Queued;
confirm click is disabled for the agent). `form-1601c-status` `value` is the
real `form_drafts` `FilingStatus` (`Queued` while claimed; never `Draft` until the release
CAS). Claimed queues add `claimed` and `outcome-pending` in `states`. 2551Q fillables: `form-2551q-creditable`,
`form-2551q-other-credit`, `form-2551q-taxable-0`. Item 14/25 must be > 0
when Any Taxes Withheld is YES; set `any_taxes_withheld=false` for zero-tax.

## Coverage (this slice)

Proven in headless host tests (not a Mac GUI run):

- Navigation: all 18 `ActiveView` page roots via `nav.go` + assert
- Admin / lock gates are not bypassed
- Profile create on an ephemeral DB, row visible and selected
- `profile.search` / `profile.set` not_found and ambiguous; `profile.edit` stays
  on Profile Manager with editor populated; `profile.tab` switches sections
- Fixture 1601-C due rows with stable ids; `dues.list` upcoming/overdue/all
- `jobs.list` read-only; `palette.search` can_create without creating;
  `search.open` exposes `overlay-command-palette` without creating a profile
- 1601-C draft save + validate + confirmation node; status stays `Draft`;
  `form.submit` / `filing.queue` rejected
- `form.fill` refuses unknown keys; `any_taxes_withheld` false/true (or
  Yes/No) updates `withheld_btn` snapshot `checked`/`value`. Desktop drain
  writes that flag through `Agent1601CHostPatch` so the next snapshot /
  `form.fields` / `filing.validate` see No without a human click. `form.pdf` writes frozen HTML to an
  absolute `path` via the real print pipeline (no bytes on the result) and
  does **not** call `filing.validate`. 1601-C Aug 2026 / withheld=No /
  amended=No fills month `08`, year `2026`, and the No xboxes. A Confirmed 2551Q can still have
  draft validation errors (stale `profile_snapshot`, missing
  `annual_income_tax_election` / `taxpayer_type` / `item_13_election`, stale
  surcharge/interest/compromise); those are orthogonal to print fill.
  headless `form.print` errors; `form.upload_receipt` is `needs_file` with
  `path: null`; 2551Q `form.save_draft` is mapped
- `form.release_abandoned_claim` is confirm-gated (`confirm` boolean true +
  `reason=abandoned_no_bir_filing`). Claimed queued 1601-C `form.revert_draft`
  without confirm refuses (same gate). With confirm, either invoke returns Draft
  and clears the claim. It does not file. Unclaimed Queued still cancels
  through `form.revert_draft` with no extra args.
- UI 1601-C / 2551Q Queued: **Cancel Queue** for unclaimed; **Return to Draft**
  then **Confirm nothing reached BIR** for claimed. Snapshot/status stay
  Queued until the CAS writes Draft.
- `profile.create` opens the editor and does not save; `profile.ensure` is
  rejected (no auto-write)
- Selected profile: checked `profile-{tin}` listitem and `context.selected_tin`
- Remaining form views: page root + back/save/submit chrome ids.
  Semantic **save** besides 1601-C and 2551Q is not mapped
- `hello.auth` is `Required` when `handle_request` is given a configured token,
  and `None` when it is not. `BirAgentHost::hello()` leaves `auth` at Default.
  Token mismatch still fails with `automation token required` / invalid token.
- `bir-headless serve` opens a **file-backed** SQLCipher path (`app_database_path()`,
  not ephemeral). `profile.save` is visible after reopen. Bind-in-use and
  live-DB owner lock are refused without `--wait`; `serve --wait` resumes after
  the owner releases both. Headless does not start cron. GUI-as-client
  (full ADR-001) and a shipped LaunchAgent are not this slice.

Remaining (not faked):

- Semantic save for 1701Q / 0619E / 0619F / 0605 / 2550Q / 1701 / 1702RT / 1702MX
- Global Dashboard has no combobox; `dashboard.set_forms` is host/tree (+ profile FilterBar)
- `form.print` copies are ignored (frozen HTML preview has no copies API)
- `form.mark_paid` on 1601-C is unsupported (UI message only)
- `profile.calendar_sync` (Google push) needs a linked account
- Virtual in-window delivery stays `virtual_unavailable`. Screenshot: macOS
  mailbox drain can write this window (`screencapture -l`); Linux / Windows /
  headless stay `screenshot_unavailable`.
- GUI-as-client of `bir-headless` (full ADR-001). First ship is shared
  persistence only; two AgentHosts must not share `GPUI_AGENT_ADDR`; two
  processes must not open the live DB together. Matrix B is
  `serve --wait` plus GUI quit releasing bind+lock, not a protocol op.
  launchd KeepAlive remains an optional supervisor (docs example only).
- Forms Set editing via gpui-agent (`profile.forms_set` or similar). Agents
  cannot toggle 1601C/2551Q onto a profile from the allow-list today.

## Claimed queue without BIR outcome (facts)

FTP target is hardcoded `103.56.5.254:21` `uploadOnly` (`transport.rs`). The
worker **opens** that session (connect / login / binary / CWD) **before**
claim, then claims immediately before STOR (`put_file`).

Claim writes token + `claimed_at` and `submission_error` “outcome pending /
auto-retry disabled”. Claimed rows are skipped on the next cron pass
(`queued_1601c_revision` is none). `filing.submit` only exposes confirmation; it
does **not** claim or queue.

Pre-STOR failures (XML / encrypt / revalidate / FTP connect-login-CWD) stay
unclaimed and use `record_submission_failure` / retry-or-Draft
(`process_queued_1601c_pre_store_failure_stays_unclaimed_and_can_retry`). A
TCP timeout to `:21` therefore does **not** freeze a claim. After claim, a
STOR `Err` is **fail-closed**: the worker logs unknown outcome and does **not**
clear the claim (`process_queued_1601c_unknown_outcome_remains_claimed_and_is_not_retried`).
A crash between claim and `finish_claimed_*` (including mid-STOR) leaves the
same state. July vs Aug/Jan/Feb is not a month-specific code path; whichever
period completed STOR + `finish_claimed` became Submitted. Periods that hit
a connect timeout used to claim first (old order) and freeze; they now retry.

Do **not** auto-release after STOR: clearing a claim after unknown upload I/O
can double-file. Stuck rows that already claimed (crash or STOR error, or
historical connect-then-claimed rows) still need human
`form.revert_draft` / `form.release_abandoned_claim` with `confirm=true` and
`reason=abandoned_no_bir_filing`. The FTP host is unchanged unless Uriah asks.
