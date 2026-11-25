# PR #327 row-drawer fix evidence

This directory contains live-application evidence for the row-drawer refusal gate.

- `before/` is the pre-fix application at commit `c7fd0dada098a67e393f23f0e0833bf3b8f1835d`.
- `after/` is the fixed application at commit `d9028f459cb6951e48f94bfe5bbf523a037745ec`.
- The taxpayer is the demo taxpayer Juana Santos; all amounts and row values are invented. Queue was never pressed and live sending was never enabled.

## How to read the states

In the fixed `after/` way-out matrix, except for the impossible 2553 other-row case described below, each `-before.png` shows a schedule-row drawer open after typing a tax base/taxable amount with no ATC chosen. Each `-after.png` is immediately after the named way-out attempt. In the passing fixed behavior the drawer is still open, the refusal is visible, Done is disabled, the refused row is not listed, and there is no row count for refused content. Each `-reopen.png` is after clearing the refused text, closing with Done, letting the page settle on `✓ Saved`, then closing and reopening the return; the refused row is absent.

The X and Escape images on Linux must be read narrowly: physical X/key delivery on this Linux/GPUI window is unreliable even for clean rows, so those images prove what the live window did, not that the input certainly reached the control. They are not continuous-interaction proof of the gate by themselves.

The `.mp4` recordings are assembled from `gpui-agent` screenshots of the app surface at 1 fps. X11/desktop capture cannot see this GPUI window on the test machine. They are therefore app-only, low-frame-rate visual evidence, not literal continuous desktop recordings; sub-second transitions can be absent.

## after

Fixed application at `d9028f45`.

