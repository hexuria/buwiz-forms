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

## Measuring

Each UI change that touches form state must keep or extend these tests. They
mount the real `form_page.js` with mocked GPUI and count which `fp-item-*`
rows rebuild:

    node --experimental-vm-modules tools/test_form_row_invalidation.mjs
    node --experimental-vm-modules tools/test_date_field_rows.mjs

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
