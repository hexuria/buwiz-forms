# Proof artifacts — independent check of PR #338 at afe7bc34

What every file shows and what it does not show.

## Recordings (the running NSH app on Linux, Xvfb, demo taxpayer, invented values)

- `2552-gate.mp4` (117 s): form 2552 with the IPO gate shut — a `form.fill` of Item 12 total shares is refused with the gate's reason — then Item 11 is set to Primary and the same fill is accepted; the Part IV row chain is shown in the transcript. Shows the replies and the page. Does not show the official program.
- `1702ex-gate.mp4` (27 s): the five attached-schedule boxes (Schedule 1 item 17i, Schedule 2 item 4) refused naming their owning schedule. Does not show opening those gates (they never open on the form page itself — they belong to the attached schedule windows).

## Screenshots of the form page in the shut state

- `2552-shut-item12.png` — fresh 2552: the page renders no Item 12 row at all when shut (items end at 11); there is no visible reason for Item 12 — the row is absent, not greyed. What the page shows, not the stylesheet.
- `2552-open-item12.png` — after Item 11 = Primary: the Item 12 row appears.
- `2552-shut-part4-row2.png` — Part IV shows `No rows yet` plus the visible gate text lines `Needs Part IV row 1` … — the page's own reason, same string as the refusal.
- `1702ex-shut-sched1-17i.png`, `1702ex-shut-sched2-item4.png` — the rows greyed with lock icons and the note `Filled from the attached schedule.` (same meaning as the refusal, not the identical string).
- `1701-shut-spouse.png` — fresh 1701: the spouse Part IV section collapsed to `0 items` when shut; opening it requires Married + Yes + Joint.

## Transcripts and comparisons (text)

- `fill-transcript.txt` — the exact form.fill calls and replies behind the two recordings.
- `fills/2552-p1…p8-afe7bc34.txt` — the exact calls and raw replies for the 2552 probe table in REPORT.md (fresh return, one-call both-orders, choice later, open-then-shut, zero-row, clearing).
- `fills/1702ex-five-boxes-afe7bc34.txt` — exact calls and replies for the five attached-schedule boxes.
- `fills/1701-claimkeys-afe7bc34.txt` — proof that the claim's three 1701 keys are refused as not-a-box on form 1701 (the F2 test's keys are imaginary).
- `fills/1701-realgates-afe7bc34.txt`, `fills/1701-g3.txt` — the real 1701 gates: shut-state refusals and the one-call deep chain that opens them.
- `fills/<form>-fill-comparison.txt` (13 forms) — one identical form.fill per commit, filled/refused lists at origin/main and at afe7bc34, plus validate errors. These are the 2b tables.
- `fills/recipe-338.log`, `fills/recipe-main.log` — the repository's own generated-recipe run over the 1701Q CSV data set at each commit: identical pass/fail rows.

What the text files do not show: the visual page (use the screenshots/recordings), and the official program's behavior beyond what the cited inventories record.
