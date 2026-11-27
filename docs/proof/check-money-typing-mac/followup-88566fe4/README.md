# money-box follow-up at 88566fe4 — macOS real-key proof

App: hexuria/buwiz-forms fix/money-box-follow-up at exactly
`88566fe43c29811343eb73f4b642da0d1c4e6a44` (rev-parse confirmed), fresh clone,
`CARGO_TARGET_DIR` inside it, kit sibling at pin 8c78cfd6, cargo 1.100.0,
`RUSTC_WRAPPER` empty, the four safety variables, never `BIR_SFTP_LIVE`, Queue
never pressed. Demo taxpayer juana santos (000-123-453), invented values, real
window, real hardware-key events, saved values read from the draft database
(`BIR_DATABASE_PATH` pinned so the quit/reopen round-trips one database).

## (1) F1 — uncommitted typing settles on quit and on page leave: PASS both

- Cmd+Q: 2551Q Schedule 1 row 1 amount focused holding `1500,50` (verified in
  the box, no Tab, no click elsewhere), real Cmd+Q key events — the app quit.
  Reopened on the same database: the draft's row 1 holds `taxable_amount`
  **1500.5** (and `tax_due` recomputed to 45.02).
- Back: same box refilled to `2700,25` uncommitted, clicked the window's Back
  without leaving the box: the draft holds **2700.25**.

## (2) the comma note: PASS

Into the zeroed box: typed `1500,50`, Tab. `shot-note-grey.png` + magnified
`crop-note.png`: under the box, in grey (not red, the box has no red border),
word for word: **"Taxable Amount: 1500,50 was read as 1,500.50."** The draft
holds 1500.5. Typed into the box again: the note is gone at once
(`crop-note-gone.png` — only the bare Tax Rate row under the box).

## (3) Z1 — unreadable in a blank schedule cell: MISMATCH

A new Schedule 1 row's untouched empty amount: typed `abc` (box showed `abc`),
Tab away. `crop-z1.png` (full shot `shot-z1.png`): the box gets a RED border
and red text word for word: **"Taxable Amount: abc is not an amount. The box
kept 0.00."** — NOT the ordered "The box was left blank." The box after
leaving reads empty; the draft still holds only one row (the new row never
persisted). Reading `nsh/ui/fields.tsx` at this head: the blank wording only
appears when the fallback amount is blank AND the host did not answer the
box's empty report with its zero — "a started row's cell" holds that zero, so
a started row's amount is reported as kept 0.00. On the Mac the new row's
empty amount takes the started-row path.
