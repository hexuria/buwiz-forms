# Check: the upload text #323 writes for its three named samples — CORRECTED

Independent check of `feat/store-launch-locked-inputs` at its merged head
`4efeca2d` (the merge of origin/main `92b3e201`). Report-only branch; nothing
here changes the pull request.

**CORRECTION (same day): the first version of this proof reported 2552
items 17/19/20/25 and 2200A's others-row due/schedule total as differing
from the official. Those differences are withdrawn: they were an artifact
of this machine's build setup, not of the pull request. The regenerated
`.ours.xml` files in this directory match the official samples on every one
of those lines, character for character. The cause, and the evidence, are
below.**

## The cause of the first report's differences

The probe harness and the crate's own test were both built into one shared
`CARGO_TARGET_DIR` (`~/BuwizNsh/target-nsh`) used by every worktree on the
machine (#323, #326, #346, main). Form definitions reach the binary at
compile time — `include_str!` in `crates/bir-core/src/forms/agent/mod.rs:125`
— so a bir-core object compiled from a checkout at main, where item 17 and
the others-row due are still `computed` in `data/agent-forms/`, satisfied
the probe binary's fingerprint (same package version; cargo's mtime
freshness cannot distinguish two checkouts). The refusal "computed by
BIR's own formulas" came from `set_refusal`
(`crates/bir-app-api/src/screen_form_module.rs:5419`) reading main's baked-in
data. In a fresh, empty, exclusive target directory at the same commit
4efeca2d the identical probe accepts item 17 in every experiment (verbatim
replication of the pull request's own test, with and without `unlocked`,
minimal batch, solo write) and all three samples reproduce the official
upload text. Nothing in the write path, the draft shapes, or the pull
request ever differed.

## What every file is

- `<sample>.ours.xml` — regenerated from the clean build: every `.steps.json`
  edit entered through the page's own write path (`screen_form_fill`, keyed
  by the official DOM ids, as a filer's edits), the sample's period set
  through `screen_form_sample_period`, saved with `screen_form_save`, then
  the payload builder `to_bir_xml_payload()` on the saved draft. Never a
  live send, never Queue.
- `<sample>.official.xml` — an exact copy of the official sample at that head.
- `probe.log` — the probe's full step/rejection log from the clean run.

What they do not show: a live filing, the paper layout, or any edit a filer
could not make. Profile boxes (TIN, RDO, e-mail) are refused by design; the
sample's profile already carries those values.

## What matches (clean build)

Number format, thousands separators and decimals match character for
character everywhere except the 1701MS penalty lines below; untouched boxes
hold the same values in both files.

**2552-03142025** — items 17, 19, 20 and 25 all match exactly:
`txtPg1P2I17TaxPaidInPrevRtrn` = `100.50`, `txtPg1P2I19TotTaxCredPmnt` =
`1,101.00`, `txtPg1P2I20TotTaxStllDueOvrPmnt` = `12,321.00`,
`txtPg1P2I25TotAmntPyable` = `12,357.00`.

**2200A-03142025** — the others-row due and everything it feeds match
exactly: `txtSched1_xa_others1_Due` = `11.00`, `txtSched1_TotalDue` =
`87,451.56`, Part III down to `txtAmtPayable` = `87,252.49`.

**1701MS-122025** — the opened boxes match exactly (2A/2B = `90,000`/
`90,000`, the 3A/3B chain byte-identical). The only diffs remain the six
penalty fields (ours `49,625`/`14,618`/`20,000`/`84,243`/`282,743`/
`282,743` vs official `0`s): the library sample draft ships pre-entered
penalties a blank return never has — a path artifact, outside the boxes
this pull request opens, unchanged from the first report.

## live/ — the running app at 4efeca2d, clean rebuild

The live app was rebuilt with the same exclusive target directory and
driven with real input (gpui-agent semantic actions on the Linux window;
demo taxpayer, scratch database; the four safety variables set, Queue never
pressed). Files:

- `2552-new-return-items-17-19.png` — a New-return 2552 (Amended = Yes,
  Local Stock Exchange): item 17 an open white box holding `100.50`, item
  18 `1,000.25`, item 19 computed `1,101.00`; the computed boxes around
  them grey with lock icons. Captured after a full close and reopen of the
  app: the values persist in the saved draft (`tax_paid_previous` 100.5,
  `creditable_tax_withheld` 1000.25 in the database row itself).
- `2552-library-item17.png` — the Form-library path: a 2552 sample, Amended
  = Yes, item 17 typed `100.50` and accepted.
- `2200a-new-return-schedule-total.png` — a New-return 2200A: the Others
  row opened with its `Add row` button ("No rows yet" until pressed — the
  row genuinely does not exist before), taxable `2.00`, rate `5.5`, tax due
  `11.00` all typed into plain enabled boxes, and the schedule total line
  following at `11.00`. After close/reopen the total still reads `11.00`
  and the database row carries the row's `tax_due: 11.0`.
- `upload-excerpts.txt` — the key upload lines, ours beside official, all
  `match=True`.

What live/ does not show: the mp4 recordings the task asked for are absent
— this machine's Xvfb does not yield pixels to x11grab/xwd/import (they
capture uniform black), so a screen recording would be 110 seconds of
black; the app-surface PNGs above come from the app's own screenshot
mechanism instead. The 2200A library-path case and the transaction date
03/14/2025 entry are not covered: on the New-return path the date boxes
carry the hint "Set by the date of transaction you start this return with
(New return)" yet the New-return dialog exposes no date input — how a
filer sets that date on path (a) is unresolved here; on the library path
the date box is a button opening a sheet with a text input. The incomplete
live drafts do not validate as whole returns (missing unrelated required
fields), so no live-draft XML is included; the upload-text evidence is the
probe's full-sample builds above.