- `fq-done-after.png` — 1601FQ: state immediately after attempting the Done button.
- `fq-done-before.png` — 1601FQ: refused state before attempting the Done button; drawer open with the reason visible.
- `fq-done-reopen.png` — 1601FQ: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `fq-flow-refusal.png` — 1601FQ flow: the refused row state, with the drawer open, the refusal visible, Done disabled, and no row line/count for the refused content.
- `fq-flow-saved.png` — 1601FQ flow after the documented way out: the refused text was cleared, Done closed the empty row, and the save line settled on ✓ Saved.
- `fq-flow.mp4` — 1601FQ flow recording: open the return, add a schedule row, type an amount with no ATC, try unavailable Done, clear the box and close with Done, ending on ✓ Saved. Assembled from app-surface still frames at 1 fps; not continuous desktop video.
- `fq-otherrow-after.png` — 1601FQ: state immediately after attempting opening another already-valid row.
- `fq-otherrow-before.png` — 1601FQ: refused state before attempting opening another already-valid row; drawer open with the reason visible.
- `fq-otherrow-reopen.png` — 1601FQ: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `fq-outside-after.png` — 1601FQ: state immediately after attempting a click outside the drawer.
- `fq-outside-before.png` — 1601FQ: refused state before attempting a click outside the drawer; drawer open with the reason visible.
- `fq-outside-reopen.png` — 1601FQ: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `pt-done-after.png` — 1600PT: state immediately after attempting the Done button.
- `pt-done-before.png` — 1600PT: refused state before attempting the Done button; drawer open with the reason visible.
- `pt-done-reopen.png` — 1600PT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `pt-flow-refusal.png` — 1600PT flow: the refused row state, with the drawer open, the refusal visible, Done disabled, and no row line/count for the refused content.
- `pt-flow-saved.png` — 1600PT flow after the documented way out: the refused text was cleared, Done closed the empty row, and the save line settled on ✓ Saved.
- `pt-flow.mp4` — 1600PT flow recording: open the return, add a schedule row, type an amount with no ATC, try unavailable Done, clear the box and close with Done, ending on ✓ Saved. Assembled from app-surface still frames at 1 fps; not continuous desktop video.
- `pt-otherrow-after.png` — 1600PT: state immediately after attempting opening another already-valid row.
- `pt-otherrow-before.png` — 1600PT: refused state before attempting opening another already-valid row; drawer open with the reason visible.
- `pt-otherrow-reopen.png` — 1600PT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `pt-outside-after.png` — 1600PT: state immediately after attempting a click outside the drawer.
- `pt-outside-before.png` — 1600PT: refused state before attempting a click outside the drawer; drawer open with the reason visible.
- `pt-outside-reopen.png` — 1600PT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `s3-done-after.png` — 2553: state immediately after attempting the Done button.
- `s3-done-before.png` — 2553: refused state before attempting the Done button; drawer open with the reason visible.
- `s3-done-reopen.png` — 2553: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `s3-flow-refusal.png` — 2553 flow: the refused row state, with the drawer open, the refusal visible, Done disabled, and no row line/count for the refused content.
- `s3-flow-saved.png` — 2553 flow after the documented way out: the refused text was cleared, Done closed the empty row, and the save line settled on ✓ Saved.
- `s3-flow.mp4` — 2553 flow recording: open the return, add a schedule row, type an amount with no ATC, try unavailable Done, clear the box and close with Done, ending on ✓ Saved. Assembled from app-surface still frames at 1 fps; not continuous desktop video.
- `s3-otherrow-after.png` — 2553 other-row impossibility evidence: unchanged because there was no second row to refuse and no different row in the same list to press. This is not a passed matrix case.
- `s3-otherrow-before.png` — 2553 other-row impossibility evidence: the one refusal-capable row 18 is valid and filled; the table exposes no Add row control for a second row, so the refused-second-row prerequisite cannot be created.
- `s3-otherrow-reopen.png` — 2553 other-row impossibility evidence after close/reopen: the valid row 18 remains; no refused second row ever existed.
- `s3-outside-after.png` — 2553: state immediately after attempting a click outside the drawer.
- `s3-outside-before.png` — 2553: refused state before attempting a click outside the drawer; drawer open with the reason visible.
- `s3-outside-reopen.png` — 2553: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `vt-back-after.png` — 1600VT: state immediately after attempting the app Back control.
- `vt-back-before.png` — 1600VT: refused state before attempting the app Back control; drawer open with the reason visible.
- `vt-back-reopen.png` — 1600VT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `vt-done-after.png` — 1600VT: state immediately after attempting the Done button.
- `vt-done-before.png` — 1600VT: refused state before attempting the Done button; drawer open with the reason visible.
- `vt-done-reopen.png` — 1600VT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `vt-escape-after.png` — 1600VT: state immediately after attempting the Escape key.
- `vt-escape-before.png` — 1600VT: refused state before attempting the Escape key; drawer open with the reason visible.
- `vt-escape-reopen.png` — 1600VT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `vt-flow-refusal.png` — 1600VT flow: the refused row state, with the drawer open, the refusal visible, Done disabled, and no row line/count for the refused content.
- `vt-flow-saved.png` — 1600VT flow after the documented way out: the refused text was cleared, Done closed the empty row, and the save line settled on ✓ Saved.
- `vt-flow.mp4` — 1600VT flow recording: open the return, add a schedule row, type an amount with no ATC, try unavailable Done, clear the box and close with Done, ending on ✓ Saved. Assembled from app-surface still frames at 1 fps; not continuous desktop video.
- `vt-otherrow-after.png` — 1600VT: state immediately after attempting opening another already-valid row.
- `vt-otherrow-before.png` — 1600VT: refused state before attempting opening another already-valid row; drawer open with the reason visible.
- `vt-otherrow-reopen.png` — 1600VT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `vt-outside-after.png` — 1600VT: state immediately after attempting a click outside the drawer.
- `vt-outside-before.png` — 1600VT: refused state before attempting a click outside the drawer; drawer open with the reason visible.
- `vt-outside-reopen.png` — 1600VT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `vt-partnav-after.png` — 1600VT: state immediately after attempting a Part-navigation click.
- `vt-partnav-before.png` — 1600VT: refused state before attempting a Part-navigation click; drawer open with the reason visible.
- `vt-partnav-reopen.png` — 1600VT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `vt-x-after.png` — 1600VT: state immediately after attempting the drawer X control.
- `vt-x-before.png` — 1600VT: refused state before attempting the drawer X control; drawer open with the reason visible.
- `vt-x-reopen.png` — 1600VT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.

## before

Pre-fix application at `c7fd0dad`. The matrix PNGs copied from the original check are supplemented here with the missing 1600PT/1601FQ/2553 outside-click and other-row cases.

