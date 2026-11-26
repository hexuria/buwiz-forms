# Linux money-box validation evidence at 30ada22f

Validated commit: `30ada22f7071b0e243e29ec5e7f17486e58c0eb9` (confirmed with `rev-parse`). All application data was invented scratch data; no Queue, no live BIR/relay, no tokens.

## Results

- Fault 1 is old on main on Linux: the keys are appended to the recomputed amount. MAIN `642bfc78221c0a87b318c7c3378c3f5b0b2c9359` and PR `7eacb1d2195b8dc1e1a1b65ad97728c15b50bc2f` both fail 5/5 at 120 ms and pass 5/5 at 0 ms on 2550M 13A -> recomputed 13B.
- The earlier passing 7eacb1d2 evidence used 18E -> 18F, allowed about 0.9 s for recompute to settle, and did not impose an initial post-Tab delay.
- With a local uncommitted widening of `WINDOWS_RESELECT` to Linux, 13A -> 13B typing `999` passes 10/10 at each first-key delay 0/120/300/1000 ms, saving `999.0` every time.
- Five 120 ms traces measured outside-write -> focus callback 17-18 ms and focus -> end-caret collapse 34-35 ms.
- Cost found: clicking the right end of freshly recomputed 13B within 100 ms of recompute is reselected by the widened guard; typing `9` replaces the recomputed amount (`9.00`, saved `9.0`) instead of appending.
- Non-recomputed filled-box End+9 appended normally even in a fast chain; an interior click inserted at the clicked position; zero + 42 worked.
- L2 on unchanged `30ada22f`: 1601FQ Schedule 1 rate with relief unanswered showed `42` while typing, `0.00` after leaving, draft `schedule1[0].tax_rate=42.0`; exact sentence: `Tax Rate: you typed 42; the return holds 0.00.`
- Fault 2 trace at prior `93c57518` is included under `fault2/`: host emits `txtRate0=0.00` when relief is unanswered; visible/host agree and dormant draft rate differs. The missing sentence cause there was the money/num-only note gate skipping runtime kind `pct`.
- MAIN stray-row control: after a plain real-UI Delete of row 1, draft JSON keeps a blank zero row while the page visibly shows one row (former row 2).

## Key files

- `summary.txt` — Step B/C summary.
- `linux-reselect-instrumentation.diff` — local uncommitted widening plus trace instrumentation.
- `matrix.jsonl`, `traces/` — guard matrix and raw traces.
- `l2-result.json`, `l2-after-leave.png` — L2 sentence evidence.
- `nothing-else-interpreted.json`, `check-d-prepositioned-trace.txt`, `check-d-after-leave.png` — no-regression checks and the click-caret cost.
- `stepa/` — MAIN `642bfc78` and PR `7eacb1d2` comparison.
- `quick-comma-results.json`, `quick-final-results.json` — comma behavior evidence.
- `main-stray-after-delete.png`, `main-stray-run.log` — stray-row evidence.
