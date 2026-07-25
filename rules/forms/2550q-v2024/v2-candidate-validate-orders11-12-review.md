# 2550Q April 2024 v2 candidate decision — Validate orders 11–12

Status: test-only candidate decision. This document authorizes only official
main-form `Validate` orders 11–12 under the `official` profile of
`2550q-v2024-p7.9.6.0`. It is not a reviewed rule set, a complete validation
implementation, a filing-safe decision, or a production capability claim.

## Evidence identity

The inspected `BIR-Form2550Qv2024.hta` is the `official-hta-runtime` asset
pinned at `rules/forms/2550q-v2024/manifest.json#/official_assets/1`, SHA-256
`3a5a4d3b2b342a4dfc55a05c69b560fb9be47af6451d8c651d19e1d33406ec70`.
The enclosing function is pinned at
`fixtures/validation-function-inventory-v796.json#/main_validate`, HTA lines
8407–8620. Direct inspection narrows this decision to lines 8494–8503.

The Item 13 controls are declared at HTA lines 897–914 and recorded at
`fixtures/runtime-control-inventory-v796.json#/static_controls/30` through
`#/static_controls/33`. Their v1 field records are
`fields.json#/fields/67` through `#/fields/70`.

The Item 14 Yes control and Item 14A text control are declared at HTA lines
935–957 and recorded at
`fixtures/runtime-control-inventory-v796.json#/static_controls/34` and
`#/static_controls/36`. Their v1 field records are
`fields.json#/fields/34` and `#/fields/64`. The Item 14 No control at runtime
inventory `#/static_controls/35` is part of the raw UI/XML coherence boundary,
but the official order-12 predicate never reads it.

## Official-profile executable scope

Direct inspection establishes this contiguous ordered sequence:

1. Lines 8494–8498 read the `.checked` property of all four Item 13 controls.
   When all four are false, the function alerts
   `Please choose taxpayer classification on Item 13.` and returns.
2. Lines 8499–8503 read only the Item 14 Yes control's `.checked` property.
   When it is true and raw Item 14A `.value == ""`, the function alerts
   `Specify cannot be empty field on item 14A.` and returns.

The candidate rule IDs are `2550q-validate-classification` and
`2550q-validate-treaty`, at official orders 11 and 12. Their v1 validation and
synthetic negative-case records are `validations.json#/rules/10` and
`#/rules/11`, and `fixtures/negative-cases.json#/cases/10` and `#/cases/11`.

## Raw-control decision

The four classification controls and Treaty Yes are consumed as exact DOM
booleans. The v2 candidate therefore accepts only exact raw `true` and `false`
for those declared boolean fields; missing, blank, or malformed adapter
authority fails closed before official evaluation.

Item 14A is consumed as its raw string with only an exact empty-string test.
There is no trimming, case conversion, normalization, syntax validation, or
length validation in this rule. Whitespace-only text passes.

The official rule does not require either Item 14 radio to be selected. If
Treaty Yes is false, order 12 proceeds even when Treaty No is also false and
Item 14A is blank. The candidate must preserve that official behavior rather
than inventing a selection requirement.

## Official evaluation policy

The existing `stop-effects-after-first-blocking-issue` policy applies. Orders
11–12 remain in the expected and evaluated inventory, while only the first
matching blocking issue is emitted. A failure at an earlier executable order
suppresses both effects; an order-11 failure suppresses the order-12 effect.

The candidate may execute only in generated tests. Validate remains incomplete
after order 12 and must not be exposed as an operational workflow action.

## Current-date boundary

Neither order 11 nor order 12 reads the clock. The candidate's required
`local-current-date` context remains solely the separately reviewed order-3
dependency. Production capture of that local date is still unresolved and is
not implicitly authorized by this decision.

## Filing-safe unresolved

No filing-safe decision establishes whether an Item 13 classification can be
derived from a taxpayer profile; whether exactly one classification must be
selected; whether Item 14 requires an explicit Yes/No choice; or how Item 14A
should be trimmed, normalized, or validated. Filing-safe behavior remains
explicitly unresolved and never falls back to official compatibility behavior.

## Explicit exclusions

Validate order 13 onward, schedule validation, calculations other than the
separate candidate-only order-3 parsing outputs, repeating groups, workflow
transitions, serialization artifacts, editable-save and Final Copy
materialization, queueing, submission, and production selection remain
unresolved or absent.

This decision does not change reviewed metadata, the reviewed provider
registry, form migration status, release evidence, capability flags, or filing
readiness.
