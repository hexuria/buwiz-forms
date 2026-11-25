# 2551Q two-code save investigation (work in progress)

Established so far:

- CI failure reproduced locally at `a8aeaca8`: `screens/form/atc2551q.test.tsx` fails 3/3 with `Unsaved changes`, rows PT040/PT060 visible, total still ₱0.00.
- `f5a3fa2c` passes the same file 3/3; `08ffd5c6` fails 3/3, identifying #346 as the first failing merge. Confirmation runs at `d95d3e9c` and `db022086` are still pending.
- Direct host fills at `a8aeaca8` (`drpATC1: "2"`, `drpATC2: "4"`, `txtATCAmt2: "2000"`) are accepted and compute total tax due 40.00.
- Direct `FormSession` at `a8aeaca8` with the same values saves cleanly.
- Instrumenting the real UI test shows it sends `txtATCAmt2` as `0.002000`, not `2000`, because headless `app.type` starts at the end of the existing `0.00` (NSH testing documentation). #346's entry refusal then rejects that malformed transient amount.
- The candidate test correction in this branch clears the zero amount before typing, matching the documented real-window behavior of a zero money box being selected on click. It has passed once locally; two more validation runs are pending.
- `session.fixes.test.ts` at `f5a3fa2c` passed 3/3 (13/13 each). The first 08ffd5c6 attempts were invalid compile failures caused by disk exhaustion, not test results; valid reruns are pending.
- Real-window manual verification is complete: both a8aeaca8 and f5a3fa2c save with green `✓ Saved`, total ₱40.00, no host refusal in the log, and database rows PT040/PT060 with PT060 amount 2000/tax due 40. The window manager enforced 448x844 rather than the requested 390x844. See `manual/REPORT.md`.

This file is temporary investigation evidence and should be removed or replaced by final proof before the pull request is undrafted.
