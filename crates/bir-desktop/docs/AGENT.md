# gpui-agent integration (bir-desktop)

Opt-in control plane so Grok Bot (or any MCP/CLI client) can drive **eBIRForms**
beside a human: navigate pages, create a tax profile, inspect dues, and prepare
a form draft. It speaks the generic gpui-agent protocol only. BIR-specific
verbs live in this host as `invoke` names. Start at the
[AI agent playbook](#ai-agent-playbook) and
[Authorization](#authorization) if you are driving painted `bir` or
`bir-headless` from CLI / MCP / Grok Bot.

Pinned crate: [`gpui-agent`](https://github.com/hexuria/gpui-agent) commit
`45ccb94bd554d7e5c2d778952de8a53f3d9e6d1e` (`main` tip, Merge PR #33 —
protocol v2 HMAC + default-deny token). Host GPUI is **gpui-pre** through
gpui-kit 0.6. Cookbook:
[`docs/INTEGRATING.md`](https://github.com/hexuria/gpui-agent/blob/45ccb94bd554d7e5c2d778952de8a53f3d9e6d1e/docs/INTEGRATING.md)
and [`docs/SDK.md`](https://github.com/hexuria/gpui-agent/blob/45ccb94bd554d7e5c2d778952de8a53f3d9e6d1e/docs/SDK.md).
Do not fork the protocol. There is no crates.io release; git/path only.
CLI and host **must** both be on this rev (or later). A v1 CLI cannot talk
to a v2 host.

## Contents

- [Locked protocol contract](#locked-protocol-contract)
- [Two hosts, one agent port](#two-hosts-one-agent-port) (`--wait` handoff)
- [Detach, logs, and production](#detach-logs-and-production-docker-like)
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
- [Background Tasks titles](#background-tasks-titles)
- [Email Settings: shared inbox OAuth](#email-settings-shared-inbox-oauth)
- [Durable queue and auth scope](QUEUE_AUTH_SCOPE.md)

## Locked protocol contract

These are host constraints. They do not fork protocol v2.

- Depend on git/path `gpui-agent` at that rev only (no crates.io).
- `bir-core` has no GPUI / gpui-agent dependency. Painted-window and virtual
  glue stay behind bir-desktop `--features agent`.
- Loopback default via `gpui_agent::from_env` / `authorize_bind`. **A
  non-empty `GPUI_AGENT_TOKEN` is required to bind.** `GPUI_AGENT_INSECURE_NO_TOKEN=1`
  restores untokened loopback for **local demos only** (prints a banner).
  Non-loopback needs `GPUI_AGENT_REMOTE=1` and a non-empty token (SDK, not a
  BIR-invented bind). This host does not add a second bind path.
- **Two AgentHosts must not share a bind or a live SQLCipher file.** Painted
  `bir` and `bir-headless serve` both default to `127.0.0.1:17421` and take
  `bir_data.db.owner.lock`. See [Two hosts, one agent port](#two-hosts-one-agent-port).
  There is **no** protocol `Op::Yield` / `Takeover`.
- ADR-001 ([daemon SoT, GUI as protocol client](https://github.com/hexuria/gpui-agent/blob/45ccb94bd554d7e5c2d778952de8a53f3d9e6d1e/docs/ADR-001-daemon-sot.md))
  is the long-term shape. **This slice is shared persistence only:** the
  daemon opens `default_database_path()` (`platform::data_dir()/bir_data.db`
  + the same SQLCipher key). Smoke C reopen of the GUI is offline
  verification of that file, not live sync. The Mac GUI is **not** a
  protocol client of the daemon yet.
- Preferred BIR `invoke` names (app-only, not CLI/MCP verbs) include
  `nav.go`, `profile.list` / `profile.search` / `profile.set` / `profile.edit` /
  `profile.tab`, `profile.html`, `profile.forms_set.get` / `profile.forms_set`,
  `dues.list` / `dues.html`, `jobs.list`, `search.open`, `palette.search`,
  `form.fill` / `form.pdf`, plus the original `profile.create`,
  `tax-dues.refresh`, `filing.start`, `filing.validate`, `filing.submit`,
  and `form.release_abandoned_claim`.
  `filing.submit` maps to the existing confirmation gate; it does not queue or
  file. Match-arm synonyms (see the Alias table) are not a second allow-list.
  There is **no** `profile.ensure` auto-write; `profile.create` only opens the
  editor. `profile.forms_set` writes only with `confirm=true` (JSON boolean).
- Semantic delivery is the supported path. Virtual ops return
  `virtual_unavailable` rather than synthesizing OS HID or a half-wired
  in-window pointer. Protocol is unchanged.
- Screenshot is observe-only. The UI-thread mailbox drain intercepts
  `Op::Screenshot`: it **confines** the client path (`require_screenshot_path`
  then `confine_screenshot_path`) **before** capture or
  `screenshot_unavailable`. Clients send a **relative `.png` name**, not an
  absolute path. Host writes under `GPUI_AGENT_SCREENSHOT_DIR` (default
  `{temp_dir}/gpui-agent-screenshots/`). macOS then writes **this** window via
  `capture_window_via_screencapture` (`screencapture -l`, Screen Recording).
  Linux, Windows, and headless `spawn_host` stay `screenshot_unavailable`.
  The semantic host has no `Window` and does not invent a PNG.
- Filing status SoT is the `form_drafts` row for that TIN/year/month (the same
  queued id `submissions.list` shows). Snapshot / `form.fields` /
  `form-1601c-status` overlay that row; a stale local Draft cannot mask
  Queued+claimed until the release CAS writes Draft.

## Two hosts, one agent port

> Cmd+Q (Ctrl+Q / Alt+F4 on Linux and Windows) and the app menu's Quit **hide
> painted `bir` to the tray**; the cron keeps polling and the agent bind and
> live-DB owner lock stay held. The only real quit is the tray menu's Quit or
> `gpui-agent shutdown`.

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
`bir-headless shutdown` (or kill) then open painted `bir`. If the maintainer later
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
- `bir-headless serve --detach` (also `bir-headless --detach`) applies the
  same rules, then backgrounds the child. A second `--detach` while a live
  pid or bind/lock is held **fails immediately** (exit 2). `--detach --wait`
  starts a background waiter that takes over after the owner releases both.
- Clean GUI **Quit** (tray menu Quit / `gpui-agent shutdown`) releases bind
  + owner lock. Hiding or closing the window does **not**.

```bash
# GUI already up on 17421 with the live app-group DB
cargo run --locked --bin bir-headless --features agent -- serve --wait
# quit painted bir (tray menu → Quit) → headless owns 17421
# after headless work:
cargo run --locked --bin bir-headless --features agent -- shutdown
# then open painted bir
```

## Detach, logs, and production (Docker-like)

Foreground `serve` still occupies the Terminal. Production and lab VMs can
run without one, then attach to logs the same way as Docker:

| Docker | bir-headless |
| --- | --- |
| `docker run -d …` | `bir-headless serve --detach` |
| `docker logs` | `bir-headless logs` |
| `docker logs -f` / `tail -f` | `bir-headless logs --follow` (`-f`) |
| `docker logs --tail N` | `bir-headless logs --tail N` |
| `docker ps` (is it up?) | `bir-headless status` |
| `docker stop` | `bir-headless shutdown` |

```bash
export GPUI_AGENT=1
export GPUI_AGENT_TOKEN=dev-secret
export GPUI_AGENT_ADDR=127.0.0.1:17421
# optional: BIR_DATABASE_PATH=/tmp/bir-headless-demo.db  (CI / demo only)

cargo run --locked --bin bir-headless --features agent -- serve --detach
# stdout:
#   pid=12345
#   log=/home/you/.taxman-ebir/logs/bir-headless.log
# parent returns; no Terminal needs to stay open

cargo run --locked --bin bir-headless --features agent -- status
cargo run --locked --bin bir-headless --features agent -- logs --tail 50
cargo run --locked --bin bir-headless --features agent -- logs --follow
# Ctrl-C leaves the daemon running
cargo run --locked --bin bir-headless --features agent -- shutdown
```

**Log path** (created automatically; append-only, no rotation in this slice):

| Host | Default logfile |
| --- | --- |
| Linux | `~/.taxman-ebir/logs/bir-headless.log` |
| Mac | `~/Library/Group Containers/group.dev.goldcoders.bir/logs/bir-headless.log` |
| `BIR_DATABASE_PATH` set | `{parent-of-db}/logs/bir-headless.log` |
| override | `BIR_HEADLESS_LOG` |

Pid file: `…/bir-headless.pid` next to the live DB override, or
`platform::data_dir()/bir-headless.pid` on the default path (`BIR_HEADLESS_PID`
overrides). Stdout/stderr of the detached child (including
`GPUI_AGENT_LOG_REQUESTS`) go to that log. `logs` without `--follow` dumps the
file; `--follow` dumps existing lines then tails. Missing log → exit 1 with
a path in the error. Ctrl-C on `--follow` exits the follower only.

**Single instance.** Same exclusivity as foreground `serve`: TCP bind +
live-DB owner lock. `--detach` also refuses a live pid file. A second
`serve --detach` while the daemon is running fails clearly. `--detach --wait`
does **not** fail: it backgrounds a waiter (same as foreground `--wait`).
`--detach` is invalid on `status` / `logs` / `shutdown` (exit 2).

**Happy path by environment**

| Where | How |
| --- | --- |
| Linux box / lab VM / no GUI | `serve --detach` then `logs --follow`. This is enough. |
| Mac lab (GUI closed, no Terminal) | same `--detach` |
| **Mac production** | launchd KeepAlive. launchd is the supervisor — **do not** pass `--detach` (KeepAlive would see the parent exit and restart). `--wait` in ProgramArguments so a painted `bir` can own the port during the day. Stdout/err to the **same** logfile `logs` reads. |

Example plist (docs only, not a shipped LaunchAgent):
[`bir-headless.launchd.plist.example`](bir-headless.launchd.plist.example).

```xml
<key>ProgramArguments</key>
<array>
  <string>/path/to/bir-headless</string>
  <string>serve</string>
  <string>--wait</string>
</array>
<key>KeepAlive</key>
<true/>
<key>StandardOutPath</key>
<string>/Users/YOU/Library/Group Containers/group.dev.goldcoders.bir/logs/bir-headless.log</string>
<key>StandardErrorPath</key>
<string>/Users/YOU/Library/Group Containers/group.dev.goldcoders.bir/logs/bir-headless.log</string>
```

Set `GPUI_AGENT=1` in `EnvironmentVariables`. Put `GPUI_AGENT_TOKEN` in a
root-restricted extras plist or `launchctl setenv` — never in git. launchd
does not expand `~`; StandardOut/Err must be absolute.

Foreground `serve` / `serve --wait` / `status` / `shutdown` are unchanged.

## Authorization

This is the token model AI agents must follow. Never print or log the token.

| Gate | Rule |
| --- | --- |
| Feature | `agent` is **off** by default. Product/release builds must leave it off. |
| Opt-in | Runtime starts only when `GPUI_AGENT=1` (`true`/`yes`/`on`). |
| Release | Release binaries also need `GPUI_AGENT_ALLOW_RELEASE=1`. |
| Bind | Loopback `127.0.0.1:17421` unless `GPUI_AGENT_ADDR` is set. `from_env` calls `authorize_bind_with_insecure`. **Token required to bind** on loopback. Non-loopback needs `GPUI_AGENT_REMOTE=1` **and** a non-empty token. Transport is still plaintext TCP. Do not invent a second bind path. |
| Shared secret | **`GPUI_AGENT_TOKEN` is required** on painted `bir` and `bir-headless serve` (local smokes use `dev-secret`). CLI / MCP / Grok Bot must set the **same** value. The wire field is protocol **v2 `auth`** = hex(`HMAC-SHA256(token, per-connection nonce)`). Never send the raw token on NDJSON. v1 CLI / raw `token` field fail closed (`token must not be sent on the wire`). |
| Insecure demo | `GPUI_AGENT_INSECURE_NO_TOKEN=1` binds loopback without a token and prints a loud banner. **Demos only.** Never on a shared machine, agent VM, or live taxpayer DB. Recipe `run` and `mcp` still require a client token. |
| Live default DB | **`bir-headless serve` requires a token** against live `default_database_path()` (Mac app-group `~/Library/Group Containers/group.dev.goldcoders.bir/bir_data.db`, Linux `~/.taxman-ebir/bir_data.db`). This refuse still applies if someone sets `GPUI_AGENT_INSECURE_NO_TOKEN=1`. |
| Path override | `BIR_DATABASE_PATH` is for CI / temp demos. A **non-empty** override may omit the *headless live-path* refuse (`live_database_token_required` is false), but **`from_env` still requires `GPUI_AGENT_TOKEN` unless `GPUI_AGENT_INSECURE_NO_TOKEN=1`**. Prefer still setting a token so recipe/MCP clients match. Do **not** set `BIR_DATABASE_PATH` for live Mac smokes. |
| Painted `bir` | Bind is `from_env` alone (no extra live-path refuse). Without `GPUI_AGENT_TOKEN` (and without the insecure flag) the mailbox **does not start**. Recipes and MCP need the same token on host and client. |
| `hello.auth` | `"required"` when a token is configured on the host, `"none"` only for the insecure/no-token demo. The TCP thread enforces HMAC and stamps `hello.auth` on the mailbox path. `BirAgentHost::hello()` does not set `auth` by hand. |
| Protocol | **v2.** After accept the host writes a challenge nonce. Clients (`gpui-agent` CLI / `AgentClient::with_token`) send `auth` HMAC. Host and CLI must both be ≥ pin `45ccb94`. |
| Logging | **Never log the token.** Opt-in `GPUI_AGENT_LOG_REQUESTS=1` (`true`/`yes`/`on`) emits one stderr line per request: `timestamp gpui-agent id=… op=hello\|invoke\|… name=profile.list ok=true`. Off by default. Not enabled by `RUST_LOG`. Invoke args, `set_value` values, typed text, screenshot paths, tokens, and HMAC hex are never included. Same helper on the painted mailbox drain. |

## Security gates

These still apply to every agent, painted or headless:

- No auto `profile.ensure` (rejected). `profile.create` opens the editor only.
  `profile.save` is the explicit persist after human confirm for live
  taxpayers.
- Queue 1601C/2551Q with filing.queue / form.queue / form.submit and confirm=true (JSON boolean). filing.submit only exposes the confirmation node. Direct click on form-1601c-submit-confirm is refused so that gate cannot be skipped. Cron then PUTs the queued return.
  form.release_abandoned_claim` only returns a claimed Queued snapshot to Draft
  after a human confirmed nothing reached BIR; it does **not** file.
- Never skip the lock screen, profile PIN/TOTP, or administrator OTP. Unsaved
  profile compliance still blocks navigation.
- No virtual HID on this host. `hello.deliveries` is `["semantic"]`. Virtual
  `click` / `type` / `key` return `virtual_unavailable`. Do not point agents at
  `--delivery virtual`.
- Snapshots include names, last-4 TIN, dues, and editor fields needed to drive
  flows. They do not include PIN hashes, TOTP secrets, or keychain material.

## Three ways to queue a 1601C/2551Q

All three end on the same in-process cron PUT. The GUI does not need to be open
for Grok Bot or bir-headless.

1. Grok Bot / gpui-agent against bir-headless (no painted window).
   Start bir-headless with GPUI_AGENT=1 and the same token the bot uses.
   Then: profile.set tin=00000000000000, filing.start code=1601C year period,
   form.fill, filing.queue confirm=true. filing.queue can also take tin/code/year/period itself.
2. Desktop confirm button while painted bir is open. Cron still needs the app
   or bir-headless running to PUT.
3. Cron on an already-queued row. Queue first via 1 or 2; cron does not invent
   a queue by itself.

Durable worker behavior (lease, 1601-C IMAP confirm, IMAP skip from session
source) is in [QUEUE_AUTH_SCOPE.md](QUEUE_AUTH_SCOPE.md).
It does not add a fourth enqueue route.

Grok Bot talks to whoever owns GPUI_AGENT_ADDR (painted bir or bir-headless).
Two hosts cannot share the bind or the live DB.

## AI agent playbook

Use this section as the day-to-day recipe. Invoke names must match the
[allow-list](#invoke-allow-list-bir-host-only). Do not invent verbs
(`Op::Yield`, `filing.queue`, …). `profile.forms_set` is allow-listed
(confirm-gated). Do not send client HTML or file bytes on invoke.

### Client env

```bash
export GPUI_AGENT_ADDR=127.0.0.1:17421
export GPUI_AGENT_TOKEN=dev-secret   # required; must match the host
```

Host (whichever owns the port):

```bash
export GPUI_AGENT=1
export GPUI_AGENT_TOKEN=dev-secret   # required to bind (default-deny)
export GPUI_AGENT_ADDR=127.0.0.1:17421
# optional: export GPUI_AGENT_LOG_REQUESTS=1
# demo-only (never live DB): export GPUI_AGENT_INSECURE_NO_TOKEN=1
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
- `auth` — `"required"` when the host has `GPUI_AGENT_TOKEN` (`"none"` only with `GPUI_AGENT_INSECURE_NO_TOKEN=1`)
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

Read-only identity card (host-written demo HTML, **not** `html-frozen/` print
geometry). `tin` and/or `q` resolve like `profile.set`; omit both to use the
selected profile. Optional `year` shows that year's Forms Set codes.
Result is `{path, kind:"html"}` — an **absolute** temp `index.html` (plus
`theme.css` / fonts). Never file bytes, never client-supplied HTML.

```bash
gpui-agent invoke profile.html --arg tin=00000000000002
gpui-agent invoke profile.html --arg q='Headless Live TIN' --arg year=2026
```

Theme tokens live in repo `html-demo/theme.css` and are copied next to the
temp `index.html`. `form.pdf` still uses `html-frozen/` cell geometry
(`kind: "frozen-html"`). Do not mix the two pipelines.

### Forms Set

Read with `profile.forms_set.get`. Write with `profile.forms_set` (alias
`profile.forms_set.set`). Write requires **`confirm=true` as a JSON
boolean** — string `"true"` is refused. Unknown registry codes refuse.
Source is Manual. One write path: `Database::save_per_year_forms` →
`execute_replace_per_year_forms`. No `profile.ensure`. No unconfirmed live
writes.

```bash
gpui-agent invoke profile.forms_set.get --arg tin=00000000000002 --arg year=2026
gpui-agent invoke profile.forms_set --arg year=2026 --arg codes=1601C,2551Q --arg confirm=true
```

`--arg confirm=true` JSON-parses as boolean. Optional `--arg reason='…'`.

### Dues HTML

Same engine as `dues.list` (no second calendar). Optional `tin`/`q`; `filter`
or `scope` `upcoming`|`overdue`|`all`; `scope` `profile`|`global` still
selects list scope; optional `limit`. Alias `calendar.html`. Same
`{path, kind:"html"}` bundle as `profile.html`.

```bash
gpui-agent invoke dues.html --arg filter=upcoming --arg limit=20
gpui-agent invoke calendar.html --arg scope=upcoming
```

### Forms workflow

`filing.start` → `form.fill` → `filing.validate` → `form.save_draft` →
`form.pdf`. `form.pdf` returns a **frozen HTML** absolute `path`
(`kind: "frozen-html"`). Convert to PDF **client-side**; the invoke does not
put file bytes on the result and does not run `filing.validate`.

Zero-tax 1601-C (`any_taxes_withheld=false`):

```bash
gpui-agent invoke filing.start --arg code=1601C --arg year=2026 --arg period=8
gpui-agent invoke form.fill --arg any_taxes_withheld=false
# Item 11 defaults to Private (`P`). Government:
# gpui-agent invoke form.fill --arg category_of_agent=G
gpui-agent invoke filing.validate
gpui-agent invoke form.save_draft
gpui-agent invoke form.pdf
```

### Headless capabilities

Semantic invokes that do not need a GPU/window:

- Profiles: `profile.list` / `search` / `set` / `create` / `save` / `edit` / `tab`
- Demo HTML: `profile.html`, `dues.html` (`calendar.html`)
- Forms Set: `profile.forms_set.get`, `profile.forms_set` (confirm-gated write)
- Dues / jobs: `dues.list`, `tax-dues.refresh`, `jobs.list`, `submissions.list`
- 1601-C / 2551Q: `filing.start`, `form.fill`, `form.fields`, `filing.validate`,
  `form.save_draft`, `form.pdf` (frozen HTML path)
- Daemon: `bir-headless serve` / `status` / `shutdown` (and `gpui-agent hello` /
  `shutdown`)

### Headless limits

- No GPU. Screenshot is `screenshot_unavailable`.
- `form.print` errors (`form.print needs the desktop window's frozen HTML preview`).
- Headless now starts the same in-process SFTP submission cron as painted `bir`.
- UI-only navigation/chrome may be thinner than painted (semantic tree, not
  pixels).
- Demo HTML (`profile.html` / `dues.html`) is a host-written temp bundle using
  `html-demo/theme.css`. It is **not** `form.pdf` / `html-frozen/` print
  geometry and does not put file bytes on the invoke result.

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
export GPUI_AGENT_TOKEN=dev-secret   # required to bind
export GPUI_AGENT_ADDR=127.0.0.1:17421
export BIR_DATABASE_PATH=/tmp/bir-headless-demo.db   # fresh path
export EBIR_TEST_ENV=1                               # test zero key; demo only
# do not set GPUI_AGENT_INSECURE_NO_TOKEN=1 here
cargo run --locked --bin bir-headless --features agent -- serve
```

## Mac: build and run beside Grok Bot

From the repo root, debug (the usual developer loop):

```bash
export GPUI_AGENT=1
export GPUI_AGENT_TOKEN='dev-secret'   # required to bind
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

Install the CLI from the pinned gpui-agent repo (separate checkout). **v1
CLI cannot speak v2 HMAC** — install this rev, not an older `main`:

```bash
git clone https://github.com/hexuria/gpui-agent
cd gpui-agent
git checkout 45ccb94bd554d7e5c2d778952de8a53f3d9e6d1e
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

Clap: `serve` / `status` / `shutdown` / `logs`, global `--wait` and `--detach`.
Shell matches gpui-agent `apps/todo-headless` on pin `45ccb94bd554d7e5c2d778952de8a53f3d9e6d1e`:
`from_env` + mailbox host, `PlatformKind::Headless`, loop until shutdown.
`GPUI_AGENT_TOKEN` is required to bind. `--detach` / `logs --follow` are the
Docker-like lab path; Mac production uses the launchd example (no `--detach`).

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
`127.0.0.1:17421`, live app-group DB). **Quit means tray Quit /
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

Quit painted `bir` (tray menu → Quit). Headless should print the usual listening lines
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
(same refuse as smoke C). `serve --detach` uses the same refuse, then
backgrounds; a second `--detach` while running fails. See
[Detach, logs, and production](#detach-logs-and-production-docker-like).

launchd KeepAlive is **not** shipped. Mac production example (KeepAlive +
StandardOut/Err to the same path `bir-headless logs` follows) lives in
[`bir-headless.launchd.plist.example`](bir-headless.launchd.plist.example).
**Do not** put `--detach` in ProgramArguments. Lab/Linux can skip launchd and
use `serve --detach`.

```xml
<key>ProgramArguments</key>
<array>
  <string>/path/to/bir-headless</string>
  <string>serve</string>
  <string>--wait</string>
</array>
<key>KeepAlive</key>
<true/>
<key>StandardOutPath</key>
<string>/Users/YOU/Library/Group Containers/group.dev.goldcoders.bir/logs/bir-headless.log</string>
<key>StandardErrorPath</key>
<string>/Users/YOU/Library/Group Containers/group.dev.goldcoders.bir/logs/bir-headless.log</string>
```

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

Other terminal (CLI from pin `45ccb94bd554d7e5c2d778952de8a53f3d9e6d1e`):

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
# The maintainer confirms the live-DB write; profile.save stays the explicit persist
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
**`GPUI_AGENT_TOKEN` is required to bind** unless `GPUI_AGENT_INSECURE_NO_TOKEN=1`.
Non-loopback needs `GPUI_AGENT_REMOTE=1` **and** `GPUI_AGENT_TOKEN`. Do not
invent a second bind. `hello.auth` is `"required"` when that token is set
because the TCP thread stamps it (never `None` when configured).

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
# no Terminal (Linux lab happy path):
# cargo run --locked --bin bir-headless --features agent -- serve --detach
# cargo run --locked --bin bir-headless --features agent -- logs --follow
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
WAL on shutdown. Headless **does** start background cron (dispatcher-backed SFTP PUT). `screenshot_unavailable`, `virtual_unavailable`, `form.print`
errors as today. Exclusive owner lock + bind probe refuse a second process
without `--wait`. `serve --wait` polls until both are free. Linux production
without a Terminal is `serve --detach` then `logs --follow` (same log path as
above). Do not invent a second agent port.

**Fixture-host unit test** (ephemeral SQLite, not the taxpayer DB):

```bash
cargo test --locked -p bir-desktop --features agent \
  agent::host::tests::headless_tcp_host_serves_hello_and_nav
cargo test --locked -p bir-desktop --features agent agent::
cargo test --locked -p bir-desktop --features agent --test headless_detach
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

macOS observe-only PNG of **this** window (Screen Recording), not Linux.
Client path is a **relative `.png` name** (confined under
`GPUI_AGENT_SCREENSHOT_DIR` or `{temp}/gpui-agent-screenshots/`):

```bash
gpui-agent screenshot --out bir-window.png
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

Pinned `gpui-agent` CLI `recipe validate` / `recipe run` uses a **protocol-only**
schema registry by default (no baked todo invokes). Host `invoke` names such
as `nav.go` fail that registry unless you pass `--schema` /
`GPUI_AGENT_SCHEMA`. Checked-in day-to-day recipes use generic protocol ops
(`wait` / `click` / `set_value` / `assert`). BIR verbs stay in this host and
are available via `gpui-agent invoke`. Do **not** add BIR verbs to
gpui-agent.

The one checked-in invoke recipe (`form-1601c-queue.json`) needs the local
schema (still host-side; not a gpui-agent verb fork):

```bash
gpui-agent recipe validate crates/bir-desktop/recipes/form-1601c-queue.json \
  --schema crates/bir-desktop/recipes/schema.json
export GPUI_AGENT_SCHEMA=crates/bir-desktop/recipes/schema.json
gpui-agent recipe run crates/bir-desktop/recipes/form-1601c-queue.json \
  --set tin=00000000000000 --set year=2026 --set period=8
```

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
# Never call this unless the maintainer confirmed no BIR filing. Still never form.file / filing.queue.
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
# Category of Withholding Agent: category_of_agent=P|G (also private/government).
gpui-agent --addr 127.0.0.1:17421 --token dev-secret invoke form.fill \
  --arg any_taxes_withheld=false --arg category_of_agent=P
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

## Filing events for an agent

Banners and the Notifications page are for a human at the Mac. Protocol v2 is
request/response only — there is no server push, and this host does not fork
the protocol to add one — so an agent driving painted `bir` **or**
`bir-headless` learns a filing's outcome by polling the same rows the
Notifications page reads:

1. Before `filing.queue`, call `alerts.list` and keep `last_id`.
2. After queueing, poll `alerts.list` with `since_id=<last_id>` and
   `kind=form_` (every few seconds is fine; it is one indexed read).
3. `form_submitted:1601C:08/26` appears when the SFTP PUT lands (title, TIN,
   local send time, mailbox, IAF filename in `detail`).
4. `form_confirmed:1601C:08/26` appears when the IMAP poll matches BIR's
   receipt email (BIR's own received date/time and file name in `detail`).
   The poll runs on the cron's schedule, so this can take an hour.
5. Painted `bir` shows the same rows as an in-app toast and, when run from a
   `.app` (`scripts/dev_bundle_macos.sh` in development), a native macOS
   notification with the app's icon; clicking either opens the Notifications
   page. `bir-headless` posts through the OS directly (AppleScript on macOS).
6. `form.fields` / `submissions.list` remain the status source of truth
   (`Queued` → `Submitted` → `Confirmed`); the alert rows are the event log.
   `alerts.dismiss` acknowledges an entry once handled.

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
| `profile.html` | `tin` **or** `q` (same resolve as `profile.set`); omit both to use the selected profile; optional `year` | Read-only identity card from the live DB. Returns `{path, kind:"html"}` with an **absolute** temp `index.html` (plus `html-demo/theme.css` + fonts). Never file bytes, never client HTML. Omits PIN/TOTP/secrets |
| `profile.forms_set.get` | `tin` **or** `q` (or selected profile); `year` (JSON number) | Read `per_year_forms` for that TIN/year. `{codes, entries, empty}` |
| `profile.forms_set` | `year`; `codes` comma list or array (e.g. `1601C,2551Q`); **`confirm` JSON boolean `true`** (string `"true"` refuses); optional `reason`; optional `tin`/`q` | Replace that year's Forms Set via `Database::save_per_year_forms` / `execute_replace_per_year_forms`. Source Manual. Unknown codes refuse. No `profile.ensure`. No unconfirmed live writes |
| `profile.edit` | optional `tin` | Select current or given TIN and stay on Profile Manager with editor fields loaded |
| `profile.tab` | `tab`=`tax`\|`cor`\|`email`\|`export`\|`calendar`\|`security` | Switch Profile Manager tab. Drain writes `active_tab`. Calendar errors if Google Calendar is not linked |
| `dues.list` | `filter`=`upcoming` (default) \| `overdue` \| `all`; `scope`=`profile` \| `global` (default: profile if a TIN is selected, else global) | Profile: selected taxpayer obligations. Global: BIR tax calendar deadlines whose **final date falls in the current local calendar month**. Date basis: `chrono::Local::now().date_naive()`. Status is date vs today, not weekend/holiday `DeadlineStatus` |
| `dues.html` | optional `tin`/`q`; `filter` or `scope`=`upcoming`\|`overdue`\|`all`; `scope`=`profile`\|`global` as in `dues.list`; optional `limit` | Same data path as `dues.list` (no second calendar engine). Returns `{path, kind:"html"}` demo bundle plus the (possibly limited) `dues` JSON |
| `tax-dues.refresh` | — | Reload unfiltered dues for the selected profile |
| `jobs.list` | optional `status` | Read-only `Database::list_jobs`. Poll `name` is the human title (`Waiting for {form} {period} confirmation for {TIN} (email?)`). `command` stays `bir_poll_email {email}`. Submit cards are UI-only and are not this list |
| `submissions.list` | optional `tin`, `status` | Queued / Submitted / Confirmed / Paid draft summaries (`list_all_filed_submissions`) plus `list_submissions_for_tin`. A return stays listed after BIR confirms it |
| `alerts.list` | optional `since_id` (JSON number), `kind` prefix, `tin` | The Notifications-page rows (`Database::list_active_alerts`), ascending by `id`, only those with `id > since_id`. `{alerts, last_id}`. Filing events are `form_submitted:{form}:{MM/YY}` and `form_confirmed:{form}:{MM/YY}` (Info, `tin` set); pass `kind=form_` to see just those. Read-only |
| `alerts.dismiss` | `id` (JSON number) | Same as the Dismiss button: `Database::dismiss_alert`. Dismissed rows leave `alerts.list` |
| `search.open` | — | Open the Command Palette overlay (`overlay-command-palette`). Same as Cmd+K / Ctrl+K. Does **not** select, create, or run a query |
| `palette.search` | `q` | Same ranking as Command Palette (shared `search_profiles_for_palette`). `{matches, can_create, create_query?}`. Does **not** create and does **not** require the overlay to be open |
| `dashboard.set_forms` | `forms`=`all` or codes | Agent-side form filter + `dashboard-form-filter` / `dashboard-form-chip-*`. **Global Dashboard UI has no form combobox**; this is host/tree + profile `FilterBar` chips |
| `dashboard.filter` | `q` | Text filter (`dashboard-filter-query`). Profile dashboard FilterBar search; Global Dashboard has no search box |
| `filing.start` | `code`, `year`, `period` | Open a form for the selected profile |
| `filing.validate` | — | Run `FormValidator` for open 1601-C or 2551Q |
| `form.fields` | — | Required/optional fields, current values, `profile_defaulted` / `fillable` for the open 1601-C or 2551Q. `status` / `claimed` / `id` come from the `form_drafts` row (same id as `submissions.list`), not a stale in-memory Draft |
| `form.fill` | `fields` object and/or fillable KEY=VALUE args | Set only provided fillable keys; refuse unknown. 1601-C: `tax_14`, `tax_25`, `sheets`, **`any_taxes_withheld`** (boolean `true`/`false` or `Yes`/`No`; aliases `withheld_btn`, `form-1601c-withheld`), **`category_of_agent`** (`P`/`G`, `private`/`government`; boolean `true`/`false` or `Yes`/`No`/`1`/`0` map to Private/Government; aliases `category_btn`, `form-1601c-category`). Live window drain applies withheld and category through `Agent1601CHostPatch` so the painted controls and the next snapshot/`form.fields`/`filing.validate` match. 2551Q: `creditable_tax_withheld`, `other_tax_credit`, `taxable_amount` — 2551Q has **no** Any Taxes Withheld Yes/No control. Does not queue or file |
| `form.save_draft` | — | Persist a 1601-C or 2551Q **draft** |
| `form.pdf` | — | Real `bir_print::frozen_html::filled_document` pipeline to a temp `index.html` (TIN stamps, writer-cell identity including email, 1601-C For the Month `txtMonth`/`txtYear` on `p1c9`/`p1c10`, Amended/Withheld `xbox_joins`, 2551Q header period, demo tax `money_joins`). Writer-cell letter combs ASCII-uppercase for BIR CAPITAL LETTERS; money/digits/xbox and profile DB values are unchanged. Does **not** run `filing.validate` and does not refuse on validation errors. Returns `{path, kind:"frozen-html"}` with an **absolute** `path`. Does **not** put file bytes on the invoke result | When the return is Confirmed the document ends with BIR's receipt confirmation page (`receipt_page: true`), as the painted Print Preview does.
| `form.print` | optional `copies` (ignored; preview has no copies API) | Desktop: flags the existing frozen HTML preview. Headless: error. Never queues filing |
| `form.revert_draft` | Unclaimed: no args. Claimed Queued: `confirm` boolean `true` + `reason`=`abandoned_no_bir_filing` (same gate as `form.release_abandoned_claim`) | Unclaimed **Queued** → **Draft** via cancel CAS. Claimed **Queued** with unresolved outcome → **Draft** via the abandoned-claim CAS (does not file). Snapshot `form-1601c-status` stays `Queued` + `claimed` until that CAS succeeds. Submitted/Confirmed/Paid refuse. Agent click on `form-1601c-return-draft` does **not** skip the confirm args |
| `form.release_abandoned_claim` | `confirm` must be boolean `true`; `reason`=`abandoned_no_bir_filing`; `tin` **or** `q` (same as `profile.set`, refuse ambiguous); `form` or `code` (`1601C` / `1601-C` / `2551Q`); `year` + `period` as `filing.start` (1601-C month, 2551Q quarter). Open 1601C/2551Q can supply form/period if omitted | Same claimed **Queued** → **Draft** CAS as confirmed `form.revert_draft`. Clears claim token/`claimed_at` and the pending-retry error; stores the release reason on the draft. Does **not** queue or file. Unclaimed queues stay on `form.revert_draft` without confirm. Submitted/Confirmed/Paid refuse. Already-Draft / no row is idempotent `{released:false}`. **Never** use unless a human confirmed nothing reached BIR |
| `form.mark_paid` | — | 1601-C: `{status:"unsupported"}` (UI does not actually mark paid). 2551Q: only from Confirmed via `save_paid_2551q_draft` |
| `form.upload_receipt` | — | `{status:"needs_file", path:null}` — file picker required. A later success must return an absolute `path`, never file bytes |
| `calendar.add` | — | Writes a native `.ics` via `build_desired_events` + `write_profile_calendar_ics` to a temp path. Does not open a calendar app |
| `profile.calendar_sync` | — | **Error**: Google push needs a linked account and the Profile Manager calendar tab |
| `filing.submit` | — | Validate and expose confirmation; does not queue until filing.queue confirm=true |
| `form.queue` / `filing.queue` / `form.submit` | `confirm=true` JSON boolean (a caller assertion that a human confirmed; the host cannot verify it) | Queue open 1601C/2551Q for cron SFTP PUT |
| `form.file` / `filing.file` / `form.submit_external` | --- | **Rejected**; queue first, then let cron PUT |
| `profile.ensure` | — | **Rejected**. The host will not auto-write a taxpayer. `profile.create` only opens the editor; the outer agent asks the maintainer before `profile.save` |

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
| `calendar.html` | `dues.html` |
| `profile.forms_set.set` | `profile.forms_set` |

`palette.search` is **not** an alias of `search.open`. Open is Cmd+K / Ctrl+K;
search is the query invoke.

Not implemented (on purpose):

- `profile.ensure` — no auto-create / upsert. Opening the editor is `profile.create`. Persisting is `profile.save` after human confirmation.

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
**`category_btn`** (Item 11 Category of Withholding Agent; role `checkbox`,
`checked` means Private, `value` `P`/`G`; Draft-only),
`form-1601c-submit-confirm`, `cancel_queue_btn` (unclaimed Queued),
`form-1601c-return-draft` / `form-1601c-release-claim-confirm` (claimed Queued;
confirm click is disabled for the agent). `form-1601c-status` `value` is the
real `form_drafts` `FilingStatus` (`Queued` while claimed; never `Draft` until the release
CAS). Claimed queues add `claimed` and `outcome-pending` in `states`. 2551Q fillables: `form-2551q-creditable`,
`form-2551q-other-credit`, `form-2551q-taxable-0`. Item 14/25 must be > 0
when Any Taxes Withheld is YES; set `any_taxes_withheld=false` for zero-tax.

### 1601-C `form.fields` / `form.fill`

| key | required | fillable | values |
| --- | --- | --- | --- |
| `tin` / `taxpayer_name` / `rdo_code` / `registered_address` | yes | no (profile) | profile defaults |
| `zip_code` / `contact_number` / `email_address` | no | no (profile) | profile defaults |
| `tax_14` | yes when withheld Yes | yes | money |
| `tax_25` | yes when withheld Yes | yes | money |
| `sheets` | no | yes | whole number |
| `any_taxes_withheld` | yes | yes | `true`/`false`, `Yes`/`No`; snapshot `Yes`/`No` |
| `category_of_agent` | yes (exactly one) | yes | **`P`** Private (new-draft default) or **`G`** Government. Also `private`/`government`. Boolean `true`/`Yes`/`1` → `P`; `false`/`No`/`0` → `G`. Snapshot value is `P` or `G`. |

Print/PDF: Item 11 CatAgent radios stay XML-only for submit (`frm1601c:CatAgent_P` / `CatAgent_G`). Frozen HTML has no xbox join for them (same as Item 13 SpecialTax); Amended/Withheld header xboxes are the joined print path.

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
- 1601-C draft save + validate + confirmation node; unconfirmed filing.queue stays Draft; filing.queue confirm=true queues; headless can pass tin/code/year/period
- `form.fill` refuses unknown keys; `any_taxes_withheld` false/true (or
  Yes/No) updates `withheld_btn` snapshot `checked`/`value`. `category_of_agent`
  `P`/`G` (or `private`/`government`, bool true/false) updates `category_btn`
  snapshot `checked`/`value` (`P` Private default). Desktop drain
  writes those flags through `Agent1601CHostPatch` so the next snapshot /
  `form.fields` / `filing.validate` see the fill without a human click. `form.pdf` writes frozen HTML to an
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
- `profile.html` / `dues.html` write a temp `html-demo` bundle (`kind:"html"`,
  absolute `path`, no bytes). `profile.forms_set` is confirm-gated (string
  `"true"` refuses); a bool-true write is visible to `profile.forms_set.get`
  and `dues.list`
- Selected profile: checked `profile-{tin}` listitem and `context.selected_tin`
- Remaining form views: page root + back/save/submit chrome ids.
  Semantic **save** besides 1601-C and 2551Q is not mapped
- `hello.auth` is `Required` when the host is configured with a token (TCP
  stamps it on the mailbox path; `handle_request` fills it when given
  `expected_token` plus a v2 HMAC). `BirAgentHost::hello()` leaves `auth` at
  Default. Missing HMAC still fails with `automation token required`. A raw
  wire `token` field fails with `token must not be sent on the wire`.
- `bir-headless serve` opens a **file-backed** SQLCipher path (`app_database_path()`,
  not ephemeral). `profile.save` is visible after reopen. Bind-in-use and
  live-DB owner lock are refused without `--wait`; `serve --wait` resumes after
  the owner releases both. `serve --detach` backgrounds the same process
  (pid + logfile); `logs --follow` attaches. A second `--detach` while running
  fails. Headless **does** start the same in-process SFTP
  cron; it executes already-queued work and does not enqueue without
  `confirm=true`. GUI-as-client
  (full ADR-001) and a shipped LaunchAgent are not this slice.

Remaining (not faked):

- Semantic save for 1701Q / 0619E / 0619F / 0605 / 2550Q / 1701 / 1702RT / 1702MX
- Global Dashboard has no combobox; `dashboard.set_forms` is host/tree (+ profile FilterBar)
- `form.print` copies are ignored (frozen HTML preview has no copies API)
- `form.mark_paid` on 1601-C is unsupported (UI message only)
- `profile.calendar_sync` (Google push) needs a linked account
- Virtual in-window delivery stays `virtual_unavailable`. Screenshot: hosts
  confine a relative `.png` name first; macOS mailbox drain can then write this
  window (`screencapture -l`); Linux / Windows / headless stay
  `screenshot_unavailable`. Absolute client paths are refused.
- GUI-as-client of `bir-headless` (full ADR-001). First ship is shared
  persistence only; two AgentHosts must not share `GPUI_AGENT_ADDR`; two
  processes must not open the live DB together. Matrix B is
  `serve --wait` plus GUI quit releasing bind+lock, not a protocol op.
  launchd KeepAlive remains an optional supervisor (docs example only, no
  `--detach` in ProgramArguments). Lab/Linux uses `serve --detach`.

## Claimed queue without BIR outcome (facts)

SFTP resolve order: dry-run → complete `BIR_SFTP_*` → production dispatcher **only** with `BIR_SFTP_LIVE=1` (HTTPS then official
HTTP). Lab `BIR_SFTP_*` needs `BIR_SFTP_HOST_KEY_SHA256` or
`BIR_SFTP_ACCEPT_ANY_HOST_KEY=1`. Dispatcher host-key policy remains
accept-any (official ebfSFTP). `BIR_SFTP_DRY_RUN=1` (or `BIR_SFTP_LIVE=0`)
fakes the PUT in-process. IMAP skip uses the **session source**, not the
process-global dry-run env flag.

The worker still opens the session **before** claim, then claims immediately
before PUT. Unstarted claims may recover after a 120s lease. Once PUT is
marked started, the claim is fail-closed.

`filing.submit` only exposes confirmation; it does **not** claim or queue.
Enqueue stays the shipped `filing.queue` / GUI Submit paths.

Pre-PUT failures (XML / encrypt / revalidate / SFTP connect-login) stay
unclaimed and use `record_submission_failure` / retry-or-Draft
(`process_queued_1601c_pre_store_failure_stays_unclaimed_and_can_retry`). A
connect timeout therefore does **not** freeze a claim. After PUT start, a
PUT `Err` is **fail-closed**: the worker logs unknown outcome and does **not**
clear the claim (`process_queued_1601c_unknown_outcome_remains_claimed_and_is_not_retried`).

Live 1601-C Submitted rows become Confirmed when IMAP matches TIN / form /
period (BIR may strip `#email#` from the receipt filename) and the receipt
belongs to this queued generation (`authorized_at` floor). Dry-run never
schedules that poll.

Do **not** auto-release after PUT: clearing a claim after unknown upload I/O
can double-file. Stuck rows that already started PUT (crash or PUT error, or
legacy no-lease claims) still need human
`form.revert_draft` / `form.release_abandoned_claim` with `confirm=true` and
`reason=abandoned_no_bir_filing`.

## Background Tasks titles

Painted **Background Tasks** cards must distinguish returns that share a TIN or
mailbox. Do not grep for the old shapes `Submit 1601C for 00000000000000` or
`Waiting for 1601C confirmation email for user@example.com`.

| Kind | Source | Title |
| --- | --- | --- |
| Submit | UI from queued/submitted `form_drafts` (not `job_queue`) | `Submit {form} {Mon YYYY \| Qn YYYY} for {TIN-dashed} (email)` |
| Confirmation poll | `job_queue.name` after a live PUT | `Waiting for {form} {period} confirmation for {TIN-dashed} (email)` |

Examples:

- `Submit 1601C Sep 2026 for 000-000-000-00000 (codeitlikemiley@gmail.com)`
- `Waiting for 1601C Sep 2026 confirmation for 000-000-000-00000 (codeitlikemiley@gmail.com)`

The `(email)` parenthetical is omitted when the mailbox is unknown. `command`
stays `bir_poll_email {email}` so workers and **Run now** still key off the
mailbox. `jobs.list` returns stored poll-job names only.

Submit titles are computed at display time, so existing queued rows pick up
the new format immediately. Confirmation poll titles are stored on insert;
already-Queued poll rows keep their old name until they complete. Operators
should not treat a leftover `Waiting for 1601C confirmation email for …` row as
a second return — it is a pre-format poller for that mailbox.

## Email Settings: shared inbox OAuth

Several taxpayer profiles can use one Gmail inbox (the production Mac setup
is Alejandro / Andrea / Juan / Jane on `codeitlikemiley@gmail.com`). That
inbox has **one** Google OAuth credential set:

- Connect / Re-authorize / Disconnect on any profile writes
  `inbox_oauth_tokens` and copies the grant onto every sibling that shares
  the IMAP/email address. A dead refresh on the first `list_profiles()` row
  must not survive a reconnect on Jane or Juan.
- The confirmation poller authenticates with that shared grant, then matches
  BIR receipts by TIN / form / period. Jane’s Submitted 1601-C is confirmed
  even when Juan owns the grant.
- Google omits `refresh_token` unless consent is re-granted. The app already
  sends `prompt=consent`; an empty refresh is rejected and must not show
  Connected.

Do not log access or refresh tokens. Do not point tests at the live Mac
app-group DB.

