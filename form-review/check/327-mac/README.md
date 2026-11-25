# form-review/check/327-mac — independent live-app check of PR #327 on macOS

Checked by `buwiz-forms-macos-2` (an independent machine, not the PR author), live in the
running desktop app at exactly **d9028f45** (the PR head; it had not moved), demo taxpayer
Juana Santos only, scratch database, `BIR_SFTP_DRY_RUN=1` throughout. Nothing was queued or sent.

The comparison in test 6 was run at **origin/main 4399ce84** (the commit forms-validation named;
origin/main has since moved to 3aceafe8).

## What the files show

All PNGs are the app's own window only (`gpui-agent screenshot`, window capture), except the
ones noted as full-screen (taken with the desktop grabber when a native overlay — confirmation
dialog, toast — does not render into the window capture). No screen shows the app version number.

Naming: `<form>-<scenario>-<moment>.png`. `vt` = 1600VT, `pt` = 1600PT, `fq` = 1601FQ, `sp` = 2553.
`refbase` = refused tax base (amount typed, no ATC chosen). `refpick` = refused ATC pick
(WV010 while Item 3/10 unanswered). Forms: 1600VT Nov 2026 and Oct 2026 (the October return is
the one with Items 3/10 unanswered, used for refpick), 1600PT Nov 2026, 1601FQ Q4 2026, 2553 2026 #1.

### Control test on a valid row (1600VT, WV012 + 1,000.00)
- `vt-valid-x-before/after` — the drawer's X on a fully valid row: closes the drawer; the row stays
  listed "WV012 · 1,000.00 · 12.0 · 120.00"; save line "✓ Saved".
- `vt-valid-outside-before/after` — a click outside the drawer on a valid row: closes it cleanly.
- `vt-valid-esc-before` — before the Escape test. There is no "after" that differs: **Escape does
  not close the drawer on macOS even for a valid row** (pressed twice, focus in the field and in the
  drawer body). The X and the outside click are the working controls.

### 1600VT refused tax base (no ATC)
- `vt-refusedbase-open`, `vt-refbase-0state` — the refused state: "Choose this row's ATC first." in red,
  Done greyed out, the row not listed in the table, save line turned to "Unsaved changes".
- `vt-refbase-done-after` — after pressing Done: nothing; Done is disabled, drawer held open.
- `vt-refbase-x-before/after` — the X on the refused row: does not close (the same X that closes a valid row).
- `vt-refbase-escape-after` — after Escape twice: drawer unchanged (full-screen grab).
- `vt-refbase-outside-after` — after a real-mouse click on the form outside the drawer: held (full-screen grab).
- `vt-refbase-row-after` — after clicking another row in the list: drawer still on the refused row;
  both messages visible (full-screen grab).
- `vt-refbase-part-after`, `vt-refbase-back-after` — after clicking another part (sidebar) and the
  toolbar Back control: window unchanged. The form behind the drawer is inert — a blocking layer
  swallows clicks and even scroll-wheel events while a refusal is up.
- `vt-refbase-fix-done-after`, `vt-refbase-fixed-after` — mid-fix states. Note: picking the ATC makes
  the pre-typed refused amount drop out of the box; it has to be typed again before Done enables.
- `vt-refbase-fix-settled` — after fix + Done: drawer closed, row correctly listed, "✓ Saved".
- `vt-refbase-clear-settled` — after clearing the box + Done: empty row gone, "✓ Saved".
- `vt-refbase-delete-before/after/clean` — Delete row on the refused row: immediate, no confirmation,
  "✓ Saved".
- `vt-delete-confirm-dialog` — the "Clearing this ATC removes its row …" confirmation (belongs to
  clearing an ATC pick on a filled row, not to Delete row).
- `vt-reopen-after-windowclose` — after closing the window with cmd+W while the refusal was up,
  quitting, relaunching and reopening the return: only the valid row persists; no ghost of the
  refused row. (cmd+W itself closes the window at once — the drawer does not hold it; nothing
  invalid survives because the refused row was never persisted.)

