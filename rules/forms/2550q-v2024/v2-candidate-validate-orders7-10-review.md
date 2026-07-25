# 2550Q April 2024 v2 candidate decision — Validate orders 7–10

Status: test-only candidate decision. This document authorizes only official
main-form `Validate` orders 7–10 under the `official` profile of
`2550q-v2024-p7.9.6.0`. It is not a reviewed rule set, a complete validation
implementation, a filing-safe decision, or a production capability claim.

## Evidence identity

The inspected `BIR-Form2550Qv2024.hta` is the `official-hta-runtime` asset
pinned at `rules/forms/2550q-v2024/manifest.json#/official_assets/1`, SHA-256
`3a5a4d3b2b342a4dfc55a05c69b560fb9be47af6451d8c651d19e1d33406ec70`.
The enclosing function is pinned at
`fixtures/validation-function-inventory-v796.json#/main_validate`,
HTA lines 8407–8620. Direct inspection narrows this decision to lines
8474–8493.

The four controls are declared at HTA lines 822–879 and recorded at
`fixtures/runtime-control-inventory-v796.json#/static_controls/25` through
`#/static_controls/28`. Their corresponding v1 field records are
`fields.json#/fields/66`, `#/fields/74`, `#/fields/71`, and `#/fields/72`.
The profile-loading path supplies address, ZIP, and contact values at HTA lines
9169–9177, and supplies either the stored email or the exact empty string at
lines 9180–9186.

## Official-profile executable scope

Direct inspection establishes this contiguous ordered sequence:

1. Lines 8474–8478 compare raw `taxpayerAddress` to `""`. A match alerts
   `Please enter a valid Taxpayer's Registered Address on Item 10.` and
   returns.
2. Lines 8479–8483 compare raw `taxpayerZip` to `""`. A match alerts
   `Please enter a valid Taxpayer's ZIP Code on Item 10A.` and returns.
3. Lines 8484–8488 compare raw `taxpayerContactNumber` to `""`. A match
   alerts `Please enter a valid Taxpayer's Contact Number on Item 11.` and
   returns.
4. Lines 8489–8493 compare raw `taxpayerEmailAddress` to `""`. A match alerts
   `Please enter a valid Taxpayer's Email Address on Item 12.` and returns.

The candidate rule IDs are `2550q-validate-address`,
`2550q-validate-zip`, `2550q-validate-contact`, and
`2550q-validate-email`, at official orders 7 through 10 respectively. Their
v1 validation and synthetic negative-case records are
`validations.json#/rules/6` through `#/rules/9` and
`fixtures/negative-cases.json#/cases/6` through `#/cases/9`.

## Raw-string decision

The official predicates read each DOM control's raw `.value` and compare only
with the exact empty string. The v2 bindings therefore perform no
normalization, trimming, case conversion, numeric conversion, length check,
ZIP/contact syntax check, or email syntax check. Whitespace-only text passes,
and any nonempty email text passes. This intentionally preserves the official
Item 12 defect recorded as `incorrect-official-behavior` in
`validations.json#/rules/9`.

The controls exist in the official DOM, so this decision covers present text
only. Present blank text is canonical blank and triggers `is-empty`; adapter
absence remains a separate fail-closed boundary decision. Although the markup
starts these controls disabled and the profile loader normally populates them,
the executable rules consume the supplied raw strings and do not model profile
loading as a calculation or workflow transition.

## Official evaluation policy

The existing `stop-effects-after-first-blocking-issue` policy applies. Orders
7–10 remain in the expected and evaluated inventory, while only the first
matching blocking issue is emitted. A failure at any earlier executable
Validate order suppresses their effects; within this slice, order 7 suppresses
orders 8–10, order 8 suppresses orders 9–10, and order 9 suppresses order 10.

The candidate may execute only in generated tests. Validate remains incomplete
after order 10 and must not be exposed as an operational workflow action.

## Filing-safe unresolved

No filing-safe decision establishes address, ZIP, contact-number, or email
syntax and normalization policy; corrected message wording; or whether all
matching issues should be emitted. Filing-safe behavior remains explicitly
unresolved and never falls back to official compatibility behavior.

## Explicit exclusions

Validate orders 11 onward, schedule validation, calculations other than the
separate candidate-only order-3 parsing outputs, repeating groups, workflow
transitions, serialization artifacts, editable-save and Final Copy
materialization, queueing, submission, focus behavior, and production
selection remain unresolved or absent.

This decision does not change reviewed metadata, the reviewed provider
registry, form migration status, release evidence, capability flags, or filing
readiness.
