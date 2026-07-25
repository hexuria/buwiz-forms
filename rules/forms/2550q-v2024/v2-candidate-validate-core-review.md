# 2550Q April 2024 v2 candidate decision — partial Validate core

Status: test-only candidate decision. This document authorizes only the five
source-ordered main-form `Validate` checks for Items 1, 3, 7, 8, and 9 below
under the `official` profile of `2550q-v2024-p7.9.6.0`. Its scope is orders 1,
2, and 4-6 only; the separate
`v2-candidate-validate-order3-research.md` evidence/translation record owns
order 3. This is not a reviewed rule set, a complete validation implementation,
a filing-safe decision, or a production capability claim.

## Evidence identity

The inspected `BIR-Form2550Qv2024.hta` is the `official-hta-runtime` asset
pinned at `rules/forms/2550q-v2024/manifest.json#/official_assets/1`, SHA-256
`3a5a4d3b2b342a4dfc55a05c69b560fb9be47af6451d8c651d19e1d33406ec70`.
The enclosing Validate function is recorded at
`fixtures/validation-function-inventory-v796.json#/main_validate`. Direct
inspection narrows the five clauses to lines 8428–8441 and 8459–8473. The
translations below also bind to the corresponding v1 validation,
negative-case, field, and runtime-control JSON pointers.

## Official-profile executable scope

Direct inspection establishes this ordered partial sequence:

1. Lines 8428–8432 read the boolean `.checked` properties of `calendarNo1`
   and `fiscalNo1`; when both are false they alert
   `Please choose a year type on Item 1.` and return.
2. Lines 8435–8441 read the boolean `.checked` properties of
   `OptQuarter1` through `OptQuarter4`; when all four are false they alert
   `Item 3 is a required field.` and return.
3. Lines 8459–8463 compare the raw values of `txtTIN1`, `txtTIN2`, `txtTIN3`,
   and `branchCode` to the exact empty string, alert
   `Please enter a valid TIN number on Item 7.`, and return `false`.
4. Lines 8464–8468 compare raw `txtRDOCode` only to the exact empty string,
   alert `Please enter a valid RDO Code on Item 8.`, and return `false`.
   Unlike Save preflight, the string `000` passes this Validate clause.
5. Lines 8469–8473 compare raw `taxpayerName` to the exact empty string, alert
   `Please enter a valid Taxpayer Name on Item 9.`, and return `false`.

The candidate rule IDs and source orders are:

- `2550q-validate-year-type`, order 1;
- `2550q-validate-quarter`, order 2;
- `2550q-validate-tin`, order 4;
- `2550q-validate-rdo`, order 5; and
- `2550q-validate-name`, order 6.

Lines 8444-8453 and their order-3 translation are intentionally outside this
decision. They are source-bound by
`v2-candidate-validate-order3-research.md`; this document neither duplicates
nor overrides that evidence. The five rules here may execute in generated
tests, but the Validate phase remains incomplete after order 6 and must not be
exposed as an operational workflow action.

## Raw-radio decision

The six radio controls are singleton raw fields. The HTA declarations are
recorded at
`fixtures/runtime-control-inventory-v796.json#/static_controls/5`,
`#/static_controls/6`, and `#/static_controls/9` through
`#/static_controls/12`; the corresponding v1 field records are
`fields.json#/fields/16`, `#/fields/25`, and `#/fields/37` through
`#/fields/40`.

The official predicates read `.checked`, not the controls' static HTML
`value`. The raw adapter therefore exposes each checked state losslessly as
the exact text token `true` or `false`, matching the v1 string-storage,
boolean-logical inventory. The official v2 branch performs no normalization,
coerces only those two tokens to boolean, and rejects absent, blank, or
unknown tokens at the adapter/evaluator boundary. An unchecked control is
canonical boolean false, not an empty field. The executable predicates compare
every canonical boolean explicitly with false in the same left-to-right order
as the source.

## Raw-string decision

The six Item 7–9 controls reuse the reviewed singleton raw-string bindings
from the Save candidate. This Validate slice adds no normalization, trimming,
numeric conversion, TIN checksum, length check, or case conversion.
Whitespace-only text therefore passes. Present blank text is canonical blank
and triggers the typed `is-empty` predicate. No absent-input fixture is
claimed for the radio or text controls because the official DOM always
supplies them; adapter-level absence remains a separate fail-closed boundary
decision.

## Official evaluation policy

The existing `stop-effects-after-first-blocking-issue` policy applies within
the Validate request. These five predicates and the separately evidenced
order-3 predicate remain in the expected and evaluated inventory, while only
the first matching blocking issue is emitted. This preserves the official
ordered alert-and-return behavior through order 6 without expanding this
decision's evidence scope.

## Filing-safe unresolved

No filing-safe decision establishes year-type or quarter-selection policy,
TIN syntax or checksum requirements, RDO-code policy, taxpayer-name
normalization, corrected messages, or whether all matching issues should be
emitted. Filing-safe behavior remains explicitly unresolved and never falls
back to official compatibility behavior.

## Explicit exclusions

Validate orders 7 onward, all schedule validation, calculations other than the
candidate-only order-3 parsing outputs, all repeating groups, workflow
transitions, serialization artifacts, editable-save and Final Copy
materialization, queueing, submission, focus behavior, and production
selection remain unresolved or absent.

The candidate must remain generated only under `cfg(test)` and excluded from
reviewed metadata, the reviewed provider registry, and every production
lookup. It does not change form migration status, release evidence, capability
flags, or filing readiness.
