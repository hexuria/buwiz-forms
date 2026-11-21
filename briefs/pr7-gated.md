# PR 7 `feat/form-fields-gated`: gated boxes are hidden until the answer that opens them (Q9 = G1)

Standing rules: briefs/00-rules.md. Worktree: buwiz-forms-gated (branch feat/form-fields-gated, from feat/form-fields-rows; I create
it). Mockup: `MOCKS.G1` (`gated(el,"hide")`). PR 1 already made the host refuse a write to a shut box with its reason (B16).

## What changes (page only; the host's conditions, `off` reasons and refusals are unchanged)
A box "another answer keeps shut" is one whose `box.off()` is non-empty (the session's `s.off[k]`, from the host's conditions:
"Needs item 17 = Others").
- Such a box is NOT drawn, and neither is its "Needs …" line. When every box of a row is shut the whole row is not drawn (its number
  and title too); the part's "N items" count and the side list count the rows drawn. A schedule table whose every cell is shut is not
  drawn; single shut cells inside a drawn table stay as today (a table keeps its grid).
- When the opening answer is given the box appears where the schema has it: a box of the same item directly under the group/box that
  opens it (PR 5 already orders "Others (Specify)" under its group); a box of another item in its own row, which is right after the
  opener on the printed forms. The page does not move focus by itself; the next Tab / pill step reaches the new box.
- Safety rule (nothing on the return may be invisible): a shut box that HOLDS a non-blank value (an old draft, an import, an agent
  write from before B16) stays drawn as today (faded, with its "Needs …" line) until it is cleared or opened. Also drawn: a shut box
  that carries a shown message or a to-do, so a sentence never points at nothing.
- The to-do pill, the part counters and Check never count a hidden box (verify `todoNow` / `isShut`; a hidden required box is not
  required until opened — the host already says so; assert it).
- Print, the official XML and the stored draft are unchanged (assert on one form).
- Reactive rule: hiding is an `If`/accessor on the row's signals, read in JSX; no body `const` copies. A row that appears must not
  remount its neighbours (the session's per-row signals already isolate rows: keep that).

## Tests first
- Session: for each gate shape — a text box opened by a radio choice (0605 Item 17 Others → Specify), a group opened by Yes/No
  (0619E Item 4 → Item 14; 2551Q Item 12 → specify), a whole row, a whole part if one exists — `row(id)()` says hidden/shown as the
  opener flips; counts follow; a shut box with a stored value stays shown.
- A sweep over all 43 forms on a new return: every box with an `off` reason and no value is hidden; no hidden box is required in the
  to-do; every opener named in an `off` reason exists and is drawn (so nothing is unreachable) — a box whose opener is itself hidden
  appears once the chain is answered (assert one two-level chain if any form has one; list them).
- Page tests at 390 and 1440: the box's testID does not exist until the opener is picked, then exists right under it; item counts.
- Rust: none new unless the sweep finds a condition with no drawn opener (then report it, do not hide that box).
- Update tests that read a faded box or a "Needs …" line; say which.

## Shots: nsh/shots/ffgated.test.tsx, 420x900 and 1280x900, light and dark; BEFORE from PR 6's tip
0605 Item 17 (closed; Others picked → Specify under it); 0619E Item 4 No → Yes (Item 14 appears); 2551Q Item 12; a part whose rows
were mostly shut (count before/after); a shut box holding a value (stays, faded). Part heights and item counts before/after at 420 and
640 pt.

## Gate: per brief while iterating; the full gate once at the end.
