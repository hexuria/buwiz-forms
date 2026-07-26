# Goal: build the 43-form candidate validation library

Make the validation-rules subsystem a portable, deterministic, fail-closed
library before doing any more application integration. The live sequence is
`docs/validation-rules/execution-plan.md`.

This goal is **library-first**:

```text
tracked v1 evidence
        |
        v
portable evidence packets
        |
        v
strict v2 candidate snapshots
        |
        v
bir-rules-codegen -> bir-rules
```

`bir-core`, GPUI, persistence, Final Copy, queueing, transport, and promotion
are frozen. The Wave 0 print objective remains preserved at
`.claude/GOAL.wave0-print-parity.md`.

## Rebaseline

Recorded 2026-07-26 from the authoritative checkout:

- branch `codex/print-preview-parity`;
- local `HEAD` and `origin/codex/print-preview-parity` both
  `de828fd05ce27afa5c71ffd88c7a8bb2b3f9a8a5`;
- v1 corpus: **43 forms / 9,592 fields / 2,007 validations /
  623 calculations**;
- v2 library: **1 candidate / 27 executable validations /
  1 executable calculation**;
- the 2550Q **53-projection classification is complete**. Those projections
  remain evidence classifications, not executable artifact authority.

These are baselines, not claims that any form is filing-safe, reviewed,
promoted, application-integrated, or complete as a candidate library entry.
The v1 manifest word `complete` means only that the evidence inventory is
complete or its gaps are explicit.

## Done when

The library objective is complete only when:

1. a deterministic, portable evidence-packet contract, verifier, safe importer,
   and external staging flow exist, and every one of the 43 indexed forms has a
   verified packet assembled from approved evidence;
2. all 43 forms have strict v2 **candidate** snapshots, processed in the exact
   order in `docs/validation-rules/execution-plan.md`;
3. record-level reconciliation reports zero unclassified and zero unresolved
   legacy records: each v1 record is represented by v2 or is source-backed and
   classified intentionally non-runtime under the closed reason enum;
4. all candidate modules are test-only, every filing-safe profile remains
   unresolved, serialization artifacts remain documented-only and node-less,
   and the reviewed registry remains empty;
5. code generation is deterministic, the packaged runtime reads no evidence
   JSON, and the library gates pass from tracked inputs on supported CI
   platforms; and
6. the completion report records the resulting executable counts instead of
   presuming that all 2,007 validations or 623 calculations became executable.

A candidate snapshot is not a reviewed ruleset, a promotion claim, a filing
authorization, or a statement that the form is finished.

## Machine condition

```sh
rtk cargo run -q --locked -p bir-rules-codegen -- status
```

The status report has three criterion classes:

- `Boundary`: production filing authority must remain closed;
- `ActiveLibrary`: deliverables that block this goal;
- `DeferredPromotion`: policy or evidence needed only for a later promotion.

Default status succeeds only when every `Boundary` and `ActiveLibrary`
criterion passes. It still reports deferred promotion work. Default success
proves the active library objective is complete with production boundaries
closed; it **does not** claim promotion readiness.

`status --require-promotion` is outside this goal. Never remove or weaken a
criterion to make either mode pass.

The recurring library gates are:

```sh
rtk cargo run --locked -p bir-rules-codegen -- validate-v1
rtk cargo run --locked -p bir-rules-codegen -- audit
rtk cargo run --locked -p bir-rules-codegen -- coverage --json
rtk cargo run --locked -p bir-rules-codegen -- operator-census --json
rtk cargo run --locked -p bir-rules-codegen -- reconciliation --json
rtk npm run rules:check
rtk cargo test --locked -p bir-rules -p bir-rules-codegen -p bir-rules-platform
rtk cargo fmt --all -- --check
```

`validate-v1` must retain the audited corpus baseline. Any count movement must
be explained by authoritative evidence; never adjust the validator to accept an
accidental change.

Record-level reconciliation is distinct from profile review. Filing-safe
profile branches may remain `unresolved` as deferred promotion work; a v1
record may not remain unclassified or unresolved at the candidate-library
gate.

## Current slice

Build and verify the portable evidence-packet contract and its
`verify-evidence`, `import-evidence`, and `stage-form` flows. This foundation
lands before any real packet or candidate expansion. Then materialize one
packet per indexed form. Do not start the ordered 43-form candidate pass until
packet verification, safe external staging, the aggregate manifest, and the
cross-platform portability gate are settled.

## Frozen until the 43-form baseline exists

- No edits to `crates/bir-core`, `crates/bir-desktop`, form views, persistence,
  Final Copy, queue, transport, or submission code for this objective.
- No 2550Q promotion, reviewed-registry entry, filing-safe decision, application
  wiring, or production evaluator selection.
- No new auxiliary worktrees and no switching validation work into an existing
  auxiliary worktree. Work only in the authoritative checkout.
- No worktree cleanup from Windows or the UNC view. Any later cleanup is a
  separate, explicit, **macOS-only** maintenance phase after the library
  baseline, with unique commits inspected before any removal.

## Constraints

- The five production switches remain closed: reviewed default designation
  returns `None`; generated candidate modules and the core candidate evaluator
  stay `#[cfg(test)]`; the reviewed registry stays empty; and
  `CheckedFinalCopyPayload::try_new` always returns
  `Err(MissingSerializationContract)`.
- 2550Q stays `candidate`; filing-safe stays `unresolved`; all three current
  serialization artifacts stay `documented_only`, node-less, and
  `values_emitted: false`.
- Preserve official defects in the official profile. A safer profile requires
  independent evidence and belongs to the deferred promotion objective.
- Never use the official submission path for discovery, real taxpayer data, or
  the live encrypted database.
- Digest rolls use the checked atomic tool. Never hand-roll or partially apply
  source-set pins.
- No broad clean, reset, checkout, prune, or deletion of uninspected `tmp/` or
  `test-results/`.
