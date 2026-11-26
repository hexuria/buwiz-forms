# PR #403 acceptance B/E report — exact 7d491c57

Date: 2026-05-21
Repository: hexuria/buwiz-forms
Commit: `7d491c57dd073a86d933361088500ab03bc74837`
Worktree: `/home/ubuntu/forms-validation/money-accept-7d491c57`
Target: `/home/ubuntu/forms-validation/target-money-accept-7d491c57`
Proof: `/home/ubuntu/forms-validation/money-accept-proof`
Scratch DB: `/home/ubuntu/forms-validation/money-accept-proof/money-accept-validation.db`

Runtime validation only. Real visible maximized Linux X11 window; xdotool key events at 120 ms unless stated; paste used xclip plus real Ctrl+V. Scratch SQLite drafts were read after leaving each box. No Queue, no live send, no token exposure, no commit/push. Android emulator/watch untouched.

## Summary

- **B amount-entry cases: RIGHT** in all eight requested runs.
- **E 2551Q Number of Sheet/s Attached: RIGHT** for click entry and Tab entry; both showed and saved `42`.
- **E 1601FQ Schedule 1 row 1 Tax Rate: FAULT** for both click entry and Tab entry. While typing it showed `4` then `42`, but after leaving it visibly reverted to `0.00` while the scratch draft saved `42.0`. No warning sentence appeared.
- No exact mismatch sentence or any box sentence was observed in these B/E cases.

## B — 2551Q Schedule 1 row 1 taxable amount, starting 0.00

| case | after keys | after leaving | saved draft | sentence | result |
|---|---:|---:|---:|---|---|
| `99999.99` | `99999.99` | `99,999.99` | `99999.99` | none | RIGHT |
| `1000000.005` | `1000000.005` | `1,000,000.01` | `1000000.01` | none | RIGHT |
| `1234567890` then Left Left BackSpace | `123456790` | `123,456,790.00` | `123456790.0` | none | RIGHT |
| `99999999.99` run 1 | `99999999.99` | `99,999,999.99` | `99999999.99` | none | RIGHT |
| `99999999.99` run 2 | `99999999.99` | `99,999,999.99` | `99999999.99` | none | RIGHT |
| `99999999.99` run 3 | `99999999.99` | `99,999,999.99` | `99999999.99` | none | RIGHT |
| `99999999.99`, no delay | `99999999.99` | `99,999,999.99` | `99999999.99` | none | RIGHT |
| `99999999.99`, real paste | `99999999.99` | `99,999,999.99` | `99999999.99` | none | RIGHT |

## E — zero-entry in non-money boxes

### 2551Q Number of Sheet/s Attached, starting 0

| entry | after keys | after leaving | saved draft | sentence | result |
|---|---:|---:|---:|---|---|
| click, type `42` | `42` | `42` | `42` | none | RIGHT |
| Tab, type `42` | `42` | `42` | `42` | none | RIGHT |

### 1601FQ Schedule 1 row 1 Tax Rate (F), starting 0

| entry | after first key `4` | after keys `42` | after leaving | saved draft | sentence | result |
|---|---:|---:|---:|---:|---|---|
| click, type `42` | `4` | `42` | `0.00` | `42.0` | none | FAULT |
| Tab, type `42` | `4` | `42` | `0.00` | `42.0` | none | FAULT |

Answer to the narrow question: **YES** — on this PR head the 1601FQ rate box still shows `0.00` after leaving while `42.0` is saved in the draft. There is no visible warning sentence.

## Harness notes

The first automated attempt at the E rate cases exposed two harness-only issues: it did not open the collapsed Schedule row drawer quickly enough, and it initially read the wrong JSON key (`schedule_1` instead of the actual `schedule1`). I opened the drawer in the real window, corrected the read path, reset the box to zero through Ctrl+A typing `0`, and reran both rate cases. The corrected rows are the rows labeled `corrected` in the machine-readable evidence. This does not affect the product finding.

## Artifacts

- Recording: `/home/ubuntu/forms-validation/money-accept-proof/money-accept-be-validation.mp4`
- Curated machine rows: `/home/ubuntu/forms-validation/money-accept-proof/be-final-results.jsonl`
- Raw B rows: `/home/ubuntu/forms-validation/money-accept-proof/be-results.jsonl`
- Raw E rows including superseded harness-error rows: `/home/ubuntu/forms-validation/money-accept-proof/e-results.jsonl`
- Key screenshots:
  - `/home/ubuntu/forms-validation/money-accept-proof/b-inversion-keys.png`
  - `/home/ubuntu/forms-validation/money-accept-proof/b-inversion-leave.png`
  - `/home/ubuntu/forms-validation/money-accept-proof/b-big-paste-leave.png`
  - `/home/ubuntu/forms-validation/money-accept-proof/e-sheets-click-keys.png`
  - `/home/ubuntu/forms-validation/money-accept-proof/e-sheets-click-leave.png`
  - `/home/ubuntu/forms-validation/money-accept-proof/e2-rate-click-after4.png`
  - `/home/ubuntu/forms-validation/money-accept-proof/e2-rate-click-keys.png`
  - `/home/ubuntu/forms-validation/money-accept-proof/e2-rate-click-leave.png`
  - `/home/ubuntu/forms-validation/money-accept-proof/e2-rate-tab-keys.png`
  - `/home/ubuntu/forms-validation/money-accept-proof/e2-rate-tab-leave.png`
