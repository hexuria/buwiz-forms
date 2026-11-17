# 2550Q v2024 candidate serialization binding inventory review

## Scope

This review binds the complete value-free occurrence surface observed in the
pinned package 7.9.6.0 plaintext and decrypted Final Copy samples. It turns the
previous key-only inventories into a reproducible projection plan without
making any serialization artifact executable.

The generated inventory is
`forms/2550q-v2024/fixtures/serialization-binding-inventory-v796.json`.
`rules/tools/build-2550q-serialization-bindings.ps1` rebuilds it only when all
five upstream evidence hashes still match.

## Complete observed occurrence binding

The inventory contains all 160 plaintext occurrences. The first 159 also bind
the encrypted Final Copy staging loop; occurrence 160 is generated
`dateFiled`. Every observed live-control occurrence resolves to exactly one
runtime control and records:

- its package DOM ordinal;
- its exact serialized key;
- static-control or dynamic-group source projection;
- control kind and extracted logical type;
- candidate v2 field identity when one exists;
- plaintext Save placement, semantic format, and body codec;
- encrypted-staging placement, semantic format, and body codec; and
- source line references.

No values are copied. `values_emitted` is `false`.

All 159 live-control occurrences now resolve to candidate v2 fields: 119
singleton occurrences and 40 materialized occurrences of the 28 repeated
family descriptors. Generated `dateFiled` resolves separately to the required
`local-current-date` context value, so all 160 observed occurrences now have a
candidate value-source identity. Forty-four derived/alias fields and nine
workflow, credential, or UI-state fields are identity-complete but deliberately
`documented_only`; they are not executable value projections. The live modeled
count is asserted by the builder so a partial candidate cannot be mistaken for
complete identity coverage.

## Dynamic group decision

The 28 extracted unbounded key families are partitioned into seven top-level
groups:

1. Schedule 1 capital goods: nine children per row;
2. Schedule 3 creditable withholding: five children per row;
3. Schedule 4 advance VAT: six children per row;
4. Item 19 additional credits: two children per row;
5. Item 42 additional input tax: two children per row;
6. Item 47 additional input tax: two children per row; and
7. Item 56 additional deductions: two children per row.

The pinned sample contains legacy indices 0 and 1 for each schedule group and
no materialized additional-item row. Each observed schedule row must contain
every family exactly once in source child order. The builder rejects a missing,
extra, reordered, or differently indexed child.

All seven groups remain unbounded because the package exposes no maximum-row
guard. The observed numeric index is retained as evidence only. The app-owned
contract assigns and persists `assigned-stable-id` identities. Executable
serialization remains blocked until a separate review binds stable-instance
order to the official live DOM/display order.

## Artifact-specific codec decision

The normal plaintext Save loop applies legacy JavaScript `escape()` only to
`taxpayerName` and `taxpayerAddress`. Other text/select controls remain raw,
and radio/checkbox controls use JavaScript boolean text.

The encrypted Final Copy staging loop concatenates all 159 live-control values
directly, including taxpayer name and address. The inventory therefore keeps
plaintext and encrypted-staging body codecs separate for every occurrence.
They must not be collapsed into one shared codec table.

Generated `dateFiled` has two placements:

- final pseudo-div before the marker for editable/finalized plaintext Save;
- standalone metadata after the marker for encrypted Final Copy staging.

The date pattern is source-established as local `YYYY/MM/DD`, but no reviewed
clock, timezone, or v2 context-value projection exists.

## Non-executable decision

The inventory records the source-established marker variants and the pinned
external encryption-helper digest, but every artifact remains
`documented_only` and node-less. Execution is still blocked by:

- 53 identity-complete but documented-only derived/workflow value projections;
- a reviewed mapping from assigned stable-instance order to official live
  DOM/display order;
- separator, encoding, newline, and non-ASCII byte behavior;
- a reviewed production clock/timezone provider for the already-projected
  `local-current-date` context;
- filename, overwrite, path-confinement, and custody rules;
- the opaque encryption container and failure contract; and
- an independent filing-safe decision.

The reviewed registry remains empty. This inventory does not activate Save,
Final Copy, Submit, queueing, transport, release status, or any production
capability.
