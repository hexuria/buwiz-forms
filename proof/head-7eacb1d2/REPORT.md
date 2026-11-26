# Money-focus fault hunt — Linux real-window validation

Exact commit verified: `7eacb1d2195b8dc1e1a1b65ad97728c15b50bc2f` (`fix/money-box-keep-text-on-focus`, PR #403 as briefed).

- Worktree preserved: `/home/ubuntu/forms-validation/money-focus-7eacb1d2` — `git status --short` empty after removing the temporary seed fixture.
- Target preserved: `/home/ubuntu/forms-validation/target-money-focus-7eacb1d2` (16 GiB).
- Binary: `/home/ubuntu/forms-validation/target-money-focus-7eacb1d2/debug/nsh-app-nsh-app-ebirforms-linux-host`.
- Scratch DB: `/home/ubuntu/forms-validation/money-focus-proof/money-focus-validation.db`.
- Demo/invented data only: Juana demo TIN `00012345300000`; no real taxpayer data.
- Safety environment: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; no live mode, no Queue press, no token printed.
- Linux window: real visible maximized X11 window on `DISPLAY=:0`, with `VK_ICD_FILENAMES=/dev/null GSK_RENDERER=cairo LIBGL_ALWAYS_SOFTWARE=1`.
- Agent ports `8899`/`17433` were used and are free again after the run. Android watch/emulator was not touched.
- Semantic snapshots were used only to locate/read controls. Entry was real `xdotool key`; ordered 120 ms cases used `--delay 120`, burst cases used `--delay 0`, with click/Tab and first key chained in one xdotool invocation where required.

## Draft reads

- 2551Q draft: `form_drafts.data_json -> schedule_1[0].taxable_amount` for TIN `00012345300000`, form `2551Q`, year `2026`, quarter `4`.
- H3 2551Q surcharge: `data_json -> surcharge` and `penalty_overrides.surcharge`.
- 2550M draft: `form_drafts.data_json -> domestic_goods_input_tax` for form `2550M`, year `2026`, period `11`.

Final relevant database values after the run: 2551Q schedule amount `15000.5`, surcharge `1500.0`; 2550M Item 18F `999.0`.

## Results

| Case | Visible after keys | Visible after leaving | Saved draft | Result |
|---|---:|---:|---:|---|
| E1 Tab into `1,234.56`, type `15000.50` at 120 ms | `15000.50` | `15,000.50` | `15000.5` | PASS |
| E1 Shift+Tab into `1,234.56`, type `15000.50` at 120 ms | `15000.50` | `15,000.50` | `15000.5` | PASS |
| E1 click, Ctrl+A, type `15000.50` at 120 ms | `15000.50` | `15,000.50` | `15000.5` | PASS |
| H1 click into zero box, `15000.50` no-delay, 10 runs | `15000.50` every run | `15,000.50` every run | `15000.5` every run | PASS 10/10 |
| H1 Tab into zero box, `15000.50` no-delay, 10 runs | `15000.50` every run | `15,000.50` every run | `15000.5` every run | PASS 10/10 |
| H2 no-delay `15000..50` | empty | `0.00` | `0.0` | FAULT |
| H2 no-delay `150a00.50` | empty | `0.00` | `0.0` | FAULT |
| Diagnostic, same `15000..50` at 120 ms | `15000.50` | `15,000.50` | `15000.5` | contrast PASS |
| Diagnostic, same `150a00.50` at 120 ms | `15000.50` | `15,000.50` | `15000.5` | contrast PASS |
| H3 form focus into filled surcharge `1,500.00`, type `2500` at 120 ms | `1,500.002500` | `1,500.00` | `1500.0` | FAULT |
| H4 2550M recomputed Item 18F, type `999` after Tab, 5 runs at 120 ms | `999` every run | `999.00` every run | `999.0` every run | PASS 5/5 |
| H4 same, 5 runs no-delay | `999` every run | `999.00` every run | `999.0` every run | PASS 5/5 |

### H1 leading-zero observation

In all 20 no-delay H1 runs the immediate post-key screenshots/semantic values showed `15000.50`, not `0.0015000.50` or another leading-zero corruption, and the saved value was correct. This is strong evidence for the end state; the 60 fps recording is retained for frame inspection, but I did not separately prove every transient frame programmatically.

### H2 fault detail

With zero in the 2551Q Schedule 1 row 1 taxable amount, a click immediately followed by a no-delay burst containing either a second decimal point (`15000..50`) or a letter (`150a00.50`) left the box visibly empty right after the keys. On blur it showed `0.00` and the draft saved `0.0`. No inline error text was visible in the screenshots/semantic text capture. Repeating the same two strings at 120 ms as diagnostics produced the expected cleaned value `15000.50` and saved `15000.5`, so this is speed-dependent, not merely invalid-character masking.

Key screenshots:

- `/home/ubuntu/forms-validation/money-focus-proof/h2-dots-after-keys.png`
- `/home/ubuntu/forms-validation/money-focus-proof/h2-dots-after-leaving.png`
- `/home/ubuntu/forms-validation/money-focus-proof/h2-letter-after-keys.png`
- `/home/ubuntu/forms-validation/money-focus-proof/h2-letter-after-leaving.png`
- `/home/ubuntu/forms-validation/money-focus-proof/diag-dots-120-after-keys.png`
- `/home/ubuntu/forms-validation/money-focus-proof/diag-letter-120-after-keys.png`

### H3 method and fault detail

Method used to make the form itself move focus: in 2551Q Part II Item 20, clicked `Edit penalties`, filled Surcharge with `1,500.00`, blurred/saved it, clicked the visible `Check` button, then used the visible to-do pill next button (`fp-pill-next`) until it was on `Item 20 · Surcharge`. I then typed `2500` without clicking the amount box. The keys landed in the form-focused surcharge box.

Observed: the existing amount was not selected/replaced. The visible text became exactly `1,500.002500`; after leaving it displayed `1,500.00`, and the draft remained `1500.0`. This is the append failure mode called out in the brief.

Caveat: the Item 20 row was a validation/to-do row in the used return (the visible message concerned the missing EOPT tier/penalty-rate prerequisite). I did not additionally prove a case where the amount itself was negative or otherwise intrinsically invalid, because the normal money mask does not provide an easy real-key path to such a value in this surface. The focus question itself was exercised exactly: the page/step moved keyboard focus into a filled amount and typing appended.

Key screenshots:

- `/home/ubuntu/forms-validation/money-focus-proof/h3-after-keys.png` — shows `1,500.002500` while focused
- `/home/ubuntu/forms-validation/money-focus-proof/h3-after-leaving.png` — shows reversion to `1,500.00`

### H4 method

On 2550M November 2026, Item 18E (`domestic_goods`) was typed as a base amount. Leaving 18E recomputed editable Item 18F (`domestic_goods_input_tax`) at 12% (for example base `1500` produced `180.00` before typing). Focus was placed in 18E, then one xdotool process sent Tab followed immediately by `999`. This was repeated five times at 120 ms and five times with no delay. Every run showed `999` after keys, `999.00` after leaving, and saved `999.0`; no append occurred.

## Artifacts

- Recording: `/home/ubuntu/forms-validation/money-focus-proof/money-focus-validation.mp4`
- Machine-readable rows: `/home/ubuntu/forms-validation/money-focus-proof/focus-results.jsonl`
- CSV: `/home/ubuntu/forms-validation/money-focus-proof/focus-results.csv`
- Driver: `/home/ubuntu/forms-validation/money-focus-proof/run_focus_cases.py`
- Shared driver helpers: `/home/ubuntu/forms-validation/money-focus-proof/drive_focus.py`
- Seed/build log: `/home/ubuntu/forms-validation/money-focus-proof/seed-build.log`
- Run log: `/home/ubuntu/forms-validation/money-focus-proof/nsh-run.log`
- H4 raw output: `/home/ubuntu/forms-validation/money-focus-proof/h4-run-output.txt`
- Scratch DB: `/home/ubuntu/forms-validation/money-focus-proof/money-focus-validation.db`

Representative screenshots are in the same directory; every H1/H4 run has `*-after-keys.png` and `*-after-leaving.png` files.

## Untested / left for later

- H5-H8, explicitly deferred by the brief.
- Frame-by-frame proof of every transient H1 frame.
- A stricter H3 variant where the amount itself is intrinsically invalid rather than sitting in an invalid/to-do penalty row.
