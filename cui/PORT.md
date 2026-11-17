# CUI port — progress and architecture

Port of the NSH (TSX) app (`nsh/`, PR #221 `feat/nsh-tsx`) to a native Rust + CUI immediate-mode UI,
using `cui-rs` (Rust bindings for CUI, crates.io `cui-rs = "0.1.3"`). Goal: full parity, then `nsh/`
is removed.

## Layout

`cui/` is its own cargo workspace (like `nsh/` was), crate `cui-app-ebirforms`, binary only.

```
cui/src/
  main.rs        modules + prepare() (process setup before the window) + main()
  app.rs         CUI window + shell (sidebar/caption/panels/toasts/palette) + frame loop
                 routes to screens::draw; BIR_SHELL_START env like NSH;
                 drives the updater driver from desktop::poll each frame
  bir.rs         THE ENGINE: one thread owns the door; UI sends Request over mpsc.
                 call/call0/call1/call_async/call_async_spawn + desktop/print/agent/shutdown faces
  door.rs        ported from nsh/src/door.rs (JsonValue -> serde_json::Value; registry() -> prepare())
  guards.rs      ported verbatim (send guard, database guard)
  desktop.rs     ported: mailbox, updater report, single-instance claim, hotkey_shortcut +
                 shortcut_parse, poll/publish_update/auto_download/instance (plain fns, no TSX module)
  hooks.rs       CuiHooks: file panels = blocking cui::file_dialog on the asking thread;
                 false when !app::ui_running() (matches NSH's try_os()==None in tests)
  auth.rs        ported verbatim (robius-authentication)
  print.rs       PORTED contract — request() disposes the source HTML (NSH's dispose), has_live
                 false, live_update no-op, states() drains one-shot `unavailable` notices;
                 desktop_host::tick toasts them. No preview window: CUI has no second window
                 or HTML renderer (NSH's non-desktop path said the same).
  agent.rs       STUB — send_draft errs "no agent control", pop_command None; feature `agent` off
  theme.rs       tokens (r#type/space/radius/size/breakpoints) + Palette light/dark from theme.json
  updater.rs     PORTED — updater.ts state machine (idle..installing) over an UpdaterApi trait;
                 UnconfiguredApi = this build (no update source, same as NSH); tests mirror
                 updater.test.ts; app.rs polls commands + publishes reports each frame
  menu_model.rs  PORTED — menu_model.ts as data (MenuEntry/MenuAction/MenuState, build_menu,
                 build_tray_menu, flatten, print_menu); 9 tests mirror menu_model.test.ts
  hotkey.rs      PORTED — hotkey.ts HotkeyWatch state machine over sync HotkeyDeps
                 (settled-state logic; in-flight register races need the async variant once a
                 real OS binding exists); tests mirror hotkey.test.ts's pure cases
  palette.rs     PORTED — paletteItems.ts: grouped items (Taxpayers/Branches/Go to/Forms/
                 Actions), TIN-digit matching, run actions; draw_palette renders groups,
                 shortcuts() wires the menu accelerators + palette keys (CUI has key APIs)
  desktop_host.rs PORTED — Desktop.tsx + actions.ts + banner.ts + UpdateSlot: updater tick,
                 perform() -> AppState, About dialog, update banner (Install now/Later ->
                 install_update/dismiss_update host fns); no native menu/tray/hide/focus yet
  lib/           pure helpers ported + tested: dates, money, format(tin/code), errors,
                 periods, queue_message, chrome_gate, mask, todo, dashboard, settings_model
                 (+ jobs/logs logic: STATUS_FILTERS/LOG_FILTERS/log_view/log_headline/
                 filter_jobs/job_tone/job_actions/job_key/job_sub/server_hint/
                 retention_choices/hotkey_choices/group_sections; tests mirror logic.test.ts),
                 facts (FactName/labels/reasons, income types, EOPT tiers, election choices,
                 fact_value, election_source_text, reader_chips, join_names, not_set_for_form/
                 sentence; tests mirror facts.test.ts),
                 filings_model (statuses/columns/period keys/sorting/filters/presets/
                 deadline keys; tests mirror model.test.ts),
                 library_rules (search/scope/freq/counts; tests mirror rules.test.ts),
                 taxpayer_logic (logic.ts: dates/ATC/errors/new-taxpayer/setups/years/
                 schedules/templates/section metas; tests mirror logic.test.ts)
  bir/types.d.ts copied from nsh/bir/types.d.ts (door test requires it current)
  state.rs       port of nsh/shell/app.ts: routes, history, selection, polls, toasts
  store.rs       per-frame data cache over bir::call (the store.ts port)
  types.rs       serde door types
  ui.rs + ui/kit.rs  UiExt trait + widget kit over cui::Ui (palette-tinted)
  form/schema.rs contract/schema types + bind_error/one_date/fold helpers
  form/model.rs   PORTED — controls/model.ts: option_of/listed_options/select_model/is_on/
                 tin_kind/mdy/read_text/split_label/match_words/row_parts/row_title_of;
                 9 tests mirror model.test.ts; sel_shown delegates to select_model
  form/session.rs PORTED — session.ts (~2900 lines TS): open/fill/validate/autosave,
                 sample period, removal hold/undo, profile locks+edit, penalties, stop-here,
                 asks/election, queue/check/print, timers+async polls via channels
  screens/       auth, home, library, calendar, filings, settings, taxpayer, form, mod (router)
                 form.rs renders FormSession each frame (thread_local session state)
```

## Key decisions

- **Engine thread.** `door.rs`'s `HostModule` and the database slot are thread-local, and the desktop
  mailbox is thread-local too, so ALL door/desktop/print/agent calls funnel through one engine thread
  (`bir.rs`). Sync calls block the UI thread on the reply (fast DB reads); async host functions are
  awaited on the engine (`call_async_spawn` for non-blocking).
- **File dialogs.** `cui::file_dialog` is a blocking native panel (zenity/kdialog on Linux); called on
  the engine thread inside the host call, so `PathAnswer` fires on the database-owning thread as the
  trait requires. On macOS this may need dispatch to the main thread later.
- **Theme.** CUI's own light/dark theme + accent stay; `theme::Palette` covers the app roles CUI has
  no token for (muted/subtle text, accent-blue, soft tints, selected fill, primary). Inter is embedded
  in cui-rs `default-fonts`.
- **Updater.** No update source is configured (same as NSH build): `updater.rs` runs the full state
  machine over `UnconfiguredApi`, publishes the honest "idle / not configured" report; a configured
  build only swaps the API. `auto_download` stays authoritative in the desktop mailbox (DB-backed).
- **Form session state.** `screens/form.rs` keeps the `FormSession` in a `thread_local`
  `RefCell<Option<..>>` keyed by form target; session events drain into `AppState` (toasts, route
  changes, store bumps) each frame.
- **Polls.** `lib/poll.ts` has no timer port: NSH's startPolls maps onto the frame tick —
  `state.tick()` runs events (30 s, POLL_KEYS)/device-filings (5 s)/update (1 s) behind the
  App.tsx `polling` gate (`!test_env || BIR_SHELL_POLL=1`), the queued watch (3 s/30 s fast
  phase) and warmLibrary (600 ms after first chrome-visible, re-arms if re-locked). The desktop
  mailbox is already drained every frame. `store::poll_key`/`worker_key` mirror POLL_KEYS/
  WORKER_KEYS exactly (jobs/saved_returns are whole-key matches, not prefixes).
- **New return.** `draw_new_return` is NewReturn.tsx's single dialog: form list (code+title+
  freq, selected row), period options, Template/No template chip, the not-set sentence with
  "Open the profile", Cancel/Open form -> `AppState::confirm_new_return` -> `open_return`.
