# 2551Q two-code save investigation

Established so far:

- CI failure reproduced locally at `a8aeaca8`: `screens/form/atc2551q.test.tsx` fails 3/3 with `Unsaved changes`, rows PT040/PT060 visible, total still ₱0.00.
- `f5a3fa2c` passes the same file 3/3; `08ffd5c6` fails 3/3, identifying #346 as the first failing merge. Confirmation runs at `d95d3e9c` and `db022086` are still pending.
- Direct host fills at `a8aeaca8` (`drpATC1: "2"`, `drpATC2: "4"`, `txtATCAmt2: "2000"`) are accepted and compute total tax due 40.00.
- Direct `FormSession` at `a8aeaca8` with the same values saves cleanly.
- Instrumenting the real UI test shows it sends `txtATCAmt2` as `0.002000`, not `2000`, because headless `app.type` starts at the end of the existing `0.00` (NSH testing documentation). #346's entry refusal then rejects that malformed transient amount.
- The test correction in this branch clears the zero amount before typing, matching the documented real-window behavior of a zero money box being selected on click. At the branch head the corrected `screens/form/atc2551q.test.tsx` passed 3/3 on Linux (1/1 each; logs in `headless-validation/`).
- `session.fixes.test.ts` at `f5a3fa2c` passed 3/3 (13/13 each). The first 08ffd5c6 attempts were invalid compile failures caused by disk exhaustion, not test results. Valid `08ffd5c6` run 1 passed 13/13; valid runs 2 and 3 are pending. The branch-head run passed 13/13. Logs are in `headless-validation/`.
- Real-window manual verification is complete: both a8aeaca8 and f5a3fa2c save with green `✓ Saved`, total ₱40.00, no host refusal in the log, and database rows PT040/PT060 with PT060 amount 2000/tax due 40. The window manager enforced 448x844 rather than the requested 390x844. See `manual/REPORT.md`.
- Real-window invalid-amount probing is complete: `2000.005`, `1000000000.01`, and `-100` each show explicit red box-level sentences, Done refuses by keeping the row sheet open, and the database keeps amount 0.0. This is not the silent-failure shape; see `amount-cases/REPORT.md`.

This NOTES file is retained as investigation evidence.
