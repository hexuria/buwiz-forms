# Goal: port all 43 BIR forms to the CUI app, pixel- and behavior-complete

You are working in `/home/agent/workspace/buwiz-forms`, specifically the `cui/` crate
(`cui-app-ebirforms`, a native Rust port of the NSH/TSX app). The NSH reference app lives in the
sibling `nsh/` directory and is **read-only** — never edit it. Your job is to make every one of
the 43 BIR forms open, render, edit, validate, save, schedule, print, and file in the CUI app
with the same UX and behavior as the NSH app.

## Current state

- `cui/src/form/` has the ported engine: `schema.rs` (FormSchema model), `session.rs` (open/edit/
  save/validate/step lifecycle), `model.rs`, `mod.rs`.
- `cui/src/screens/form.rs` renders the form page.
- Clicking a form in the library currently shows **"This form has no on-screen layout."** for
  most forms — `session.rs` ~line 615 calls `bir::call1("screen_form_schema", form_code)` and
  sets that error when the call fails OR `serde_json::from_value::<FormSchema>` rejects the
  payload. First audit which of the two it is: the host function exists
  (`crates/bir-app-api/src/screen_form_module.rs`, wrapped in `nsh/bir/functions.ts`), so either
  it errors for layout-less forms (match NSH's fallback) or our `FormSchema` model in
  `schema.rs` can't parse schemas that use field/section kinds we haven't modeled — in that
  case extend the model until every schema the host emits parses.
- The schema/layout data comes from the Rust engine through the same door the NSH app uses
  (`bir::call*` / `crate::bir`). NSH gets it via `bir.screen_form_schema`-style calls; find the
  exact host function names in `cui/src/bir.rs` and in `nsh/bir/functions.ts` /
  `nsh/screens/form/session.ts`.
- NSH's form UI is `nsh/screens/form/` — `FormPage.tsx`, `rows.tsx`, `formUi.ts`, `contract.ts`,
  `controls/` (FieldControl, Layout, Pick, Radios, Schedule, model.ts, types.ts). This is the
  behavioral and visual reference. There are also `*.test.tsx` files (`all43.test.ts`,
  `FormPage*.test.tsx`) that encode expected behavior — read them.

## Hard requirements — nothing gets left behind

1. **All 43 forms.** Enumerate the authoritative list first (form index / `per_year_forms` /
   `library` data — the same source the Form library screen uses). Produce a tracking table
   (form code → schema source → controls used → status → verified). No form may be skipped,
   stubbed, or left at "no on-screen layout" without an explicit, documented reason.

2. **Schema/layout pipeline.** If schemas are missing for some forms, find where NSH/old-app
   gets them (host functions, bundled schema JSON, generated layouts). Port or call the same
   source — do not invent layouts. If a form genuinely has no layout upstream, document it and
   match whatever NSH does for that case.

3. **Full control parity.** Every field type NSH supports must render and behave identically:
   text/number/currency/date pickers, masked inputs, selects, radios/checkboxes, repeating
   rows/schedules, calendars, computed/readonly fields, add/remove row, per-field validation
   messages, max-length rules, required markers, help text/tooltips.

4. **Session lifecycle parity.** Open (new/draft/saved/filed), autosave, dirty tracking,
   validation on step/finish, step navigation (prev/next, jump list if NSH has one), print
   preview, schedules (add/edit/remove), due-date/calendar integration, filing/submit flow,
   cancel-queue, and the todo/checklist features — all must work per form.

5. **Visual parity.** Match NSH layout: section/row structure, labels, hints, alignment
   (`RowAlign`), spacing tokens, colors from the theme palette, narrow/wide breakpoints,
   print stylesheet behavior. Use `cui/src/ui/kit.rs` primitives and `icons::glyph` for icons —
   never pass icon names where glyphs are expected (that bug class already bit us twice).

6. **State correctness.** CUI is immediate-mode: all form state lives in `FormSession`/thread-
   locals outside one frame; reads and writes must survive frames. Input buffers need reserved
   capacity (see the earlier input-corruption fix in `kit.rs`). No per-frame state resets.

## Working method (required)

- **One form family at a time**, but keep a persistent tracking file (e.g. `cui/FORM-PORT.md`
  or a checklist in the todo list) updated after every form: code, controls used, quirks,
  verification status.
- For each form: read the NSH screen/control code AND its tests, implement, then verify live
  on display `:1` (the dev app is launched via `cui/run-dev.sh`; the window is viewable
  through Voxl/noVNC). Take screenshots for visual comparison where possible.
- Reuse, don't duplicate: shared widgets (pickers, row editors, schedule grids, masks) go in
  `cui/src/form/` or `cui/src/ui/kit.rs` once, then every form benefits.
- Run `cargo check`, `cargo test` (208 tests currently green — keep them green), `cargo fmt`
  after each batch. Fix warnings you introduce.
- Test data: the dev DB has Juana Santos (`000-123-453-00000`) seeded with 1701/1701Q/2550Q/1601C
  etc. Use `BIR_SHELL_START='tp=000123453;branch=00000;theme=dark'` style env or the library to
  reach forms. `BIR_AUTH_BYPASS=1 EBIR_TEST_ENV=1 BUWIZ_NO_KEYCHAIN=1 BIR_SFTP_DRY_RUN=1` are
  already set by `run-dev.sh`.

## Definition of done

- All 43 forms open from the library with real layouts (no "no on-screen layout" errors except
  documented upstream-missing ones).
- Each form: fields render per schema, editing works, validation fires, save/autosave works,
  print preview works, schedules work where the form has them.
- Tracking table shows every form verified live on `:1`.
- `cargo test` green, `cargo fmt` clean, no new warnings.
- A final report listing per-form status and any deviations from NSH with reasons.

Start by enumerating the 43 forms and auditing which already render vs. which hit the
"no on-screen layout" path — that audit IS the first deliverable.
