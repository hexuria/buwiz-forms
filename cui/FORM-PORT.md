# Form port tracking — all 43 NSH forms

Goal: `GOAL-FORMS.md`. Reference: `nsh/screens/form/` (FormPage, rows, controls/*).
Implementation: `cui/src/form/` (schema/model/session) + `cui/src/screens/form.rs`.

## Pipeline status (fixed this pass)

- `screen_form_schema` payloads contain explicit `null`s and `{value,label}` option
  objects; `FormSchema` now deserializes both (`null_default`, `Vec<serde_json::Value>`
  options). Regression test: `door::tests::screen_form_schema_all_parse` (all 43).
- CUI renders on demand; spawned host calls are counted (`bir::async_in_flight`) and the
  frame loop calls `ui.request_redraw()` while any are unresolved — async opens no longer
  stall at "Opening the return…" while the window is idle.
- Dev deep link: `BIR_OPEN_SAMPLE=<code> ./run-dev.sh` opens that form's sample on launch.
- Live audit driver: `cui/audit-forms.sh` (screenshots + OCR per form -> /tmp/form-audit.txt).

## Layout (FormPage.tsx three-region port)

- Wide (>= ~760pt column): left "On this form" navigator (~214pt + edge pad) with part
  count ("N of M"/"N parts"), expand/collapse-all toggle, needs-attention row, one row per
  part (part name, error badge, item count; `compact_form_nav` pref gives the one-line
  `compactPartName` variant). Click jumps (`jump_to`) and reveals.
- Centre: header (BIR Form code, revision, title, chips: freq/pages/fields/Draft layout/
  status/Amended/Filed original/Sample/Opening), action row (Print preview, filed-original
  buttons, Check, Queue, Cancel queue, Save as return…, Save amendment), SaveLine, notes
  strips (sample/profile-updates/notes/profile-differs+Use profile/amends/filed-original/
  profile-changed/submitted/form errors), then part cards with profile fold, items,
  asks/elections/penalties/to-profile/from-hints/stop-here/messages.
- Bottom floating pill centred over the form column: ‹ prev, list icon + sections popover,
  two-line label (small: "N to do · tag" / "i of n to do · tag"; big: item or section
  short), attention badge, › next (check icon on the Check target). Sections popover is
  anchored TOP_END on the next icon at 476pt so it sits centred on the pill; rows jump and
  close; inner scroll caps at the popup max height.
- Narrow: same pill; NSH uses a bottom Sheet for the section list there — CUI keeps the
  popover (documented deviation, see below).
- Reveal: `FormSession.reveal` consumed one settled frame later; scrolls `form-scroll` the
  least that shows the target (cui__dp_reveal rule).
- Modal guard: `kit::modal_off()`/`set_modal` suppress custom `press_hit` hit tests under
  the popover, date picker and schedule sheet so their own controls still take clicks and
  the dismiss click can't leak to the page.

## Dialogs (NSH `Dialogs`)

- removal-confirm: message as title, Continue/Cancel.
- existing-offer: `schema::existing_offer_for` — title/subtitle, choices with
  label+hint+variant (primary/danger/secondary), "Don't ask again for drafts" fast-lane
  checkbox -> `set_fast_lane`; Cancel/keep -> `take_existing("keep")`.
- queue-confirm: "Queue this return?" + warnings, Continue/Cancel.
- sample-period: "Which period is this return for?", PERIOD_PART_LABELS inputs, quarter
  parts as single-select Q1-Q4 buttons (one "true", rest "false"), error line, Cancel/Save.

## Session-level audit — `form::session::tests::all_form_sessions_open_edit_and_check`

Drives the real `FormSession` against the host on a scratch database for **all 43 codes**
in two passes (18s total, all PASS):

- **Sample pass**: open → step through every step target → one edit of each editable kind
  present (text/money/pct/int/date/tin/sel/chk/radio) → `flush` → `check` (validate) →
  autosave quiescence.
- **Return pass**: open as a real return at the form's first valid period → answer every
  ask the host offers through `save_ask` → `check` and take the income-tax election when
  validation asks (2551Q picks `graduated`) → edit → `check` → `save_now` → quiescence.
  All 43 reach `save=Saved`; asks were offered and answered on 1700, 1701, 1702EX.

- Every form opens with a draft (no "no on-screen layout", no open errors).
- `screen_form_fill` accepts the edits on every form (no `SaveState::Failed`).
- `screen_form_validate` answers on every form; `shown_counts` reports the expected
  attention items for an intentionally-touched sample.
- Sample saves correctly report `manual(Sample · set its period to save)` — the NSH
  period gate, not a bug (1702MX reports `Unsaved`, also correct).
- `steps=80` is the test's bound: `step_target(1)` wraps, so each form cycled its full
  step list repeatedly without error.

## Live audit — all 43 forms (audit-forms.sh, display :1)

Every code opened its sample and rendered parts/fields/navigator/pill with no
"Opening the return…" stall, no "no on-screen layout", no parse errors.

| Code | Status | Controls exercised (schema kinds) |
|------|--------|----------------------------------|
| 0605 | PASS | text/number/money/date/select/radio/check |
| 0619E | PASS | alphalist-style fields |
| 0619F | PASS | schedule rows |
| 1600PT | PASS | percentage/penalty fields |
| 1600VT | PASS | VAT fields |
| 1600WP | PASS | withholding fields |
| 1601C | PASS | compensation (verified interactively: nav, pill, popover, step) |
| 1601EQ | PASS | schedule (equity) |
| 1601FQ | PASS | schedule (franchise) |
| 1602Q | PASS | quarterly fields |
| 1603Q | PASS | quarterly fields |
| 1604C | PASS | schedule (creditable) |
| 1604E | PASS | schedule (estate) |
| 1604F | PASS | schedule (verified: row drawer content) |
| 1606 | PASS | annual fields |
| 1700 | PASS | income tax fields |
| 1701 | PASS | annual fields |
| 1701A | PASS | annual fields |
| 1701MS | PASS | monthly fields |
| 1701Q | PASS | quarterly fields |
| 1702EX | PASS | export fields |
| 1702MX | PASS | misc fields |
| 1702Q | PASS | quarterly fields |
| 1702RT | PASS | percentage fields |
| 1706 | PASS | capital gains fields |
| 1707 | PASS | schedule rows |
| 1707A | PASS | schedule rows |
| 1800 | PASS | schedule rows |
| 1801 | PASS | annual fields |
| 2000 | PASS | schedule rows |
| 2000OT | PASS | annual fields |
| 2200A | PASS | percentage fields |
| 2200AN | PASS | annual fields |
| 2200C | PASS | corporate fields |
| 2200M | PASS | percentage fields |
| 2200P | PASS | partnership fields |
| 2200S | PASS | percentage fields |
| 2200T | PASS | percentage fields |
| 2550M | PASS | monthly fields |
| 2550Q | PASS | quarterly + schedule rows (verified rendering) |
| 2551Q | PASS | quarterly fields |
| 2552 | PASS | quarterly fields |
| 2553 | PASS | quarterly fields |

## Known shared behaviors (verified on samples)

- Radio groups render as tick boxes grouped by `grp` (`pick_radio` enforces one-answer).
- Schedules render as row lists + right sheet drawer editor (NSH Schedule.tsx), per-row
  number/caption, add/remove/done.
- One-date items collapse month/day/year into a single date box; split dates get a
  calendar button beside the boxes (`pick_date` fills open parts). Plain `date` fields
  draw a 170pt input with the calendar icon at its right edge (NSH DateField); the
  picker's `popover_begin` runs inside the trigger's `pick-` id scope so `cui_make_id`
  hashes the same popup id `popover_open` set (verified live on 1701 item 10).
- Date picker is a real calendar popover (`fp-cal-*`): Su–Sa day grid with dimmed
  outside-month days, ‹ › month paging, month and year label buttons switching to
  12-month and 12-year grids (nsh-base DatePicker's month/year toggle views), min/max
  days disabled per `dateRange`, pick writes back and closes, outside click/Escape
  dismisses. A `settle` frame guard stops the opening/view-switch press from landing
  on a cell that appears under the cursor (CUI buttons re-report the press).
- Elections, asks (choice/text), penalties (edit/use computed), stop-here, from-hints
  ("Use it"), to-profile switches, profile-edit cards all wired in the item footer.
- "Additional details" items fold behind a ghost toggle (`more_open`).
- Autosave/dirty/save-state line, step/jump/reveal, todo rows, queue + confirm + cancel.
- Print preview: honest "not available" on this build (no second window/HTML renderer —
  same answer NSH gives off-macOS); the host sheet file is still read+deleted per contract.

## Deviations from NSH (documented)

- Narrow layout keeps the sections popover instead of NSH's bottom Sheet (CUI sheet is a
  right-side panel; popover anchors better at all widths).
- `fp-agent-review` chip omitted: the agent feature is a cargo feature (off by default),
  same as NSH where `formUi.reviewing()` only fires under the agent.
- Print preview unavailable on Linux (matches NSH's non-macOS answer).
- Nav selected row uses text-color emphasis; NSH PressCell also has a selected fill.