- **Taxpayer facts.** `lib/facts.rs` is fully wired: the profile tab's "Facts your forms use"
  card (Facts.tsx FactsBody/FactRow) lists ROW_FACTS filtered by used_by, shows fact_value,
  "Read by" chips, election source/locked/why lines, head-office owner note, Not set flags
  (rowFlagged), hidden-facts line; Set/Change opens an inline edit — segmented choices
  (incomeTypeChoices/ELECTION_CHOICES/EOPT_TIERS/Yes-No), ATC codes text field
  (parseAtc_codes), Graduated-keeps-method rule — saved via set_year_facts with the
  "{label} saved for {year}." toast. facts_meta/hidden_facts_text/parse_atc_codes/
  year_facts_view/readiness/not_set_view are ported with tests.
- **Settings jobs/logs.** `settings_model` jobs+logs helpers are wired into
  `screens/settings.rs`: Jobs/Logs tabs, status + level filters, log tail cached until
  Refresh or tab open (jobsState.ts LOG_KEY), job rows offer Cancel queue (real
  cancel_queue host call with tin/code/year/quarter-or-month)/Archive/Delete, Purge
  archived, failure line stays on the tab.
- **Taxpayer Forms/Templates tabs.** `screens/taxpayer.rs` draw_forms is now the NSH
  two-mode model: view rows (schedule line via store::form_deadlines +
  taxpayer_logic::schedule/form_sub, freq chip, Due date, Template/No template chip),
  Edit forms checklist over store::screen_index with search_text matching + freq
  filter + setups row (apply_setup/copy -> after_apply toast + ReplaceAsk banner ->
  confirm_replace replace=true; delete_setup for non-built-in), Add year band
  (allowed_years/default_add_year/year_added_text, copy-forms checkbox), annual
  conflict + replace banners. Templates tab lists the year's forms with
  template_counts/Updated date and Create/Update (create_template_from_latest +
  template_error_text)/Delete (delete_form_template now passes tin — was missing).
  New taxpayer submit validates via new_taxpayer_problems' rules (TIN shape + in-use,
  name, fact_error for zip/rdo/email with knownRdo).