### 1600VT refused ATC pick (WV010, Items 3/10 unanswered, Oct 2026 return)
- `vt-refpick-0state` — refusal "Selecting an ATC is not necessary when item no. 3 is set to ' NO '.",
  Done disabled, row not listed.
- `vt-refpick-x-after`, `vt-refpick-held` — X, Escape, outside click, Back all held (held = full-screen grab).
- `vt-refpick-delete-after` — Delete row closes cleanly, "✓ Saved".

### Payee non-refusal (1600VT, Schedule 1)
- `vt-payee-before-done` — a 57-character payee name: Done stays enabled, no refusal anywhere.
- `vt-payee-done-after` — Done closes the drawer normally, "✓ Saved".
- `vt-payee-check-report` — after pressing Check. Does not show an "at most 40 characters" message
  in the tree at that moment; the message itself was not confirmed either way — out of scope for the
  holding question, which is answered: the drawer is not held.

### 1600PT / 1601FQ refused tax base
- `pt-refbase-0state/held/outside-after/delete-settled` — same pattern verified: refusal shows,
  Done disabled, X/Escape/outside held, Delete row settles "✓ Saved".
- `fq-refbase-…` — same for 1601FQ (Q4 2026).

### 2553
- `sp-refbase-0state`, `sp-refbase-held` — refusal "Choose the ATC in 18B first."; same four ways held.
- `sp-valid-row18-saved` — a valid row 18 (OT011, 701.00, 5.0), saved.
- `sp-row18-delete-toast` — after Delete row + the confirmation "Clearing row 18 removes its entry
  (OT011, ₱701.00). Continue?" + Continue: toast "Removed row 18." with Undo — **but the row is
  still listed** as "18 701.00 · 5.0" (classification/ATC stripped, amount and rate remain) and the
  page settles on "✓ Saved". Full-screen grab.
- `sp-row18-zombie`, `sp-row18-zombie-open` — the zombie row in the table; it is clickable and
  reopens in the drawer.
- `sp-row18-zombie-MAIN` — **the same zombie at origin/main 4399ce84**: the anomaly pre-exists;
  PR #327 did not cause it. Full-screen grab.
- (After quitting and relaunching, the zombie is gone — it is a display-layer artifact, not persisted.)

### Videos (window-only, H.264 .mp4, under two minutes)
- `vt-flow.mp4` (51 s) — 1600VT canonical arc: Add row, type a tax base with no ATC, the refusal
  appears; Done, the X, Escape and an outside click each leave the drawer open with the reason;
  then the documented way out (clear the box, Done) closes cleanly. Recorded with ffmpeg cropped
  exactly to the window rectangle (1200x800) — only the app's own window is in frame.
- `pt-flow.mp4` (110 s) — 1600PT: refused-base creation, ways out held, Delete-row exit, settle.
- `fq-flow.mp4` (110 s) — 1601FQ: same arc.
- `sp-flow.mp4` (79 s) — 2553: refused-base creation and ways out held (the recording stops before
  the delete arc; the delete/zombie sequence is documented in the PNGs above).

## What the files do NOT show
- No real taxpayer data anywhere; invented amounts only.
- The videos are continuous single takes but not scripted; pauses are real thinking time of the
  driver. Nothing was edited out; the sp recording simply ends early.
- The print preview is never shown: during a refusal it cannot be reached at all (the blocking
  layer swallows the clicks and scrolling needed to reach fp-print, and there is no menu-bar print item).
- Escape key behavior is a keyboard event; only the resulting state is visible.
- The exact pixel positions of clicks are not visible in stills; where it matters (outside click,
  Back control) the full-screen grabs show the unchanged state after the click.

## Screen test counts
See the report message (rowDrawer.test.tsx plus the form screen tests run headless at d9028f45).
