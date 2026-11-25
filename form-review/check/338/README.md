# PR #338 Evidence Inventory (`form-review/check/338/`)

Independent check of PR #338 (`feat/store-launch-agent-fill-gaps` @ `75dcab06`) run in the live NSH desktop app on Xvfb `:99` (`1440×900`) with a fresh scratch DB and demo taxpayer (`00012345300000`).

## Recordings (`.mp4`, app window only, Linux `ffmpeg x11grab`)
- `recording-2552.mp4` (18.0 s) — **Shows:** opening 2552 TY 2027, selecting Primary IPO, live `form.fill` of Item 12 shares and Part IV rows 1–5, and opening Print Preview. **Does not show:** the JSON payload of the fresh-state/LSE `form.fill` acceptance (recorded in `REPORT.md`).
- `recording-1702ex.mp4` (16.7 s) — **Shows:** opening 1702EX TY 2027, live `form.fill` attempts on popup-owned Part IV Item 49, Schedule 1, and Schedule 2 rows, and opening Print Preview. **Does not show:** verbatim JSON refusal strings (quoted in `REPORT.md`).
- `recording-1702mx.mp4` (15.8 s) — **Shows:** opening 1702MX TY 2027, live `form.fill` refusals on Schedules 5 & 6, live `form.fill` of Schedule 7.1 & 8.1 NOLCO rows 4–6 A–D, and opening Print Preview. **Does not show:** the SQLite `data_json` proof that `WholePesoInput.raw` is `""` (documented in `REPORT.md`).

## Live Form-Page Screenshots (`1400×875`)
- `form-2552-part1.png` — **Shows:** Form 2552 Part I with Item 11B `Primary` selected and Item 12A (`2500000`) / Item 12B (`10000000`) filled. **Does not show:** Part IV or print output.
- `form-2552-part4.png` — **Shows:** Form 2552 Part IV rows 1–5 filled with classifications and amounts (`101,234,567.89` .. `505,678,901.56`). **Does not show:** print alignment.
- `form-1702ex-part4.png` — **Shows:** Form 1702EX Part IV after `form.fill` refusal of Item 49. **Does not show:** refusal JSON text.
- `form-1702ex-sch1.png` — **Shows:** Form 1702EX Schedule 1 after `form.fill` refusal of attached-schedule rows. **Does not show:** Schedule 2 Item 4 `LegalBasis` acceptance (documented in `REPORT.md`).
- `form-1702mx-sch7.png` — **Shows:** Form 1702MX Schedule 7.1 rows 4–6 filled on the form page (`150,000,000` .. `130,000,000`, computed Col E `95,000,000` / `96,000,000` / `97,000,000`). **Does not show:** that these A–D values print blank on paper.
- `form-1702mx-sch8.png` — **Shows:** Form 1702MX Schedule 8.1 rows 4–6 filled on the form page (`120,000,000` .. `100,000,000`, computed Col E `88,000,000` / `82,000,000` / `75,500,000`). **Does not show:** that these A–D values print blank on paper.

## Saved PDFs & Paired Page Renders (`1400 px` wide)
- `saved-2552.pdf`, `preview/2552-p1.png..p2.png`, `pdf/2552-p1.png..p2.png` — **Shows:** rendered 2552 return from the live app's `sheet.html`, including left-aligned Item 12A/12B combs (`p1`), comb-tick crossings (`p1`), and left-aligned Part IV amounts (`p2`). **Does not show:** BIR XML submission (dry-run only).
- `saved-1702ex.pdf`, `preview/1702ex-p1.png..p3.png`, `pdf/1702ex-p1.png..p3.png` — **Shows:** rendered 1702EX return, including truncated Registered Address (`p1`) and header Registered Name comb-tick crossing (`p2–p3`). **Does not show:** popup-schedule attachments.
- `saved-1702mx.pdf`, `preview/1702mx-p1.png..p4.png`, `pdf/1702mx-p1.png..p4.png` — **Shows:** rendered 1702MX return; `p4` proves that all 24 newly fillable Schedule 7.1 & 8.1 NOLCO A–D boxes print **completely blank** while Years, Col E, and Row 8 print left-aligned and unformatted. **Does not show:** non-empty BIR XML for A–D (because `raw` is `""`).

## Official Reference Pages (`1400 px` wide)
- `official/1702ex-p1.png..p3.png` — **Shows:** official `1702-EX Jan 2018 ENCS v2.pdf` (exact provenance SHA-256 match). **Does not show:** filled values.
- `official/1702mx-p1.png..p4.png` — **Shows:** official `1702-MX Jan 2018 ENCS Final with OSDv2.pdf` (exact provenance SHA-256 match). **Does not show:** filled values.
- `official/2552-p1.png..p2.png` — **Shows:** blank frozen `html-frozen/2552-2018/index.html` template rendered to PNG. **Does not show:** the raw BIR PDF itself (BIR CDN returned HTTP 403 for all candidate `2552` PDF URLs).

## Report
- `REPORT.md` — Full independent check report with verdicts, ranked faults, box-by-box tables, verbatim refusal texts, sample-payload counts, and file-by-file diff reading.
