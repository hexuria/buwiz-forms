# 2550Q April 2024 v2 candidate evidence - Validate order 3

Status: source behavior resolved and translated into the candidate-only v2
official profile. This evidence/translation record authorizes only that
candidate implementation and its deterministic fixtures. It does not authorize
a reviewed rule set, a filing-safe branch, a serialization artifact, or a
production capability.

## Evidence identity

The inspected runtime is the exact `official-hta-runtime` asset pinned at
`manifest.json#/official_assets/1`, SHA-256
`3a5a4d3b2b342a4dfc55a05c69b560fb9be47af6451d8c651d19e1d33406ec70`.
The file bytes were re-hashed before this review and matched that pin.

The main `validate` function is pinned by
`fixtures/validation-function-inventory-v796.json#/main_validate` at HTA lines
8407-8620. Validate order 3 is the Item 4 branch at lines 8444-8453. Its input
and clock setup is at lines 8409-8426.

Corroborating repository records are:

- `validations.json#/rules/2`;
- `fixtures/negative-cases.json#/cases/2`;
- `fields.json#/fields/53` for
  `frm2550qv2024:RtnPeriodToNo4`;
- `fields.json#/fields/96` for `frm2550qv2024:txtYearNo2`;
- `fixtures/runtime-control-inventory-v796.json#/static_controls/14` for the
  return-period-to control; and
- `fixtures/runtime-control-inventory-v796.json#/static_controls/8` for the
  year-ended control.

The v1 validation and negative fixture preserve the correct message and broad
exception summary, but the executable coercion details below come from direct
inspection of the pinned HTA.

## Exact official behavior

At function entry, before any ordered validation branch, the HTA performs these
steps:

1. Read `.value` from `frm2550qv2024:RtnPeriodToNo4` as a raw string.
2. Read `.value` from `frm2550qv2024:txtYearNo2` and apply
   `parseInt(value, 10)` to produce `yearEnded`.
3. Split the return-period string on `/`.
4. Apply `parseInt(component, 10)` to components 2, 0, and 1 to produce,
   respectively, `rtnPeriodYear`, `rtnPeriodMonth`, and `rtnPeriodDay`.
   Components after index 2 are ignored.
5. Read the local system date once with `new Date()`, then copy its local
   year, month, and day into `new Date(currentYear, currentMonth - 1,
   currentDay)`. Time-of-day is therefore discarded.
6. Construct `returnPeriodDate` with
   `new Date(rtnPeriodYear, rtnPeriodMonth - 1, rtnPeriodDay)`.

After Validate order 1 (year type) and order 2 (quarter), order 3 evaluates:

```text
if returnPeriodDate > currentDate:
    allow when yearEnded is exactly 2025 and the raw parsed return month/year is
      11/2024, 12/2024, or 1/2025
    otherwise alert "Should not accept advance filing." and return
```

The exception is nested inside the future-date branch. A return date equal to
or earlier than the local current date proceeds regardless of `yearEnded`.
The exact issue is blocking, has message
`Should not accept advance filing.`, is associated with Item 4
`frm2550qv2024:RtnPeriodToNo4` and Item 2
`frm2550qv2024:txtYearNo2`, and occupies official Validate order 3. It precedes
the Item 7 TIN, Item 8 RDO, and Item 9 taxpayer-name checks at orders 4-6.

The HTA alerts and immediately returns from `validate`; later checks do not run.
The candidate's already-reviewed
`stop-effects-after-first-blocking-issue` policy may preserve the first visible
issue while still inventorying all applicable rules, but an order-3 fixture
must prove that order 3 suppresses issues from orders 4-6.

## Coercion, blanks, and malformed input

The source uses JavaScript number/date semantics, not a strict
`MM/DD/YYYY` parser:

- `parseInt(value, 10)` accepts leading whitespace, an optional sign, and the
  maximal leading decimal-digit prefix. Suffix text does not invalidate an
  otherwise parsed prefix.
- A blank string, a component with no leading decimal integer, or a missing
  component produces `NaN`.
- `new Date(year, monthIndex, day)` is a local civil-date constructor. It
  normalizes out-of-range month/day values instead of rejecting them. Numeric
  years 0-99 receive JavaScript's 1900 offset.
- Any `NaN` component produces an invalid Date. Comparing that invalid Date
  with `currentDate` using `>` is false, so order 3 emits no issue and
  processing continues to order 4.
- The hard-coded exception compares the original parsed integer components,
  not normalized components read back from `returnPeriodDate`. Thus an
  overflow-normalized date and the exception can disagree.
- `yearEnded` uses the same prefix-accepting `parseInt(..., 10)` behavior.
  Blank or malformed text becomes `NaN` and cannot satisfy strict numeric
  equality with 2025.

