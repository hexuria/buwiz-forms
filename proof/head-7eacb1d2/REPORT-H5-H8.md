# H5-H8 money-box fault hunt — exact commit 7eacb1d2

Validation surface: real visible maximized Linux X11 window, real `xdotool key` events at 120 ms for ordered typing, real Ctrl+V paste through xclip. Saved values were read directly from the scratch SQLite draft after leaving each box.

- Exact commit: `7eacb1d2195b8dc1e1a1b65ad97728c15b50bc2f`
- Worktree: `/home/ubuntu/forms-validation/money-focus-7eacb1d2`
- Target: `/home/ubuntu/forms-validation/target-money-focus-7eacb1d2`
- Scratch DB: `/home/ubuntu/forms-validation/money-focus-proof/money-focus-validation.db`
- Recording: `/home/ubuntu/forms-validation/money-focus-proof/money-focus-h5-h8-validation.mp4`
- Machine rows: `/home/ubuntu/forms-validation/money-focus-proof/h5-h8-results.jsonl`

No Queue press, no live submission, invented values only, no token printed/stored, no commit/push. Android emulator/watch untouched.

## H5 — percent and whole-number boxes holding zero

Percent surface: form **1601FQ**, Schedule 1 row 1, **Tax Rate (F)**, field `fp-txtRate0`, initially `0`.

| Entry | Exact keys/action | After keys | After leaving | Saved draft | Result |
|---|---|---:|---:|---:|---|
| click | click, `4` `2` at 120 ms | `420` | `0.00` | `420.0` | **fault: click entry produced 420, and after leaving the visible box showed 0.00 while the draft had 420.0** |
| Tab | Tab into box, `4` `2` at 120 ms | `42` | `0.00` | `42.0` | immediate entry is 42, but after leaving the visible box showed 0.00 while the draft had 42.0 |

Whole-number surface: form **2551Q**, Return details Item 5 **Number of Sheet/s Attached**, field `fp-frm2551Qv2018:txtSheets`, initially `0`.

| Entry | Exact keys/action | After keys | After leaving | Saved draft | Result |
|---|---|---:|---:|---:|---|
| click | click, `4` `2` at 120 ms | `04` | `04` | `4` | **fault: click entry produced 04 and saved 4, not 42** |
| Tab | Tab into box, `4` `2` at 120 ms | `42` | `42` | `42` | passed |

Evidence: `h5-pct-click-after-keys.png`, `h5-pct-click-after-leaving.png`, `h5-pct-tab-after-keys.png`, `h5-pct-tab-after-leaving.png`, `h5-int-click-after-keys.png`, `h5-int-click-after-leaving.png`, `h5-int-tab-after-keys.png`, `h5-int-tab-after-leaving.png` in this folder.

## H6 — comma characters in a 0.00 money box

Surface: **2551Q Schedule 1 row 1 Taxable Amount**, reset to `0.00` before each case; click then real ordered keys at 120 ms.

| Typed | Shown after keys | Shown after leaving | Saved draft | Message | Result |
|---|---:|---:|---:|---|---|
| `1500,50` | `1500,50` | `150,050.00` | `150050.0` | none | matches review expectation numerically: 150050 saved silently |
| `1.234,56` | `1.234,56` | `1.23` | `1.23` | none | matches review expectation: 1.23 saved silently |
| `1,2,3` | `1,2,3` | `123.00` | `123.0` | none | matches review expectation: 123 saved silently |

Note: the first three JSONL rows with `saved: null` are a tooling artifact from using the wrong draft path (`schedule1` rather than `schedule_1`). The cases were rerun unchanged with the correct path; use the rows marked `(correct draft read)`.

Evidence: `h6c-comma1-after-keys.png`, `h6c-comma1-after-leaving.png`, `h6c-comma2-after-keys.png`, `h6c-comma2-after-leaving.png`, `h6c-comma3-after-keys.png`, `h6c-comma3-after-leaving.png`.

## H7 — real Ctrl+V pastes into a 0.00 money box

Same surface, reset to `0.00` before each case. Text was placed on the X clipboard with xclip and pasted with real Ctrl+V.

| Clipboard text | Taken/refused | Shown after paste | Shown after leaving | Saved draft | Message |
|---|---|---:|---:|---:|---|
| `₱ 15,000.50` | taken/normalized | `15000.50` | `15,000.50` | `15000.5` | none |
| ` 15000.50 ` | taken/normalized | `15000.50` | `15,000.50` | `15000.5` | none |
| `15,000.50` | taken/normalized | `15000.50` | `15,000.50` | `15000.5` | none |
| `PHP 15000.50` | taken/normalized | `15000.50` | `15,000.50` | `15000.5` | none |
| `15000.50.25` | refused/erased | empty | `0.00` | `0.0` | none |

Evidence: `h7-peso-after-keys.png`, `h7-peso-after-leaving.png`, `h7-spaces-after-keys.png`, `h7-spaces-after-leaving.png`, `h7-grouped-after-keys.png`, `h7-grouped-after-leaving.png`, `h7-php-after-keys.png`, `h7-php-after-leaving.png`, `h7-twodots-after-keys.png`, `h7-twodots-after-leaving.png`.

## H8 — exact mismatch message

The exact sentence pattern `the box shows ..., but the draft stored ...` **never appeared** during H5-H7. Every `messages` array in `h5-h8-results.jsonl` is empty.

Nevertheless, silent mismatches were observed:

1. In H5 percent cases, after leaving the box the visible value was `0.00` while the draft had `420.0` or `42.0`; no mismatch warning appeared.
2. In the earlier H3 run at this same commit, visible surcharge was `1,500.00` while the scratch draft carried `penalty_overrides.surcharge = 1500.0025`; no mismatch warning appeared in that run either. Thus a wrong raw amount was written silently, though that evidence belongs to the H1-H4 phase and is not an H5-H8 trigger of the exact message.

## Concise fault list

- **Percent click fault:** zero percent box + click + `42` => `420` while typing, saved `420.0`.
- **Percent post-blur mismatch:** both percent variants visibly reverted to `0.00` while draft retained the typed numeric value; silent.
- **Whole-number click fault:** zero whole-number box + click + `42` => `04`, saved `4`.
- **Comma behavior:** matches review expectations exactly as numeric saved values, silently.
- **Paste behavior:** currency symbol, whitespace, grouping, and PHP prefix are normalized; malformed two-dot paste is refused silently back to zero.
- **Exact mismatch message:** not observed.