- **Profile tab sections.** draw_profile now covers all seven NSH sections as cards:
  Taxpayer, Registered activity (branch code/name/RDO + year's line_of_business/
  business_start_date from year_facts), Tax types · your forms (forms+freq, tax-type
  chips via store::tax_types, Change -> open_forms_edit, Save as a setup -> save_setup),
  Facts your forms use, Address and contact (inherit "(head office)" markers),
  Confirmation inbox — full Inbox.tsx parity: method pick (GoogleOAuth/AppPassword),
  credential editor (email/host/password with EMAIL_RE check, "Stored"/"Not set"
  read state, 2-Step note), save via set_inbox_app_password (empty -> null keeps
  stored), Google Connect/Re-authorize/Disconnect (disconnect_inbox_oauth),
  Verify connection / Check now as one-at-a-time async jobs (bir::call_async_spawn
  drained per frame; start line while running, answer line + toast after, key-round
  connect clears the line, check_text wording, still_pending), Receipt tracking
  switch with NSH toast wording, Last check line, trackingLine status, branch-switch
  reset of edit+job line, Agent filing (grant_send/revoke_send
  switches), Archive or delete (admin-only canRemove, two-step delete -> delete_taxpayer
  -> Home). SectionEdit is now page.ts-faithful: inherit flags + "Same as head office",
  touched/submitted error display via edit_problems/edit_errors (TIN/branch-code rules,
  factError, pastDateError), yearFields/year binding with the changed-year save refusal
  and leave_year_edit on year switch, TIN move (update_taxpayer then move_taxpayer_tin
  + remap_recent + reselect, refusal keeps edit open), branch-code move
  (move_branch_code + remap + select_branch), set_year_facts patch for the year's
  activity fields. Sections are now collapsible Disclosures (SEC_OPEN first-visit
  defaults, chevron+meta header, Edit in the header, close cancels the section's
  edit/fact-edit/inbox edit) and a (tin, code) watch drops every in-progress edit on
  a taxpayer/branch switch (page.ts `reset`). New branch is full NewBranch.tsx
  parity: suggest_next_branch_code on open, head-office values with "Same as
  head office" inherit switches, per-field fact_error validation via
  shown_errors semantics, host "branch code" errors keyed to the code field,
  create_branch with null for inherited fields, select_branch + "{name} added.".
