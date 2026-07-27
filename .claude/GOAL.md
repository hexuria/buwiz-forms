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
- remote baseline `de828fd05ce27afa5c71ffd88c7a8bb2b3f9a8a5`;
- portable library-foundation checkpoint
  `f2e78ef52ce1bc9ff9dabecea97590c09ce84c46`;
- local-artifact ignore checkpoint
  `e772d390fd6bfacb983157cdb936517e400f73ed`;
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

## Completed packet phase

Phase 1 is complete in the working tree:

- all **43** exact form identities have reviewed packets at
  `evidence/validation-rules/packets/v1`;
- the set contains **130** canonical JSON files and has aggregate digest
  `9fa6df6c6657166140b37b67d4d7ca382ce3dc4986fb13f4f6a8caf0e828ac74`;
- portable checking succeeds while honestly reporting
  `full_upstream_verified: false`;
- checking against the external content-addressed vault succeeds with
  `full_upstream_verified: true`;
- the external reviewed ledger binds all 43 planned packet digests and has
  SHA-256
  `b79f9c547199b6261506588787f75fae861fe1480cfc6a0b2a34df4f62155a4f`;
- the external vault catalog remains outside the repository and has SHA-256
  `5692989438d3a92c4b9281f6726fa4c947694482bec8527d6efbc7bb82aa08e7`;
- a 1701Q format proof verified both with and without the vault, then imported
  exactly its two reviewed derived files into a fresh external staging root;
  and
- mapped-drive publication now canonicalizes the packet-set root before child
  containment checks, covering the Windows `X:` to extended-UNC spelling
  change without weakening path-escape rejection.

The packets retain every evidence gap. Review means the value-free packet
projection was independently checked; it does not mean official behavior,
filing safety, or a v2 snapshot was reviewed.

## Current slice

Process the first ordered form, `1701q-v2018`, in one packet-backed external
workspace. Its skeleton tree digest is
`0309add72b8066b36693a05211bd6728ba950ce855f83a44d211760b1e1a0f97`.
The locked source reconciliation now accounts for all 172 fields, all 40
validation records as 33 executable plus seven source-backed classifications,
all 19 calculations, two workflow states, one executable workflow transition,
and seven workflow classifications. Portable reviewed evidence now closes the
TIN checksum, selected-index/RDO state, the documented official no-op, the
ordered JavaScript rounding chain, and all three item-46 tax schedules,
including the pre-2018 branch.

Working-tree checkpoint, 2026-07-27:

- the generic v2 IR/runtime/codegen now represents exact field-event programs,
  ordered calculation/effect interleaving, mutable calculation writeback,
  exact money/year normalization, and finite-only selected calculation output;
- the first independent 1701Q audit reverified every existing source lock, the
  204-attribute/203-pair event inventory, all 19 finite calculation envelopes,
  and the recovered pinned HTA bytes against the official executable;
- those new event and finite-domain records remain **unlocked review drafts**.
  They do not authorize candidate generation, and their required executable
  fixture packet is not complete;
- the external 1701Q authoring generator and determinism script now invoke a
  fail-closed authorization guard before their first mutation. The guard
  requires an explicit reviewer identity, reviewed decision and UTC timestamp,
  candidate-generation authorization, exact dependency pins, and a complete
  source lock. Its dedicated verifier proves the present drafts are rejected
  while the four protected artifacts remain byte-identical; and
- external/tracked filesystem readers are being consolidated behind retained,
  scope-explicit capabilities. Until the adversarial root/child replacement
  and in-place-mutation tests pass and every production caller uses that API,
  portable evidence verification is the active prerequisite.

Before integrating the candidate:

- finish the field-event review surface, including the unresolved dynamic
  `.chkTin input` selector, conditional branch observations, and the four
  intentionally unavailable anonymous handlers;
- materialize and execute the finite-domain fixture specifications, including
  concrete endpoint inputs, just-outside-envelope rejection, save/reopen and
  external-DOM overflow, no-partial-sibling-write, and drag/drop disposition;
- review and source-lock both records as one atomic authorization transaction,
  then regenerate through the guarded path; and
- run the candidate through the generic audit, deterministic generation,
  runtime fixtures, dry-run source-set roll, and packet-backed integration
  gates.

The official `round(this, 2); compute...` order maps ordinary invalid finite
text and overprecision to `0.00` before arithmetic, while signed Infinity
produces an official malformed non-decimal string. Do not replace that behavior
with the provisional `on_invalid=error` authoring shorthand or silently map
non-finite values to zero/absence. Keep the candidate external until the
reachable behavior is reviewed and the exact finite envelope is enforced.

Every unobserved serialization occurrence remains explicitly evidence-blocked.
Do not skip to `1601eq-v2018` while 1701Q remains unresolved, and do not
integrate a skeleton or partially reconciled candidate.

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
