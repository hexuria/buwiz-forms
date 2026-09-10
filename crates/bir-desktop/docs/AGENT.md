# gpui-agent integration (bir-desktop)

Opt-in control plane so Grok Bot (or any MCP/CLI client) can drive **eBIRForms**
beside a human: navigate pages, create a tax profile, inspect dues, and prepare
a form draft. It speaks the generic gpui-agent protocol only. BIR-specific
verbs live in this host as `invoke` names.

Pinned crate: [`gpui-agent`](https://github.com/hexuria/gpui-agent) commit
`8857139af12fb033b4dd04eabd8d19b5bfc5ffc6` (`main` tip, Merge PR #32 / epic
children). Host GPUI is **gpui-pre** through gpui-kit 0.6.1. Cookbook:
[`docs/INTEGRATING.md`](https://github.com/hexuria/gpui-agent/blob/8857139af12fb033b4dd04eabd8d19b5bfc5ffc6/docs/INTEGRATING.md)
and [`docs/SDK.md`](https://github.com/hexuria/gpui-agent/blob/8857139af12fb033b4dd04eabd8d19b5bfc5ffc6/docs/SDK.md).
Do not fork the protocol. There is no crates.io release; git/path only.

## Locked protocol contract

These are host constraints. They do not change protocol v1.

- Depend on git/path `gpui-agent` at that rev only (no crates.io).
- `bir-core` has no GPUI / gpui-agent dependency. Painted-window and virtual
  glue stay behind bir-desktop `--features agent`.
- Loopback default via `gpui_agent::from_env` / `authorize_bind`. Non-loopback
  needs `GPUI_AGENT_REMOTE=1` and a non-empty `GPUI_AGENT_TOKEN` (SDK, not a
  BIR-invented bind). This host does not add a second bind path. Product BIR
  is still an **in-process mailbox** on the painted window (widget E2E), not a
  `todo-headless` daemon + GUI-as-client (ADR-001). There is no `bir serve`
  binary.
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

## Security

- Feature `agent` is **off** by default. Product/release builds must leave it off.
- Runtime starts only when `GPUI_AGENT=1` (`true`/`yes`/`on`).
- Release binaries also need `GPUI_AGENT_ALLOW_RELEASE=1`.
- Bind defaults to loopback (`127.0.0.1:17421` unless `GPUI_AGENT_ADDR` is set).
  `from_env` calls `authorize_bind`: non-loopback requires `GPUI_AGENT_REMOTE=1`
  **and** a token. Do not invent a second remote bind. Transport is still
  plaintext TCP.
- `GPUI_AGENT_TOKEN`, when set, is required on every request. Recipe run and MCP
  **always** need the same non-empty token on host and client. The token is never logged.
  `hello.auth` is `"required"` when that token is configured on the host, `"none"`
  otherwise. The TCP thread enforces the token; the UI-thread mailbox drain passes
  the same configured token into `handle_request` so hello does not overwrite `auth`
  to `"none"`. `BirAgentHost::hello()` does not set `auth` by hand.
- The agent **cannot** skip the lock screen, profile PIN/TOTP, or administrator
  OTP. Unsaved profile compliance still blocks navigation.
- There is **no** invoke that queues or files a return. `filing.submit` (and a
  semantic click on `submit_btn`) only exposes the confirmation node.
  Confirming `form-1601c-submit-confirm` is refused. Complete filing in the BIR UI.
  `form.release_abandoned_claim` only returns a claimed Queued snapshot to Draft
  after a human confirmed nothing reached BIR; it does **not** file.
- Virtual `click` / `type` / `key` return `virtual_unavailable`. Do not point
  agents at `--delivery virtual` on this host.
- Snapshots include names, last-4 TIN, dues, and editor fields needed to drive
  flows. They do not include PIN hashes, TOTP secrets, or keychain material.

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

## Headless vs painted window (CLI smoke)

BIR has **no** `todo-headless serve` binary. Linux can run headless **host unit
tests** (`spawn_host` + `PlatformKind::Headless`). A long-running daemon that
owns the live taxpayer DB is not shipped.

Live mailbox (needs a GPUI window + display; Linux X11/Wayland, Mac window):

```bash
export GPUI_AGENT=1
export GPUI_AGENT_TOKEN='dev-secret'
export GPUI_AGENT_ADDR='127.0.0.1:17421'
cargo run --locked --bin bir --features agent
# other terminal:
gpui-agent --addr 127.0.0.1:17421 --token dev-secret hello
gpui-agent snapshot
# macOS only, observe-only PNG of this window (Screen Recording):
# gpui-agent screenshot --path /tmp/bir-window.png
```

Headless TCP without a painted window (fixture host, not the live DB):

```bash
cargo test --locked -p bir-desktop --features agent --bin bir \
  agent::host::tests::headless_tcp_host_serves_hello_and_nav
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

### Recipes

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

`profile-create.json` **writes a taxpayer profile into the live app database**.
Do not run it against a real Mac app-group DB. Headless tests use
`Database::open_ephemeral()`.

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

Linux/cloud agents can compile and run the headless host tests. **macOS runtime
is not claimed until Uriah runs the commands above on a Mac.**

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
| `form.fields` | — | Required/optional fields, current values, `profile_defaulted` / `fillable` for the open 1601-C or 2551Q |
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
real `FilingStatus` (`Queued` while claimed; never `Draft` until the release
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

Remaining (not faked):

- Semantic save for 1701Q / 0619E / 0619F / 0605 / 2550Q / 1701 / 1702RT / 1702MX
- Global Dashboard has no combobox; `dashboard.set_forms` is host/tree (+ profile FilterBar)
- `form.print` copies are ignored (frozen HTML preview has no copies API)
- `form.mark_paid` on 1601-C is unsupported (UI message only)
- `profile.calendar_sync` (Google push) needs a linked account
- Virtual in-window delivery stays `virtual_unavailable`. Screenshot: macOS
  mailbox drain can write this window (`screencapture -l`); Linux / Windows /
  headless stay `screenshot_unavailable`.

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
