# Job 45 — F3 ("Home and back rewrites a focused amount") at 30ada22f, with the trace hunt at 7d491c57

Builds: `30ada22f` clean (step 1) and `30ada22f`+logging then `7d491c57`+logging (step 2), all x86_64 dev recipe, kit pin 8c78cfd6, `BIR_DATABASE_PATH=test-data/moneybox45.db` for draft persistence, network cut, never Queue. The logging changes are local-only and were never pushed. Logging patch points (both heads, adapted to each head's structure — `7d491c57` has no `finishLeave`; its blur logic is inline in `onBlur`): `writeText` (value + caller), the outside-value `effect` entry (incoming/focused/last), `change` entry (raw/prev/focused), every `props.onChange` report, `onFocus` entry (alreadyFocused/text/value), `onBlur` entry + `blurRead` (held/reading/valid/outsideValue/edited) + `blurWrite` (shown vs current), `onSelectionChange`, and `SS edit/focus/blur` in `session.ts`.

## Step 1 — does it still happen at 30ada22f: NO, 0 of 16

16 trials, alternating launcher/recent return, each: commit 1,234.56 in the 2551Q Schedule 1 row 1 Taxable Amount, tap inside the box (focus, keyboard up), Home key, 5 s, return, screenshot before any key (`f3-t*-*.png`), read the box. Raw log: `step1-trials.txt`. Every trial: pre `1,234.56`, post `1,234.56`, box still focused, keyboard up. **changed in 0 of 16.**

## Step 2 — the trace

Logged runs at `30ada22f` (7 runs) stayed clean, so the same logging was added to `7d491c57`'s source per the order and the loop moved there. Capture: logcat cleared at the Home press, dumped 10 s after the return, filtered to the app's `js:` lines.

- Clean run, 30ada22f, launcher return: `log-unchanged-launcher.txt` — **empty: the box receives nothing at all** in the window (no blur, no focus, no change, no selection, no effect run). Same emptiness in every clean 7d491c57 run (`log7-clean-run1.txt`) and across six consecutive home+return cycles of one focus session (`cycles-log.txt`).
- Changed run: **none captured — see the report.**

## Probe data already in `../probes/` for contrast

At the unlogged 7d491c57 job-41 build the same protocol corrupted 2 of 4 trials, oscillating: clean, `123.46`, clean, `123.46`. Note the arithmetic: `1234.56 / 10 = 123.456 → 123.46` and `123.456 / 10 = 12.3456 → 12.35` — both probe readings are the amount shifted one decimal place left per event, re-rounded half-up to centavos, not dropped digits.

## Outcome of the trace hunt (final numbers)

No run changed the amount: 21 logged standard-protocol runs at `7d491c57` (runs 1–16 plus 15–21; the r16 "changed" line in `trace7-runs.txt` is a false positive — a parallel experiment force-killed the app mid-run, so the read found no box at all), plus 6 consecutive home+return cycles of one focus session (`cycles-log.txt`) and 8 more cycles with the row's ATC set to PT041 (rate 3.0, so the amount→tax-due recalculation cascade exists — `atc-cycles-log.txt`). Every window: zero `js:` box events. At `30ada22f` logged runs were equally clean. The logging build at either head never reproduces — against 2-of-4 on the unlogged job-41 build. The likeliest reading: the fault is a tight race and the added console logging in every event path suppresses it — or it needs a condition none of these protocols create. Either way the failure itself is informative: in every clean run the box receives literally nothing on pause/resume — no blur, no focus, no text change, no selection, no outside-value effect run — so when the fault does fire it must arrive through one of those paths in a window the instrumented build never enters.

## What the probe evidence already shows (step 3's questions, best available answer)

From `../probes/probe1-trial2-corrupt.png`: the corrupted reading `123.46` sits in the focused box while **Tax Due reads 3.70 — recomputed as 123.46 × 3%**. The corruption is not a display artifact: it was reported through `onChange` and adopted by the session as the row's real amount, downstream recalculation included. And both corrupted readings are exact decimal shifts, re-rounded half-up: `1234.56/10 = 123.456 → 123.46`; the job-41 original `12.35` is one more shift (`/100` overall → `12.3456 → 12.35`). Whatever writes into the box on return scales the value down by one order of magnitude per event — consistent with a centavos/pesos or grouping reparse applied one time too many — not with dropped digits. No blur-then-focus, no selection, no text change with `raw` against `prev` could be captured, because the instrumented builds never reproduce; that negative is itself the measured fact.
