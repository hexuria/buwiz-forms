# gpui-agent integration (bir-desktop)

Opt-in control plane so Grok Bot (or any MCP/CLI client) can drive **eBIRForms**
beside a human: navigate pages, create a tax profile, inspect dues, and prepare
a form draft. It speaks the generic gpui-agent protocol only. BIR-specific
verbs live in this host as `invoke` names.

Pinned crate: [`gpui-agent`](https://github.com/hexuria/gpui-agent) commit
`254953f2b6a1d91030f9467caa05f69c063c7204`. Host GPUI is **gpui-pre** through
gpui-kit 0.6.1. Do not depend on gpui-agent PR #20 (macOS screenshot). Do not
fork the protocol. There is no crates.io release; git/path only.

## Security

- Feature `agent` is **off** by default. Product/release builds must leave it off.
- Runtime starts only when `GPUI_AGENT=1` (`true`/`yes`/`on`).
- Release binaries also need `GPUI_AGENT_ALLOW_RELEASE=1`.
- Bind is loopback only (`127.0.0.1:17421` unless `GPUI_AGENT_ADDR` is a loopback address).
  Authenticated remote bind is not implemented (gpui-agent epic #10).
- `GPUI_AGENT_TOKEN`, when set, is required on every request. Recipe run and MCP
  **always** need the same non-empty token on host and client. The token is never logged.
- The agent **cannot** skip the lock screen, profile PIN/TOTP, or administrator
  OTP. Unsaved profile compliance still blocks navigation.
- There is **no** invoke that queues or files a return. `filing.submit` (and a
  semantic click on `submit_btn`) only exposes the confirmation node.
  Confirming `form-1601c-submit-confirm` is refused. Complete filing in the BIR UI.
  Virtual clicks on submit/queue controls are refused so they cannot hit `mark_submitted`.
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
id for a selected 1601-C obligation (year/period are not list indices):

```bash
gpui-agent recipe run crates/bir-desktop/recipes/form-1601c-draft.json \
  --set due_id=due-1601C-2026-1
```

Linux/cloud agents can compile and run the headless host tests. **macOS runtime
is not claimed until Uriah runs the commands above on a Mac.**

## Invoke allow-list (BIR host only)

Preferred names (domain commands). Aliases in parentheses still work.

| Name | Args | Effect |
| --- | --- | --- |
| `nav.go` | `page` | Navigate to an `ActiveView` slug (`global-dashboard`, `dashboard`, `profile-manager`, `settings`, `form-1601c`, …) |
| `profile.create` (`profile.new`) | — | Open Create Profile |
| `profile.save` | — | Validate + `Database::save_profile` (same checks as the Save Profile button) |
| `tax-dues.refresh` | — | Reload dues for the selected profile |
| `filing.start` (`form.open`) | `code`, `year`, `period` | Open a form for the selected profile |
| `filing.validate` (`form.validate`) | — | Run 1601-C `FormValidator` |
| `form.save_draft` | — | Persist a 1601-C **draft** |
| `filing.submit` | — | Validate and expose confirmation; **does not queue or file** |
| `form.submit` / `form.queue` / `form.file` / `filing.queue` / `filing.file` | — | **Rejected** |

## Stable IDs

See `src/agent/ids.rs`. Page roots are `page-*`. Sidebar nav IDs match existing
widget ids (`global_dashboard_btn`, `settings_sidebar_btn`, …). Profile rows are
`profile-{tin}`. Dues rows are `due-{form}-{year}-{period}` (not list indices).
1601-C: `page-form-1601c`, `save_draft_btn`, `submit_btn`, `form-1601c-tax-14`,
`form-1601c-tax-25`, `form-1601c-submit-confirm`. Item 25 is required when
Any Taxes Withheld is YES.

## Coverage (this slice)

Proven in headless host tests (not a Mac GUI run):

- Navigation: all 18 `ActiveView` page roots via `nav.go` + assert
- Admin / lock gates are not bypassed
- Profile create on an ephemeral DB, row visible and selected
- Fixture 1601-C due rows with stable ids
- 1601-C draft save + validate + confirmation node; status stays `Draft`;
  `form.submit` / `filing.queue` rejected
- Remaining form views: page root + back/save/submit chrome ids.
  Semantic **save** besides 1601-C is not mapped (`draft save for … is not mapped`)

Follow-up: per-widget painted bounds for reliable virtual clicks; semantic save
for 2551Q / 1701Q / 0619E / 0619F / 0605 / 2550Q / 1701 / 1702RT / 1702MX;
screenshot once gpui-agent PR #20 lands.
