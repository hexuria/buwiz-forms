# 2550Q April 2024 v2 candidate decision — Save preflight

Status: test-only candidate decision. This document authorizes only the three
source-ordered `initialValidateBeforeSave` checks below for the `official`
profile of `2550q-v2024-p7.9.6.0`. It is not a reviewed rule set, a complete
validation implementation, a filing-safe decision, or a production capability
claim.

## Evidence identity

The inspected `BIR-Form2550Qv2024.hta` is the `official-hta-runtime` asset
pinned at `rules/forms/2550q-v2024/manifest.json#/official_assets/1`, SHA-256
`3a5a4d3b2b342a4dfc55a05c69b560fb9be47af6451d8c651d19e1d33406ec70`.
The same identity and the `L9065-L9080` Save-preflight range are recorded in
`fixtures/validation-function-inventory-v796.json`. The typed translations
below bind to the corresponding v1 validation and negative-case JSON pointers.

## Official-profile executable scope

Direct inspection of `initialValidateBeforeSave`, lines 9065–9080, establishes
this complete three-branch sequence:

1. Lines 9066–9068 compare the raw values of `txtTIN1`, `txtTIN2`, `txtTIN3`,
   and `branchCode` to the exact empty string with JavaScript `==`, alert
   `Please enter a valid TIN number on Item 7.`, and return `false` on the
   first matching branch.
2. Lines 9070–9072 compare the raw `txtRDOCode` value to the exact string
   `000`, alert `Please enter a valid RDO Code on Item 8.`, and return `false`.
3. Lines 9074–9076 compare the raw `taxpayerName` value to the exact empty
   string, alert `Please enter a valid Withholding Agent's Name on Item 9.`,
   and return `false`. The incorrect “Withholding Agent” noun is preserved as
   official behavior.
4. Line 9079 returns `true` only after all three checks pass.

The candidate therefore makes only the `official` Save-preflight slice
executable. Rule IDs and reviewed order are:

- `2550q-save-tin`, order 1;
- `2550q-save-rdo`, order 2; and
- `2550q-save-name`, order 3.

## Raw-string decision

The six controls are singleton string fields. The official candidate applies
no trimming, case conversion, numeric conversion, currency formatting, or
other normalization. Present blank text remains distinguishable from an
absent input at the raw boundary; string coercion maps those states to
canonical blank and absent values respectively. The typed `is-empty`
predicates match both, while non-empty text is preserved exactly. In
particular, whitespace-only text is not blank under this official branch.

This decision intentionally does not copy the v1 `taxpayerName`
`decimal-money`/`NumWithComma` metadata. The HTA accesses that control's raw
`.value` as text in the Save guard, so those v1 properties are extraction
metadata errors for this executable slice.

## Official evaluation policy

The official candidate uses
`stop-effects-after-first-blocking-issue`. All three predicates remain part of
the complete expected/evaluated rule inventory, but after the first matching
blocking issue no later issue effect is emitted. This reproduces the ordered
alert-and-return behavior without hiding evaluation coverage.

## Filing-safe unresolved

No filing-safe decision has established TIN syntax/checksum requirements, RDO
handling, taxpayer-name normalization, corrected user-facing wording, or
whether all matching issues should be emitted. The filing-safe profile,
evaluation policy, all six field behaviors, and all three rule branches remain
explicitly unresolved and never fall back to the official branch.

## Explicit exclusions

The other 35 v1 validation records, all 27 calculations, all repeating groups,
workflow transitions, serialization artifacts, editable-save and Final Copy
materialization, queueing, submission, focus behavior, and production
selection remain unresolved or absent from this candidate. Existing legacy
record counts and unresolved mappings remain unchanged.

The candidate must be generated only under `cfg(test)` and excluded from
reviewed metadata, the reviewed provider registry, and every production
lookup. It does not change form migration status, release evidence, capability
flags, or filing readiness.
