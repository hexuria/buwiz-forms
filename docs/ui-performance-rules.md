# UI performance rules (shell form pages)

The shell UI (app/shell/*.js on gpui-kit, GPUI Fast) stays fast only if a
change redraws what it changed and nothing else. These rules are mandatory
for every change to app/shell, and reviewers should reject code that breaks them.

## Rules

1. **Never notify the whole app view for per-field state.** Typing, focus,
   a picker opening or closing, a dropdown, a validation badge or a spinner
   must not call `app.invalidate()` or the page-level `notify()` that rebuilds
   every row. Use the row's own `cx.notify()` or the owning entity's
   `set_props`.
2. **One retained view per row or control that changes on its own.** Form
   rows are `FormItemView` entities kept in `state.rowViews`. Status controls
   that change while you work (for example the Check button spinner) get
   their own small entity, like `FormCheckButton`. Release them when the
   return closes or reloads.
3. **Rebuild only changed rows.** `rowSnapshot()` lists everything a row
   displays: value, lock, profile flag, options, message, open picker and
   width. `refreshRows()` calls `set_props` only when that snapshot changes.
   When you add row-visible state, such as `dateErrors`, add it to the
   snapshot. Do not bypass the snapshot by marking every row dirty.
4. **Debounce validation and draft writes.** Keystrokes go through `edit()`,
   which stages edits and flushes after `VALIDATE_DELAY`. Live preview sync
   uses `LIVE_PREVIEW_DELAY`. Never call the host (`screen_form_fill` or
   `screen_form_validate`) per keystroke.
5. **No "Checking…" flash for fast validations.** Validation progress lives
   inside the existing Check button's icon slot, with a stable label and no
   layout shift. No banner or chip appears in the header.
6. **No layout shift from transient state.** Spinners, disabled states and
   icons swap inside fixed-size slots. The 18 px validation line is always
   reserved.
7. **Host objects are not change signals.** A fresh `values` object from the
   host must not invalidate a row whose displayed values are equal. Compare
   snapshots instead.
8. **Profile-derived and computed boxes are read-only.** They render with
   `lockedBox`, have no `InputState` change handler that writes, and never
   trigger a fill.
9. **Long scrolling columns are lazy `list`s, and a lazy item renders only
   chrome plus retained entities.** The form page's parts column is
   `list("fp-parts", …)` driven by a flat, keyed item stream (page head, chip
   navigator, `h:<part>` headers, `g:<part>` profile banners, `r:<part>:<item>`
   rows) — scrolling materializes only what is on screen. An item renderer
   runs during layout, so it must not create entities, `InputState`s, focus
   handles or click handlers; anything with handlers is a retained `View`
   (`FormTopView`, `FormNavView`, `FormItemView`, `FormGroupView`) created when
   the part opened, embedded by the renderer. Header actions dispatch through
   the list's `on_item_click` by key. Keys must stay stable and domain-derived
   so a fold changes which items exist, never what an item is.

## Measuring

Each UI change that touches form state must keep or extend these tests. They
mount the real `form_page.js` with mocked GPUI and count which `fp-item-*`
rows rebuild:

    node --experimental-vm-modules tools/test_form_row_invalidation.mjs
    node --experimental-vm-modules tools/test_date_field_rows.mjs

The lazy parts list has its own guard — the item stream's keys, the fold
behaviour and the "no retained state inside a renderer" rule:

    node --experimental-vm-modules tools/test_form_virtualized_parts.mjs

Runtime baselines come from `BIR_SHELL_METRICS=1` on the release shell
(materializations per active second). Form 1702MX, all 17 parts open
(198 rows), 1440×900, a `scroll_to_item` sweep ticking ~25×/s — the same
per-tick invalidate a wheel event produces:

| Path | Materializations/s | Materialize ms/s | Animation hitches (xctrace, ~16 s window) |
|---|---|---|---|
| Rebuilt parts column (before) | ~1,400 | ~98 | 10, worst 117 ms; 12 frames >33 ms |
| Lazy `list` + retained rows (after) | ~8–110, only rows entering view | ~1–8 | 0 |

The same sweep on 1701 (all 14 parts open, 946 boxes): ~1,700–2,300
materializations/s before, ~70–120 after. A hover crossing a row boundary
re-renders only that row's retained view, never the page.

Current baselines:

| Interaction | Row rebuilds | App view |
|---|---|---|
| 6 radio toggles (1601C category) | 6 (one row) | 0 |
| 8 keystrokes into 1700 Item 9 date | 0 (typing stays in the input) | 0 |
| Blur, invalid date or picked date | 1 (date row only) | 0 |
| Calendar open / close | 1 / 1 (date row only) | 0 |
| Validation spinner | 0 rows | 0 |

A new interaction should come with an assertion like
`assert.deepEqual([...new Set(traces)], [row])`. When an assertion fails,
fix the invalidation. Don't loosen the test.
