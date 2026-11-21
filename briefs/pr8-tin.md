# PR 8 `feat/form-fields-tin`: TIN boxes fill the row on a phone (Q8 = P8a)

Standing rules: briefs/00-rules.md. Worktree: buwiz-forms-tin (branch feat/form-fields-tin, from feat/form-fields-gated; I create
it). Mockup: `MOCKS.P8a`. PLAN decision 10 (keyboard: number-pad).

## What changes (rows.tsx `isTinItem` branch of `Controls`, and the kit box it uses)
- The TIN stays FOUR boxes (3-3-3 digits and the branch code), same keys, same values.
- Under 600 pt: the four boxes are one grid that fills the row, columns 1 : 1 : 1 : 1.4, gap 6 pt, no dashes between them, each box
  44 pt tall (touch), digits centred, tabular figures. No horizontal overflow at 320–420 pt.
- 600 pt and wider: as today (fixed 58 / 58 / 58 / 76 pt boxes with the dashes).
- A TIN the tax profile fills (locked boxes) shows ONE line under the boxes: the lock icon and "From the tax profile" (meta, subtle
  tone), on both widths; the per-row profile mark/tooltip that says the same thing today goes if it would repeat it (one statement,
  not two). A typed TIN (`it.typed_tin`, another party's: a donee, a seller, a spouse) has no such line and takes typing: number-pad
  keyboard, digits only, `maxLength` per box; no auto-advance (that was the pick not chosen).
- A TIN item that is not four boxes (three boxes, or one box holding the whole TIN: enumerate over the 43 built schemas) keeps the
  same rules with its own column count (equal columns; the last one 1.4 only when it is the 5-digit branch code).
- The box sentences of a TIN (PR 1: printed by the row) stay one line under the grid.

## Tests first
- controls/page tests at 320, 390 and 1440: the four boxes' widths follow 1:1:1:1.4 and sum to the row's inner width on a phone (read
  widths if the test API can; else assert the layout props and let the shots carry the pixels); dashes only on desktop; the "From the
  tax profile" line only for a locked TIN; a typed TIN takes digits only and keeps its four keys (fieldKinds.test.ts kind "tin": type,
  paste with dashes/spaces/letters, clear, save/reopen, print mapping).
- A sweep over all 43 forms: every `isTinItem` row is one of the listed shapes.
- Update tests pinned to the 58/76 widths on a phone; say which.

## Shots: nsh/shots/fftin.test.tsx, 320x700, 420x900 and 1280x900, light and dark; BEFORE from PR 7's tip
0605 Item 9 (locked); a typed TIN (1800 donee or 1706 buyer/seller) empty and filled; a TIN row with a refusal sentence; a non-4-box
TIN if any. Row height of 0605 Item 9 at 420 and 640 pt before/after.

## Gate: per brief while iterating; the full gate once at the end.
