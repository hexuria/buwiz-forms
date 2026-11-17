# BIR Form 1702Q v2018C

This package records the offline validation, calculation, state, and serialization behavior of the January 2018 ENCS quarterly income-tax return for corporations, partnerships, and other non-individual taxpayers as shipped in Offline eBIRForms 7.9.6.0.

The exact runtime binding is `BIR-Form1702Qv2018C.hta` (SHA-256 `7184de87a76401da98da3df38dab9e29f848acf6150b425e36574f0a2443ab01`). The title and application name omit the trailing `C`, while filenames and serialized form type include it. Older installed `1702Q` and `1702Qv2008C` HTAs were excluded.

Contents:

- `fields.json`: 113 DOM occurrences and 112 serialized keys.
- `validations.json`: 39 ordered and event-driven rules.
- `calculations.json`: 25 calculation nodes.
- `workflow.json`: draft, validation, final-copy, submission, and retry states.
- `fixtures/`: synthetic positive, negative, boundary, and runtime-control evidence.
- `evidence.md`, `audit.md`, and `gaps.md`: provenance, independent checks, and explicit limits.

This is a research package only. It does not alter rendering, routing, migration status, release evidence, or filing capability.
