# Review of PR #259 against issue #247 (2026-11-21)

PR #259: branch `feat/form-fields-combined` → `main`, about 130 files, +11,174 / −1,189. It replaces the open stack #250, #253–#257.

Coverage: every removed production line was read, plus the one-date, period and date-range registries, the profile-ask
persistence path, the 0605 date writer, `Facts.tsx` focus, the CI step, and the export and import wiring. The ~9,000 added lines of UI
and test code were **not** read line by line, so more findings are possible.

## Checklist status (#247)

| Box | Scope | In #259? | Gap |
|---|---|---|---|
| 1 (#250) | B1–B4, B6, B8–B10, B12–B21; Q6, Q13, Q12 | Mostly | B13 done. B12, B18, B21 and Q6 not traced to code or tests yet. Q12 is the owner's (skip). |
| 2 (#253) | Q2 one date per split date | Yes | `ONE_DATES`, `DateField`, `DateTouchField` |
| 3 (#254) | Q3 month+year field, Q3a ranges | Yes | `ONE_PERIODS` (18 pairs), `DATE_RANGES` (6), `periodField.tsx`, `MonthPicker` |
| 4 (#255) | Q1, Q1a, Q1b; B5, B7, B11 | Mostly | B11 not traced yet |
| 5 (#256) | Q4 segmented 2–4, dropdown 5+ | Yes | `Radios.tsx`, `choice.tsx` `Segmented` |
| 6 (#257) | Q5 message line only when needed | Yes | |
| 7 | Q9 gated boxes hidden | No | Work exists on branch `feat/form-fields-gated` (unfinished, not on #259's tip) |
| 8 | Q8 TIN boxes fill the row on a phone | No | Work exists on branch `feat/form-fields-tin` (stacked on gated) |

A B-number missing from the diff can just mean the code doesn't cite it. Look for the behaviour, not the label.

## Code findings

### F1 (important): a typed "0" escapes the atomic date refusal
- `crates/bir-app-api/src/screen_form_module/rows_as_needed.rs` around lines 756 (judging) and 857 (date helper) both use
  `filter(|v| !v.is_empty() && v != "0")`.
- Every one-date form treats "0" as a blank part, not only 1707's whole-number parts.
- Failure: 0619E due date, month and year filled, day typed "0" → the part becomes `None`, the judging loop `continue`s, no sentence
  at the month box, and the Queue gate doesn't stop it.
- Fix: keep zero-as-blank only for 1707's whole-number writer; refuse "0" elsewhere with the existing date sentence. Add a test for a
  "0" day on 0619E (and one on 0605 Item 4). Check what the ordinary writer saves after the skip.

### F2 (minor): profile-ask read errors are swallowed
- `crates/bir-app-api/src/screen_form_module/fact_asks.rs` around line 1157: `profile_asked` ends in `unwrap_or_default()`.
- Failure: a locked or corrupt settings DB → empty list → the "Save to profile?" ask repeats on every open, nothing logged.
- Fix: return a `Result`; log or surface the error. Add a test.

### F3 (minor): unescaped LIKE prefix
- `crates/bir-core/src/db/mod.rs` around line 1498: `settings_with_prefix` uses `key LIKE ?1 || '%'`.
- `_` is a LIKE wildcard and LIKE ignores ASCII case. Correct today only because the caller re-filters with `strip_prefix`.
- Fix: `substr(key, 1, length(?1)) = ?1`. Add a test with a key that differs only at an `_`.

### F4 (minor): tin not validated in profile-ask keys
- `crates/bir-app-api/src/screen_form_module.rs` around line 6722: `screen_form_profile_asked` accepts any `tin`; only the field is
  checked for `:`.
- Failure: a tin containing `:` makes `profile_ask:<tin>:<year>:<field>` keys from different taxpayers collide.
- Fix: validate the tin's shape (the app's existing TIN validator if there is one) before reading or writing. Add a test.

## Process items
- #250 and #253–#257 are open and superseded by #259.
- #259's local checks were run on Linux only; macOS and Windows rely on CI.
