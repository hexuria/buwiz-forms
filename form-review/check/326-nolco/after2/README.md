# PR #326 F1 Follow-up Proof (`after2/` — `#326` @ `2555a2d8`)

Follow-up fix on `feat/store-launch-unfillable-rows` (`2555a2d8`) so computed 1702MX NOLCO whole-peso boxes (Schedule 7.1 & 8.1 Column E, row 7 current-year loss, and row 8 totals) carry official `formatCurrencyWOC` thousands separators (`150,000` / `50,000` / `0`) in both the official upload XML and printed page 4.

## Files

- `official-1702MX-1225-sched6-nolco.txt`: exact 50-line Schedule 7.1 & 8.1 XML block from `crates/bir-core/tests/sample-payloads/1702MX-1225-sched6.official.xml`.
- `ours-before-13e6abe4-nolco.txt`: 50-line Schedule 7.1 & 8.1 XML block produced at `13e6abe4` (before this change: `Shed7_4E=150000`, `Shed78D=50000` without commas).
- `ours-after-2555a2d8-nolco.txt`: 50-line Schedule 7.1 & 8.1 XML block produced at `2555a2d8` through the page write path (and identically through `form.fill`) — matches `official-1702MX-1225-sched6-nolco.txt` 50/50 lines byte-for-byte.
- `side-by-side.txt`: side-by-side comparison of all 50 lines (`official` vs `before 13e6abe4` vs `after 2555a2d8`).
- `ours-24box-after-2555a2d8.txt`: full 50-line NOLCO XML block for the 24-box `326-nolco` return at `2555a2d8` (showing `150,000`, `120,000`, `55,000`, `145,000`, `225,000`, `155,000`, `90,000`, `165,000` with commas).
- `saved-1702mx.pdf` and `p4.png`: re-rendered printed page 4 of the same 24-box `326-nolco` return at `2555a2d8`, showing the comma-formatted Column E and Row 8 totals on paper (compared against `../after/pdf/p4.png` at `13e6abe4`, which printed raw digits without commas).

## What is not shown

- No screen recording (requested omitted for this step).
- Pages 1–3 of `saved-1702mx.pdf` are unchanged from `../after/pdf/p1.png..p3.png`.
