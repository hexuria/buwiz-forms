# Offline eBIRForms validation rules

This directory is a revision-specific, evidence-backed inventory of the validation, calculation, serialization, and workflow behavior of the official Offline eBIRForms package.

The inventory records official behavior without assuming that behavior is legally or logically correct. Suspicious behavior is retained as `official_behavior` and separately assessed in `recommended_app_behavior`.

## Evidence policy

- Every nontrivial finding cites a hashed official asset, a source line/function, a runtime observation, an official document page/item, representative XML, or an existing repository implementation/test.
- Facts and inferences are distinguished with `confidence`, `evidence_type`, and notes.
- Unknown controls and serialized fields are preserved.
- Form identifiers include the revision; rules from different revisions are never merged.
- Local filesystem paths are evidence-locator metadata only. Hashes and source identifiers are the durable bindings.

## Completion states

`pending` means no revision has been selected. `researching` means the form is incomplete and must not be used as an implementation specification. `complete` means the per-form definition of complete in the goal has been audited and all remaining uncertainty is explicit in `gaps.md`.

Run `rtk cargo run --locked -p bir-rules-codegen -- validate-v1` for the
portable corpus audit. It reads all 659 JSON files, performs the 43-form
structural audit, and validates the 520 legacy-v1 JSON documents separately
from the 139 v2 JSON documents. The `rules/ir/v2` and `rules/schema/v2`
documents use cross-file Draft 2020-12 references and are also checked by
`rtk npm run rules:check`. The per-form audit records the exact verification
command used.

## Repository ownership and runtime consumption

This top-level directory is the canonical, append-only research and evidence
corpus. Do not move it into a Rust crate and do not load it directly in the
packaged application. It contains cross-language extraction tools,
machine-local evidence locators, prose assessments, and unresolved findings
that are intentionally unsuitable for automatic execution.

Reviewed executable contracts are compiled deterministically into
[`../crates/bir-rules`](../crates/bir-rules/README.md). `bir-core` supplies
form-specific adapters and owns trusted export/submission enforcement. The GPUI
desktop consumes the same local validation reports through `bir-core`; it does
not maintain a second frontend implementation.

When an official eBIRForms package changes, follow
[`UPDATING.md`](UPDATING.md). Existing snapshots are retained so old drafts and
official-package compatibility tests remain reproducible.

## Current coverage

All 43 forms in `FORM_BUILD_PRIORITY.md` are indexed once, in priorities 1–43,
with revision-specific manifests and explicit gaps. The final strict audit
covered 659 JSON files: 520 legacy-v1 and 139 v2. It also covered 9,592 typed
field entries, 2,007 validation rules, 623 calculations, and 1,354 negative
fixtures. Structural checks and all 216 schema-bearing documents passed.

The validator also rejects duplicate form IDs, priorities, or manifest paths; noncontiguous priorities; queue/order mismatches; and index/manifest identity, revision, or status mismatches.