The return-period controls are disabled by default at HTA lines 652-658, and
ordinary Calendar/Fiscal quarter changes generate their values at lines
8826-9035. They are not universally generated or immutable:
`enableShortPeriod("yes")` enables both Item 4 controls at lines 8389-8390, and
the declarations attach no lexical/date blur validator to either control.
Malformed and overflow inputs therefore cannot be dismissed as unreachable
from the official UI.

Illustrative deterministic cases, all with local current date 2024-10-15 and
valid order-1/order-2 selections, are:

| Return period to | Year ended | Order-3 result | Reason |
| --- | ---: | --- | --- |
| `10/15/2024` | `2024` | proceed | Equal dates are not greater-than. |
| `10/16/2024` | `2024` | issue | Future and outside the exception. |
| `11/30/2024` | `2025` | proceed | Exact November 2024 exception. |
| `11/30/2024` | `2024` | issue | Exception requires `yearEnded === 2025`. |
| `01/31/2025` | `2025suffix` | proceed | Year prefix parses to 2025; exact January 2025 exception. |
| `02/01/2025` | `2025` | issue | February is outside the exception. |
| empty string | `2025` | proceed | Missing components produce an invalid Date; `Invalid Date > currentDate` is false. |
| `not-a-date` | `2025` | proceed | Non-numeric components produce an invalid Date. |
| `13/01/2024` | `2025` | issue | Date normalizes to 2025-01-01, but raw month 13 is not an exception month. |
| `1x/32x/2025suffix/ignored` | `2025suffix` | proceed | Prefixes parse; date normalizes to 2025-02-01, while the exception still sees raw month/year 1/2025. |
| `01/01/25` | `2025` | proceed | Numeric year 25 constructs local year 1925. |

These are official-compatibility observations only. They are not recommended
filing-safe date acceptance rules.

## Required candidate fixture contract

An executable translation needs one pinned local-current-date context value
materialized once per evaluation, plus the two exact raw fields. The minimum
fixture set is:

### Positive/no-issue

- return date earlier than current date;
- return date exactly equal to current date;
- each of the three exception arms independently: November 2024, December
  2024, and January 2025, all future relative to the fixture clock and with
  `yearEnded` parsing to 2025;
- blank return-period text;
- non-numeric/missing return-period components;
- a leading-whitespace/signed/prefix-suffixed parse case;
- an overflow case that normalizes to a future date but is allowed because the
  exception examines the original parsed month/year; and
- a 0-99 numeric-year case proving JavaScript's 1900 offset.

### Negative/order-3 issue

- a one-day-future ordinary date;
- each exception month with `yearEnded` not parsing to 2025;
- a future February 2025 date with `yearEnded` 2025;
- a month/day overflow that normalizes into the future but whose raw parsed
  month/year misses the exception; and
- a suffix-only or blank `yearEnded`, proving `NaN` does not satisfy the
  exception.

### First-error ordering

- order 1 failing together with an order-3 future date and failures at orders
  4-6: only the order-1 issue is visible;
- order 1 passing, order 2 failing together with an order-3 future date and
  failures at orders 4-6: only the order-2 issue is visible; and
- orders 1-2 passing, order 3 failing together with failures at orders 4-6:
  only `Should not accept advance filing.` is visible and focused first.

Every candidate fixture now supplies deterministic values for the two newly
declared raw fields and the required `local-current-date` context value. The
context snapshot is captured once by the caller per evaluation and its
fingerprint is bound into the input and expected report.

## Translation decision

The candidate runtime now has four closed, deterministic expression nodes:

- literal `/` split/component access;
- radix-10 JavaScript integer-prefix parsing, with unusable NaN/infinite values
  represented by the nullable integer sentinel;
- JavaScript numeric local civil-date normalization, including the 0-99 year
  offset and Invalid Date sentinel; and
- canonical local-date conversion into the same civil-day ordinal.

`2550q-validate-future-period` uses these nodes directly from the two raw text
fields. It does not outsource parsing to an adapter. Its predicate first guards
the nullable constructed date, compares it to the single captured context
date, and then applies the exception to the original parsed month/year
components. Adversarial runtime tests and candidate fixtures bind prefix
parsing, missing components, overflow normalization, huge numeric input,
legacy numeric years, all three exception arms, ordinary future rejection, and
first-error ordering.

This remains a candidate translation:

- `review_status` remains `candidate`;
- `filing_safe` remains `unresolved`;
- `serialization.artifacts` remains empty; and
- the reviewed registry and all renderer, release, migration, capability,
  Final Copy, queue, and submission state remain untouched.
