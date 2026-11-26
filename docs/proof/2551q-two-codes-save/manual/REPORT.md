# Manual real-window 2551Q two-ATC proof

## Verdict

Both real Linux GTK runs saved the draft. The failure seen in the headless a8aeaca8 test is a test artifact, not an app bug.

- a8aeaca8: PASS — green `✓ Saved`, total `₱40.00`, scratch draft contains PT040 and PT060/2000.
- f5a3fa2c: PASS — green `✓ Saved`, total `₱40.00`, scratch draft contains PT040 and PT060/2000.

No `Queue` control was pressed. `BIR_SFTP_LIVE` was never set.

## Exact environment/window notes

Each commit was launched with its own wrapper from its own checkout, using `GSK_RENDERER=cairo`, `BIR_LOG_DIR`, and the wrapper-provided `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`. The wrapper does not itself pass an outer `CARGO_PROFILE_DEV_DEBUG=0` into Docker, so it was also passed inside the container command and verified in the cargo process environment.

Requested window size was `390x844`; `wmctrl` enforced a wider minimum and measured both real windows as `448x844` at position `100,80`.

The return was the default `2551Q` Q4 2026. The demo taxpayer (Juana Santos, TIN `000-123-453-00000`, RDO 039, ZIP 1100) was created manually through the first-run UI in each fresh scratch database, then 2026 and 2551Q were enabled through the visible Forms UI.

## a8aeaca8 result

Manual flow: initial Schedule 1 row `PT010 · 3.0`; row 1 changed to `PT040`; row 2 added and set to `PT060`; one click in the row-2 amount followed by typing `2000` produced visible amount `2,000` and tax due `40.00`; final Done left rows PT040 and PT060 and total `₱40.00`; scrolling to the top showed green `✓ Saved`.

The `0.00` text did not show a visible blue selection highlight after the click, but typing still replaced it rather than concatenating: the visible value became `2,000`, not `0.002000` or another concatenated value. The first recording contains one overly strong red annotation about the missing visible highlight; the very next state and the final result prove the zero was replaced.

Database evidence (`a8/a8-db-draft.json`): scratch DB `/tmp/ebirforms-door/36857-0/bir_data.db` inside the container, inspected through `/proc/714711/root/tmp/ebirforms-door/36857-0/bir_data.db` while the process was alive. One `form_drafts` row: tin `00012345300000`, form `2551Q`, year 2026, quarter 4, status `Draft`; schedule rows:

1. PT040, amount 0.0, rate 0.03, tax due 0.0
2. PT060, amount 2000.0, rate 0.02, tax due 40.0

`total_tax_due`, `tax_payable`, and `total_amount_payable` are all `40.0`.

Logs: no host refusal and no 12-character/non-negative error. `a8/run.stdout.log` has only normal build/connect output plus the harmless tray warning; `a8/logs/ebirforms.2026-11-25.log` has startup, migrations, and cron wake messages.

## f5a3fa2c result

Manual flow: initial Schedule 1 row `PT010 · 3.0`; row 1 changed to `PT040`; row 2 added and set to `PT060`; one click in the row-2 amount followed by typing `2000` produced visible amount `2,000` and tax due `40.00`; final Done left rows PT040 and PT060 and total `₱40.00`; scrolling to the top showed green `✓ Saved`.

Database evidence (`f5/f5-db-draft.json`): scratch DB `/tmp/ebirforms-door/36933-0/bir_data.db` inside the container, inspected through `/proc/875744/root/tmp/ebirforms-door/36933-0/bir_data.db` while the process was alive. One `form_drafts` row: tin `00012345300000`, form `2551Q`, year 2026, quarter 4, status `Draft`; schedule rows:

1. PT040, amount 0.0, rate 0.03, tax due 0.0
2. PT060, amount 2000.0, rate 0.02, tax due 40.0

`total_tax_due`, `tax_payable`, and `total_amount_payable` are all `40.0`.

Logs: no host refusal and no 12-character/non-negative error. `f5/run.stdout.log` has only normal build/connect output plus the harmless tray warning; `f5/logs/ebirforms.2026-11-25.log` has startup, migrations, and cron wake messages.

## Artifacts

- `a8-real-2551q-save.mp4` — a8 recording (see note about the one misleading red highlight annotation)
- `f5-real-2551q-save.mp4` — f5 recording
- `a8/a8-01-schedule-initial.png`
- `a8/a8-02-row1-pt040.png`
- `a8/a8-03-row2-before-amount.png`
- `a8/a8-04-row2-2000-tax40.png`
- `a8/a8-05-after-done-total40.png`
- `a8/a8-06-saved-status.png`
- `a8/a8-db-draft.json`
- `f5/f5-01-schedule-initial.png`
- `f5/f5-02-row1-pt040.png`
- `f5/f5-03-row2-before-amount.png`
- `f5/f5-04-row2-2000-tax40.png`
- `f5/f5-05-after-done-total40.png`
- `f5/f5-06-saved-status.png`
- `f5/f5-db-draft.json`
