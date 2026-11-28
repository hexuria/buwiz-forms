driver: gpui-agent-cli 0.5.0 + nsh-gpui-agent b0c1a5f2e0a23d9b37a8c3059fac52c0c9feae0b

Apple proof for hexuria/buwiz-forms#584 at head b3bddd4d252d0daac05b322f18ce6e45e6c1fa18 (macOS desktop, smoke suite via scripts/e2e/machines/macos.sh, dev test build, scratch database, dry-run send).

`gpui-agent --version`: gpui-agent-cli 0.5.0 (prebuilt route). Note: report.json's cli_version reads 0.4.1 — the runner parses cargo install metadata, which is stale on a prebuilt install; the binary is 0.5.0.

Evidence layout: runN/smoke = the smoke suite report; runN/rdo-row-press = the second suite the machine script runs. No absolute local paths here: ~ means the builder's home.

## macOS desktop smoke, three runs at b3bddd4d252d0daac05b322f18ce6e45e6c1fa18

Run header of each run: `app b3bddd4d, gpui-agent-cli 0.5.0`. All three runs produced the identical table (run1/, run2/, run3/smoke/report.json):

| recipe | status | note |
|---|---|---|
| setup-start | pass |  |
| sign-up | pass |  |
| tax-year | pass |  |
| navigate | pass |  |
| open-2551q | pass |  |
| money-boxes | pass |  |
| sheet-press | skipped | no desktop variant |
| delete-row | setup |  |
| queue | fail | precondition: Queue validates first. The recipe records the profile's business s |
| birth-date | pass |  |
| sign-out | known gap |  |

sign-up: pass 3 of 3 (run1 12.4s, run2 pass, run3 8.5s). No row is worse than the #497 baseline: same passes, same sheet-press skip, same delete-row setup, same queue fail at fp-send-line, same sign-out known gap.

## scrolled

`grep -r scrolled` over every run's evidence (report.json, run.log, the per-step files): zero matches in all three runs. **No step scrolled** — no step carries error_code "scrolled" anywhere on the desktop.

## rdo-row-press suite

The machine script also runs a second suite (rdo-row-press: setup-start, rdo-row-press, sign-up, profile-pick-press): all pass in run1 and run2; in run3 its report.json could not be written because the disk filled transiently during that run's final write (the smoke suite's report.json of run3 is complete; the machine had ~170 Mi free at that moment). Its step files are under run3/rdo-row-press/.

## video

run3/macos-sign-up.mp4: screen recording of run 3. sign-up's typing happens in the first ~20 seconds (21:48:48-21:48:56 machine-local). h264 1600x1200.