- `fq-done-after.png` — 1601FQ: state immediately after attempting the Done button.
- `fq-done-before.png` — 1601FQ: refused state before attempting the Done button; drawer open with the reason visible.
- `fq-escape-after.png` — 1601FQ: state immediately after attempting the Escape key.
- `fq-escape-before.png` — 1601FQ: refused state before attempting the Escape key; drawer open with the reason visible.
- `fq-flow-refusal.png` — 1601FQ flow: the refused row state, with the drawer open, the refusal visible, Done disabled, and no row line/count for the refused content.
- `fq-flow-saved.png` — 1601FQ flow after the documented way out: the refused text was cleared, Done closed the empty row, and the save line settled on ✓ Saved.
- `fq-otherrow-after.png` — 1601FQ: state immediately after attempting opening another already-valid row.
- `fq-otherrow-before.png` — 1601FQ: refused state before attempting opening another already-valid row; drawer open with the reason visible.
- `fq-otherrow-reopen.png` — 1601FQ: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `fq-outside-after.png` — 1601FQ: state immediately after attempting a click outside the drawer.
- `fq-outside-before.png` — 1601FQ: refused state before attempting a click outside the drawer; drawer open with the reason visible.
- `fq-outside-reopen.png` — 1601FQ: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `fq-x-after.png` — 1601FQ: state immediately after attempting the drawer X control.
- `fq-x-before.png` — 1601FQ: refused state before attempting the drawer X control; drawer open with the reason visible.
- `pt-done-after.png` — 1600PT: state immediately after attempting the Done button.
- `pt-done-before.png` — 1600PT: refused state before attempting the Done button; drawer open with the reason visible.
- `pt-escape-after.png` — 1600PT: state immediately after attempting the Escape key.
- `pt-escape-before.png` — 1600PT: refused state before attempting the Escape key; drawer open with the reason visible.
- `pt-flow-refusal.png` — 1600PT flow: the refused row state, with the drawer open, the refusal visible, Done disabled, and no row line/count for the refused content.
- `pt-flow-saved.png` — 1600PT flow after the documented way out: the refused text was cleared, Done closed the empty row, and the save line settled on ✓ Saved.
- `pt-otherrow-after.png` — 1600PT: state immediately after attempting opening another already-valid row.
- `pt-otherrow-before.png` — 1600PT: refused state before attempting opening another already-valid row; drawer open with the reason visible.
- `pt-otherrow-reopen.png` — 1600PT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `pt-outside-after.png` — 1600PT: state immediately after attempting a click outside the drawer.
- `pt-outside-before.png` — 1600PT: refused state before attempting a click outside the drawer; drawer open with the reason visible.
- `pt-outside-reopen.png` — 1600PT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `pt-x-after.png` — 1600PT: state immediately after attempting the drawer X control.
- `pt-x-before.png` — 1600PT: refused state before attempting the drawer X control; drawer open with the reason visible.
- `s3-done-after.png` — 2553: state immediately after attempting the Done button.
- `s3-done-before.png` — 2553: refused state before attempting the Done button; drawer open with the reason visible.
- `s3-escape-after.png` — 2553: state immediately after attempting the Escape key.
- `s3-escape-before.png` — 2553: refused state before attempting the Escape key; drawer open with the reason visible.
- `s3-flow-refusal.png` — 2553 flow: the refused row state, with the drawer open, the refusal visible, Done disabled, and no row line/count for the refused content.
- `s3-flow-saved.png` — 2553 flow after the documented way out: the refused text was cleared, Done closed the empty row, and the save line settled on ✓ Saved.
- `s3-otherrow-after.png` — 2553 other-row impossibility evidence: unchanged because there was no second row to refuse and no different row in the same list to press. This is not a passed matrix case.
- `s3-otherrow-before.png` — 2553 other-row impossibility evidence: the one refusal-capable row 18 is valid and filled; the table exposes no Add row control for a second row, so the refused-second-row prerequisite cannot be created.
- `s3-otherrow-reopen.png` — 2553 other-row impossibility evidence after close/reopen: the valid row 18 remains; no refused second row ever existed.
- `s3-outside-after.png` — 2553: state immediately after attempting a click outside the drawer.
- `s3-outside-before.png` — 2553: refused state before attempting a click outside the drawer; drawer open with the reason visible.
- `s3-outside-reopen.png` — 2553: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `s3-x-after.png` — 2553: state immediately after attempting the drawer X control.
- `s3-x-before.png` — 2553: refused state before attempting the drawer X control; drawer open with the reason visible.
- `vt-back-after.png` — 1600VT: state immediately after attempting the app Back control.
- `vt-back-before.png` — 1600VT: refused state before attempting the app Back control; drawer open with the reason visible.
- `vt-back-reopen.png` — 1600VT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `vt-done-after.png` — 1600VT: state immediately after attempting the Done button.
- `vt-done-before.png` — 1600VT: refused state before attempting the Done button; drawer open with the reason visible.
- `vt-done-reopen.png` — 1600VT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `vt-escape-after.png` — 1600VT: state immediately after attempting the Escape key.
- `vt-escape-before.png` — 1600VT: refused state before attempting the Escape key; drawer open with the reason visible.
- `vt-escape-reopen.png` — 1600VT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `vt-flow-refusal.png` — 1600VT flow: the refused row state, with the drawer open, the refusal visible, Done disabled, and no row line/count for the refused content.
- `vt-flow-saved.png` — 1600VT flow after the documented way out: the refused text was cleared, Done closed the empty row, and the save line settled on ✓ Saved.
- `vt-otherrow-after.png` — 1600VT: state immediately after attempting opening another already-valid row.
- `vt-otherrow-before.png` — 1600VT: refused state before attempting opening another already-valid row; drawer open with the reason visible.
- `vt-otherrow-reopen.png` — 1600VT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `vt-outside-after.png` — 1600VT: state immediately after attempting a click outside the drawer.
- `vt-outside-before.png` — 1600VT: refused state before attempting a click outside the drawer; drawer open with the reason visible.
- `vt-outside-preview-not-open.png` — Diagnostic original-check capture: print preview did not open while the escaped refused row left the return unsaved.
- `vt-outside-reopen.png` — 1600VT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `vt-partnav-after.png` — 1600VT: state immediately after attempting a Part-navigation click.
- `vt-partnav-before.png` — 1600VT: refused state before attempting a Part-navigation click; drawer open with the reason visible.
- `vt-partnav-reopen.png` — 1600VT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `vt-x-after.png` — 1600VT: state immediately after attempting the drawer X control.
- `vt-x-before.png` — 1600VT: refused state before attempting the drawer X control; drawer open with the reason visible.
- `vt-x-reopen.png` — 1600VT: the same return closed and reopened after the way-out sequence; used to check whether the refused row persisted.
- `vt2-done-after.png` — 1600VT refused ATC-pick variant: state immediately after attempting the Done button.
- `vt2-done-before.png` — 1600VT refused ATC-pick variant: refused state before attempting the Done button; drawer open with the reason visible.
- `vt2-escape-after.png` — 1600VT refused ATC-pick variant: state immediately after attempting the Escape key.
- `vt2-escape-before.png` — 1600VT refused ATC-pick variant: refused state before attempting the Escape key; drawer open with the reason visible.
- `vt2-x-after.png` — 1600VT refused ATC-pick variant: state immediately after attempting the drawer X control.
- `vt2-x-before.png` — 1600VT refused ATC-pick variant: refused state before attempting the drawer X control; drawer open with the reason visible.
- `vt3-done-after.png` — 1600VT non-refusal validation-message variant: state immediately after attempting the Done button.
- `vt3-done-before.png` — 1600VT non-refusal validation-message variant: refused state before attempting the Done button; drawer open with the reason visible.
- `vt3-escape-after.png` — 1600VT non-refusal validation-message variant: state immediately after attempting the Escape key.
- `vt3-escape-before.png` — 1600VT non-refusal validation-message variant: refused state before attempting the Escape key; drawer open with the reason visible.
- `vt3-x-after.png` — 1600VT non-refusal validation-message variant: state immediately after attempting the drawer X control.
- `vt3-x-before.png` — 1600VT non-refusal validation-message variant: refused state before attempting the drawer X control; drawer open with the reason visible.
