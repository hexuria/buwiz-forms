# Proof artifacts — independent check of PR #323 at 0ef364ec

What every file shows and what it does not show.

## Recordings (the running Linux NSH app in docker on Xvfb, demo taxpayer Juana Santos, invented values)

- `2552-flow.mp4` (101 s): 2552 with Amended Return = No — Item 17 is **entirely absent** from the page (numbering jumps 16 → 18) — then Amended = Yes, Item 17 appears, 123,456,789.01 typed, downstream amounts follow, print preview opened.
- `1701ms-flow.mp4` (58 s): compensation income source and joint filing open items 1/2 in both columns; large values typed; taxable compensation and tax due follow; print preview.
- `2200a-flow.mp4` (21 s): Add row opens a Schedule 1 Others row editor; description and Basic Excise Tax Due 123,456,789.01 typed; print preview.
The recordings show the app window only. They do not show the official program.

## Screenshots of the form page in each state

- `2552-item17-shut.png` / `2552-item17-open.png` — Item 17 absent vs present. Described from the picture: in the shut state the row is NOT rendered at all (no greyed box, no lock icon, no reason text); the item numbering skips it.
- `1701ms-closed.png` / `1701ms-single.png` / `1701ms-joint.png` — before the compensation schedule opens; single filer (column A inputs visible and enabled, column B absent); joint filer (column B appears).
- `2200a-empty.png` / `2200a-filled.png` — Schedule 1 Others rows without and with a filled row.
- `2200-part4-crop.png` — close-up of 2200A's printed Part IV: blank even though the draft held Cash/Bank Debit Memo payment values.

## Print proof

- `<form>-preview-page-N.png` — every print-preview page (about 1400 px wide) of each form with large invented amounts in the opened boxes.
- `1701ms.pdf`, `2552.pdf`, `2200a.pdf` — Chromium print-to-PDF of the exact sheet.html the app's print preview generates.
- NO official-page PNGs: the PDFs named in html-frozen/*/provenance.json are not present in the repository, so side-by-side official pairing was impossible. What the preview pages show is therefore compared against the printed captions and box geometry only, not against the official bitmap.

## Text evidence

- `upload-comparison.txt` — the official samples' plain.xml serialization next to the replayed draft values, key by key, for the PR boxes and the downstream chain.
- `probes/replay-2552.log`, `replay-1701MS.log`, `replay-2200A.log` — the exact form.fill calls and raw replies replaying each official sample's .steps.json through the page's write path on fresh scratch databases.
- `probes/probe323-2552states.log` — Item 17 fill when not amended; amend+fill in one call; un-amending after typing (value zeroed).
- `probes/probe323-2552arith.log` — large/negative/oversized Item 17 amounts and the computed chain.
- `probes/probe323-1701MSsingle.log`, `probe323-1701MSjointswitch.log` — 2A/2B single-filer fills; joint → not-applicable switch after typing.
- `driver-transcript.txt` — the GUI driver's own transcript behind the recordings.

What the transcripts do not show: the visual page (use the screenshots/recordings); the GUI interactions are in the recordings, not the probe logs.