- **Tests.** `cargo test` in `cui/` — 196 tests green: door/guards/desktop/auth, lib helpers,
  mask + todo suites ported from `nsh/ui/mask.test.ts` + `nsh/lib/todo.test.ts` (these caught a real
  empty-message bug in `todo_rows` and a negative-paste bug in money masks), updater state machine,
  menu model, hotkey machine, update banner.
  `cargo check`/`test`/`build` must stay green.

## Not yet done (in rough order)

1. Frame polish DONE: `theme=dark` start key honored (state.rs boot), narrow layout = bottom tab
   bar (state::TABS) + sidebar/panel as modal dialogs below 860 (CUI has no absolute layer; the
   dialog's scrim stands in for NSH's floating scrim).
2. Form page depth: schedule drawer PORTED (compact row lines, Add row, totals cards, modal
   drawer with stacked cells + Delete row/Done — controls/Schedule.tsx); date fields PORTED
   (typed + month-grid picker dialog with min/max, one_date single box, split-date calendar
   with dateRange/splitIso — rows.tsx). Still open: true row virtualization (CUI exposes no
   scroll-offset read, so viewport culling is impossible; all rows draw through the page
   scroll), form sections/pill keyboard nav, selection-aware mask reconciliation DONE (`reconcile` takes `before`; 6 test fns mirror
   mask.test.ts's selection cases — CUI exposes no selection/caret yet, callers pass None).
3. Print preview window itself: needs a second CUI window + HTML/WebView renderer; the
   request/dispose/unavailable-notice contract is ported and honest.
4. `agent.rs` + `agent` feature (needs vendored gpui-agent; git fetch needs credentials).
5. Native menu/tray/window-hide/focus/global-hotkey: CUI has none — menu_model/hotkey/updater
   machines are ported; accelerators already work via `shortcuts()`; `perform` covers every
   action. Needs platform code or CUI upstream features for the rest.
6. Real-window screenshot checklist vs NSH shots: app launches under Xvfb at 1440x900 and
   resizes live to 500x800 (xdotool); dark theme via BIR_SHELL_START="theme=dark". PNGs land in
   /tmp (cui-wide.png, cui-narrow.png). Visual diff vs NSH shots still needs a human eyeball —
   the `read` tool can't display images in this environment.
7. Remove `nsh/`, `.github/workflows/nsh.yml`, update docs when parity lands.
8. `cargo check --workspace` at repo root CANNOT run in this environment: `bir-mcp` has a
   non-optional git dep on github.com/hexuria/gpui-agent@dab85991 which is unfetchable here
   (private/no creds; cargo git db lacks the rev). `cui/` is its own workspace and stays green;
   the same block prevents the `agent` feature port (needs that source).

## Reference

- NSH source of truth for behavior: `nsh/` (screens/, shell/, lib/, bir/, src/*.rs).
- CUI API: `~/.cargo/registry/src/*/cui-rs-0.1.3+cui1.4.0/cui/src/lib.rs` + `ext.rs` + `gen_safe.rs`;
  C header in `cui-sys-*/c/cui.h` (499 bound functions, incl. data grid, file picker, kanban, dock).
- Host function signatures: `crates/bir-app-api` `declared()` (TS in `nsh/bir/types.d.ts`,
  mirrored at `cui/bir/types.d.ts`).
