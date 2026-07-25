# 2550Q v2024 candidate static-control projection review

## Scope

This review classifies the 87 package 7.9.6.0 live static controls that were
still extraction landmarks after the first 32 singleton fields and 28 repeated
family descriptors were modeled. It completes the candidate identity binding
for every one of the 159 live serialized-control occurrences without making
any serialization artifact executable.

The classification is reproduced by
`rules/tools/update-2550q-v2-static-projections.ps1`. The updater fails unless
the value-free serialization binding inventory still contains exactly 87
unique unprojected `raw-static-control` occurrences, the v1 field and runtime
control inventories resolve each key exactly once, and the three categories
below remain disjoint and exhaustive.

## Raw lexical controls

Thirty-four controls have an app-owned raw capture path:

- 30 use the pre-existing closed core singleton raw inventory;
- amended-return Yes/No contributes two independently captured official radio
  controls; and
- short-period Yes/No contributes two independently captured official radio
  controls.

These 34 controls extend the executable candidate field surface. Their
official candidate branch may execute only lossless lexical capture.
Radio controls use boolean coercion because the package reads JavaScript
`true`/`false`; text and select controls use string coercion with an exact
empty string. No trimming, number/date parsing, calculation, or semantic
validation is approved by this decision. In particular, v1 records that call
a live text control computed or money-like remain raw text here until their
calculation policy is independently reviewed.

The four radio controls use independent hidden `InputState` buffers behind
visible Yes/No choices. New drafts do not infer them from the two typed
booleans. An explicit click atomically captures the complete pair as exact
`true`/`false` text and only then updates the typed value; reviewed import
restores the four exact raw values before evaluation.

## Derived and alias controls

Forty-four controls are totals, intermediate results, schedule totals,
page-two identity duplicates, or additional-row aggregate aliases. Their
identity, DOM control, plaintext/encrypted occurrence, and v1 descriptor are
source-established, but their authoritative expression, evaluation order,
duplicate-source policy, or lexical rendering is not fully reviewed in v2.

These 44 controls therefore remain identity-only documented bindings in the
value-free occurrence inventory. They are not candidate raw fields and gain no
executable coercion or calculation ID. A later review must add exact
calculation and lexical-rendering behavior before promoting any of them into
the executable field surface.

## Workflow, credential, and UI-state controls

Nine controls belong to package workflow or UI state rather than the tax-form
data model:

- `driveSelectTPExport`;
- `ebirOnlineUsername`;
- `ebirOnlineConfirmUsername`;
- `ebirOnlineSecret`;
- `frm2550qv2024:txtCurrentPage`;
- `frm2550qv2024:txtMaxPage`;
- `txtEmail`;
- `txtEnroll`; and
- `txtFinalFlag`.

They remain identity-only documented bindings outside the candidate field
surface. The candidate must not capture credentials into the tax draft,
promote navigation state into filing data, or infer finality from
`txtFinalFlag`.

## Candidate-only boundary

All 87 controls have:

- exact numeric provenance pointers into the v1 field, runtime-control, and
  serialization-binding inventories;
- candidate identities in the value-free serialization-binding inventory;
- no authority to Save, Final Copy, Submit, queue, transport, or release.

Only the 34 reviewed raw controls become executable fields, each with an empty
`serialized` projection and unresolved filing-safe behavior. Keeping the 53
derived/workflow controls outside `fields` is deliberate: an executable
profile requires every declared raw field to be supplied and canonicalized, so
a `documented_only` field inside that surface would make every evaluation fail
closed before validation.

After this projection, every live serialized-control occurrence has a
candidate identity, while the executable field surface contains 66 singleton
fields and 28 repeated-family descriptors (94 total). The separate
`dateFiled` review now projects generated metadata from the required immutable
`local-current-date` context snapshot, and all 66 singleton fields have exact
core/GPUI raw bindings. All three artifacts remain
`documented_only`, node-less, and unmaterializable. The reviewed registry
remains empty.
