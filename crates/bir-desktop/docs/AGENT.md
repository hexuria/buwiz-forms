# gpui-agent integration (bir-desktop)

Opt-in control plane so Grok Bot (or any MCP/CLI client) can drive **eBIRForms**
beside a human: navigate pages, create a tax profile, inspect dues, and prepare
a form draft. It speaks the generic gpui-agent protocol only. BIR-specific
verbs live in this host as `invoke` names.

Pinned crate: [`gpui-agent`](https://github.com/hexuria/gpui-agent) commit
`254953f2b6a1d91030f9467caa05f69c063c7204`. Host GPUI is **gpui-pre** through
gpui-kit 0.6.1. Screenshot is unavailable on this pin; it is available after
bumping gpui-agent to main tip ≥ `b630bf7` (PR #20). Do not fork the protocol.
There is no crates.io release; git/path only.

## Locked protocol contract

These are host constraints. They do not change protocol v1.

- Depend on git/path `gpui-agent` at that rev only (no crates.io).
- `bir-core` has no GPUI / gpui-agent dependency. Painted-window and virtual
  glue stay behind bir-desktop `--features agent`.
- Loopback only via `gpui_agent::security::from_env`. Authenticated remote bind
  / daemon source of truth is gpui-agent epic #10 and is **not** on main. This
  host does not invent a remote bind.
- Preferred BIR `invoke` names (app-only, not CLI/MCP verbs) include
  `nav.go`, `profile.list` / `profile.search` / `profile.set` / `profile.edit` /
  `profile.tab`, `dues.list`, `jobs.list`, `search.open`, `palette.search`,
  `form.fill` / `form.pdf`, plus the original `profile.create`,
  `tax-dues.refresh`, `filing.start`, `filing.validate`, `filing.submit`.
  `filing.submit` maps to the existing confirmation gate; it does not queue or
  file. Match-arm synonyms (see the Alias table) are not a second allow-list.
  There is **no** `profile.ensure` auto-write; `profile.create` only opens the
  editor.
- Semantic delivery is the supported path. Virtual ops return
  `virtual_unavailable` rather than synthesizing OS HID or a half-wired
  in-window pointer. Protocol is unchanged.
- Screenshot returns `screenshot_unavailable` on current pin `254953f`.
  Available after bumping gpui-agent to main tip ≥ `b630bf7` (#20). This BIR
  branch does not bump the pin.

## Security

- Feature `agent` is **off** by default. Product/release builds must leave it off.
- Runtime starts only when `GPUI_AGENT=1` (`true`/`yes`/`on`).
- Release binaries also need `GPUI_AGENT_ALLOW_RELEASE=1`.
- Bind is loopback only (`127.0.0.1:17421` unless `GPUI_AGENT_ADDR` is a loopback address).
  Authenticated remote bind is not implemented (gpui-agent epic #10).
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
git checkout 254953f2b6a1d91030f9467caa05f69c063c7204
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
gpui-agent invoke nav.go --arg '{"page":"profile-manager"}'
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

Natural-language → invoke examples (CLI, not recipe JSON):

```bash
# "open the Acme profile"
gpui-agent invoke profile.set --arg '{"q":"acme","view":"dashboard"}'
# 0 hits → result.status=not_found (does not create)
# many hits → result.status=ambiguous + candidates; prompt the human with widgets

# "edit this taxpayer's COR tab"
gpui-agent invoke profile.edit --arg '{}'
gpui-agent invoke profile.tab --arg '{"tab":"cor"}'

# "what is due this month / what is overdue"
gpui-agent invoke dues.list --arg '{"filter":"upcoming"}'
gpui-agent invoke dues.list --arg '{"filter":"overdue","scope":"global"}'

# "show background jobs"
gpui-agent invoke jobs.list --arg '{}'
gpui-agent invoke submissions.list --arg '{}'

# "fill 1601-C from the profile then export a print document"
gpui-agent invoke form.fill --arg '{"fields":{"tax_14":"1000.00","tax_25":"100.00"}}'
gpui-agent invoke form.pdf --arg '{}'

# "open command palette" (Cmd+K overlay). palette.search stays the query invoke.
gpui-agent invoke search.open --arg '{}'
gpui-agent invoke palette.search --arg '{"q":"acme"}'
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
gpui-agent invoke form.fill --arg '{"fields":{"tax_14":"1000.00","tax_25":"100.00"}}'
gpui-agent invoke form.save_draft --arg '{}'
gpui-agent invoke form.pdf --arg '{}'
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
| `form.fill` | `fields` object | Set only provided fillable keys; refuse unknown. 1601-C: `tax_14`, `tax_25`, `sheets`. 2551Q: `creditable_tax_withheld`, `other_tax_credit`, `taxable_amount` |
| `form.save_draft` | — | Persist a 1601-C or 2551Q **draft** |
| `form.pdf` | — | Real `bir_print::frozen_html::filled_document` pipeline to a temp `index.html` (TIN stamps, writer-cell identity including email, header period, demo tax `money_joins`). Does **not** run `filing.validate` and does not refuse on validation errors. Returns `{path, kind:"frozen-html"}` with an **absolute** `path`. Does **not** put file bytes on the invoke result |
| `form.print` | optional `copies` (ignored; preview has no copies API) | Desktop: flags the existing frozen HTML preview. Headless: error. Never queues filing |
| `form.revert_draft` | — | Unclaimed queued 1601-C / 2551Q cancel APIs only |
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
`form-1601c-submit-confirm`. 2551Q fillables: `form-2551q-creditable`,
`form-2551q-other-credit`, `form-2551q-taxable-0`. Item 25 is required when
Any Taxes Withheld is YES.

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
- `form.fill` refuses unknown keys; `form.pdf` writes frozen HTML to an
  absolute `path` via the real print pipeline (no bytes on the result) and
  does **not** call `filing.validate`. A Confirmed 2551Q can still have
  draft validation errors (stale `profile_snapshot`, missing
  `annual_income_tax_election` / `taxpayer_type` / `item_13_election`, stale
  surcharge/interest/compromise); those are orthogonal to print fill.
  headless `form.print` errors; `form.upload_receipt` is `needs_file` with
  `path: null`; 2551Q `form.save_draft` is mapped
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
- Virtual in-window delivery stays `virtual_unavailable`. Screenshot stays
  unavailable on current pin `254953f`; available after bumping gpui-agent to
  main tip ≥ `b630bf7` (#20).
