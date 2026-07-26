# Validation-rules library execution plan

This is the **live sequencing document** for the validation-rules objective.
It supersedes the sequencing in:

- `docs/validation-rules/action-plan.md`, which is retained as historical
  2550Q promotion analysis;
- `docs/validation-rules/implementation-plan.md`, which remains useful for
  architecture and phase history but is not the current work queue; and
- `handoff.md`, which remains an append-only record of the earlier session.

The active goal is `.claude/GOAL.md`. No historical document authorizes
skipping this plan.

## Rebaseline: 2026-07-26

| Measure | Recorded baseline |
| --- | --- |
| Branch | `codex/print-preview-parity` |
| Local HEAD | `de828fd05ce27afa5c71ffd88c7a8bb2b3f9a8a5` |
| Origin branch | same commit |
| v1 evidence corpus | 43 forms / 9,592 fields / 2,007 validations / 623 calculations |
| v2 candidate library | 1 candidate / 27 executable validations / 1 executable calculation |
| Reviewed v2 rulesets | 0 |
| 2550Q 53-projection classification | complete; still value-free and non-authorizing |

The checkout was already dirty with unrelated user-owned changes. The commit
identity above records the source baseline underneath those changes; it is not
a clean-tree claim.

The v1 corpus is complete only in the evidence-inventory sense: a gap may be
explicit and still satisfy a v1 form manifest. It does **not** mean that 43
forms have executable candidates, reviewed rules, filing-safe behavior, or
application support. Do not describe any form as completed merely because its
v1 manifest says `complete`.

## Objective

Build a portable candidate library for all 43 indexed forms before resuming
application work:

```text
rules/ v1 evidence
        |
        v
portable evidence packet for each exact form identity
        |
        v
strict v2 candidate snapshot
        |
        v
deterministic test-only Rust module
        |
        v
bir-rules static runtime tests
```

The target is a **43-form candidate baseline**, not 43 promoted forms and not
100% executable coverage. Record-level reconciliation must account for every
v1 record: it is represented by v2 or source-backed and classified
intentionally non-runtime under the closed reason enum. The final gate permits
zero unclassified and zero unresolved legacy records. Filing-safe profile
branches may remain unresolved as deferred promotion work; that is a different
axis. Final executable totals are measured outputs, not numbers to manufacture.

## Scope and freeze

In scope:

- `rules/` evidence, schemas, candidate snapshots, fixtures, and portable
  packet inputs/outputs;
- `crates/bir-rules-codegen/` audit, census, packet, code-generation, drift,
  digest, and status tooling;
- `crates/bir-rules-platform/` narrow development-tool-only platform
  primitives, currently the safe Windows no-replace directory publisher;
- `crates/bir-rules/` form-agnostic static runtime and tests;
- validation-library commands, CI, and documentation.

Frozen until the 43-form candidate baseline is accepted:

- `crates/bir-core`, including form adapters, checked payload construction,
  persistence, Final Copy, queue, and submission boundaries;
- `crates/bir-desktop`, GPUI controllers, views, focus wiring, and summaries;
- 2550Q filing-safe decisions, review-status changes, registry population,
  production provider selection, promotion, and filing integration;
- print migration and form renderer work;
- new auxiliary worktrees or moving validation work into an existing auxiliary
  worktree.

Read-only boundary checks against frozen files remain required. A frozen path
may not be edited merely to make a library gate pass.

## Machine-checkable condition

```sh
rtk cargo run -q --locked -p bir-rules-codegen -- status
```

`status` reports three criterion classes:

| Kind | Meaning | Blocks default status |
| --- | --- | --- |
| `Boundary` | A production filing authority remains closed | yes |
| `ActiveLibrary` | A deliverable of this library objective | yes |
| `DeferredPromotion` | Policy/evidence needed only for later promotion | no |

Default status succeeds only when all `Boundary` and `ActiveLibrary` criteria
pass. Deferred promotion criteria remain visible. Default success proves the
active library objective is complete with production boundaries closed; it
does **not** claim promotion readiness.

`status --require-promotion` additionally requires every
`DeferredPromotion` criterion. That mode is not a target of this plan. Existing
criterion IDs remain present; never delete, weaken, or reclassify a real
boundary to obtain a passing exit code.

Recurring gates:

```sh
rtk cargo run -q --locked -p bir-rules-codegen -- status
rtk cargo run --locked -p bir-rules-codegen -- validate-v1
rtk cargo run --locked -p bir-rules-codegen -- audit
rtk cargo run --locked -p bir-rules-codegen -- coverage --json
rtk cargo run --locked -p bir-rules-codegen -- operator-census --json
rtk cargo run --locked -p bir-rules-codegen -- reconciliation --json
rtk npm run rules:check
rtk cargo test --locked -p bir-rules -p bir-rules-codegen -p bir-rules-platform
rtk cargo fmt --all -- --check
```

The v1 census remains 43 / 9,592 / 2,007 / 623. The fuller audit baseline
remains 659 JSON (139 v2), 1,354 negative fixtures, and 216 schema documents
until an intentional candidate-library change explains a v2-file increase.
Fields, v1 validations, v1 calculations, negative fixtures, and v1 schema
documents do not move as a side effect of library onboarding.

## Phase 0 — documentation and status rebaseline

Purpose: make the active objective unambiguous before implementation resumes.

Deliverables:

1. `.claude/GOAL.md` names the 43-form candidate library as the goal.
2. This file is the live ordered plan.
3. Historical plans and handoff material carry superseded pointers without
   erasing their record.
4. `status` distinguishes `Boundary`, `ActiveLibrary`, and
   `DeferredPromotion`, while directly asserting all five production guards.
5. Baseline commit, corpus counts, v2 counts, and the completed 53-projection
   classification are recorded without a promotion claim.

Gate: documentation and status semantics agree, all old criterion IDs remain
represented, and default status does not mistake deferred promotion work for
unfinished library work.

## Phase 1 — portable evidence packets

No additional form candidate starts until the packet contract is stable.

### Phase 1A — contract and safe tooling

Land the packet foundation before creating a real packet or changing the
corpus. Its command surface is:

```sh
rtk cargo run --locked -p bir-rules-codegen -- \
  discover-evidence-vault-sources --output <fresh-external-file> \
  [--search-root <external-dir> ...] [--dry-run]
rtk cargo run --locked -p bir-rules-codegen -- \
  verify-evidence-vault-source-map --source-map <relative-external-file>
rtk cargo run --locked -p bir-rules-codegen -- \
  write-evidence-capture-metadata --output <fresh-external-file> \
  --capture-session-id <id> --tool-commit <full-sha> \
  --source-map-sha256 <verifier-map-sha256> \
  --source-verification-sha256 <verifier-result-sha256> \
  --command-arg bir-rules-codegen \
  --command-arg verify-evidence-vault-source-map \
  --command-arg --source-map --command-arg ../<external-dir>/<source-map>.json \
  --capture-tool-version <text> --operating-system windows \
  --windows-version <text> --official-app-version <text> \
  --started-at-utc <timestamp> --finished-at-utc <timestamp> [--write]
rtk cargo run --locked -p bir-rules-codegen -- \
  acquire-evidence-vault --source-map <absolute-canonical-file> \
  --capture-metadata <absolute-canonical-file> \
  --vault-root <absolute-fresh-external-dir> \
  [--dry-run]
rtk cargo run --locked -p bir-rules-codegen -- \
  scaffold-evidence-review-ledger \
  --input <canonical-external-request> --vault-catalog <file> \
  --output <fresh-external-ledger> [--dry-run]
rtk cargo run --locked -p bir-rules-codegen -- \
  verify-evidence --packet <dir> [--vault <dir>]
rtk cargo run --locked -p bir-rules-codegen -- \
  import-evidence --packet <dir> --staging-root <dir>
rtk cargo run --locked -p bir-rules-codegen -- \
  stage-form --form-id <id> --packet <reviewed-packet> \
  --staging-root <dir>
rtk cargo run --locked -p bir-rules-codegen -- \
  stage-evidence-packet-review --form-id <id> \
  --review-ledger <candidate-ledger> --vault-catalog <file> \
  --output-root <fresh-review-dir>
rtk cargo run --locked -p bir-rules-codegen -- \
  build-evidence-packet --form-id <id> --output-root <fresh-dir> \
  --vault-catalog <file> --review-ledger <file> [--dry-run]
rtk cargo run --locked -p bir-rules-codegen -- \
  build-evidence-packet-set --output-root <fresh-dir> \
  --vault-catalog <file> --review-ledger <file> [--dry-run]
rtk cargo run --locked -p bir-rules-codegen -- \
  check-evidence-packet-set --packet-root evidence/validation-rules/packets/v1
```

Packet v1 is strict UTF-8 canonical JSON under a closed schema. It uses only
relative `/`-separated paths. Every payload is derived canonical JSON with a
declared hash and size; the packet carries an aggregate digest, packet and file
review statuses, and all four schema-required attestations. Upstream source
metadata is recorded, but upstream bytes remain outside the packet in an
optional vault.

The tracked packet set lives at
`evidence/validation-rules/packets/v1`, deliberately outside `rules/` so the
immutable v1 corpus census cannot be changed by packet materialization.
Vault bytes use content-addressed paths
`upstream/sha256/<first-two-hex>/<full-sha256>`. Packet construction reads
only an explicit vault catalog and review ledger; it must never make
machine-local paths from a v1 manifest operational.

`discover-evidence-vault-sources` resolves exact manifest locators and explicit
external search roots by pinned hash/size. Recursive search opens only files
whose leaf exactly matches an acquirable manifest locator; renamed assets must
be supplied through an explicit source map. Broad home roots are rejected. Any
unresolved acquirable identity produces a structured gap report and no source
map. Metadata-only and zero-size locators are never opened, and
symlink/reparse, taxpayer/save, credential, and submission-shaped paths are
never traversed.

`verify-evidence-vault-source-map` is the no-write capture boundary. It
reconciles the map with all 43 manifests and re-hashes every allowed source
through a handle-identity checked reader. Run it from the repository with a
portable relative source-map argument, recording exact start and finish times.
Only after it succeeds may `write-evidence-capture-metadata` record that exact
argv and both digests emitted by that exact verification result. Acquisition
rejects capture metadata whose source-map or verification digest differs from
the source inventory it independently recomputes. The writer accepts every
fact explicitly, reads no clock/user/host state, defaults to dry-run, and
publishes only with `--write`. Capture provenance accepts exactly the portable
four-argument verifier invocation shown above; an unrelated command, absolute
locator, backslash path, or in-repository path fails closed.

`acquire-evidence-vault` requires absolute, lexically normalized paths for its
source map, capture metadata, and fresh external vault root; the recorded
portable `../...` locator belongs only to the verifier provenance. It accepts
machine-local source locations only through its explicit external source map.
The shared closed asset-disposition policy admits non-empty official/runtime
assets, converts zero-size provenance records into metadata gaps, and forbids
copying dummy save, final-copy, or taxpayer-shaped bytes. The packet summary
still binds every declared asset's ID, kind, hash, size, disposition, and exact
manifest locator.

The recorded verifier locator must resolve to the exact source-map file being
acquired; equal bytes at another path do not satisfy provenance. All external
reads use one identity-checked, hard-link-free opened handle. Fresh file and
directory outputs are create-only and never delete by pathname on failure. If
construction cannot finish, the incomplete fresh output is left for explicit
inspection so a concurrent path substitution cannot turn automatic cleanup
into deletion of unrelated data. Successful Windows vault publication also
binds the published directory identity to the verified staging directory.

Before a v2 snapshot exists, a packet records its planned rule-set identity
with a tagged `planned` source state and a null runtime source-set digest. It
still binds the real tracked-v1 source-set digest. A builder must never
fabricate a v2 source digest merely to make a planned packet look pinned.

Packets must contain no taxpayer data, credentials, encryption secrets, or
invented official behavior. Do not copy untracked official assets merely to
make a packet self-contained; bind permitted evidence by digest and preserve a
gap where the tracked corpus cannot prove a fact. Verification rejects
symlinks and Windows reparse points, traversal or absolute paths, undeclared
files, digest/hash/size drift, and credential-, taxpayer-, or
submission-shaped content.

`verify-evidence` validates packet structure and derived payloads without a
vault, but its report must say `full_upstream_verified: false`. Supplying a
vault permits upstream hash/size verification; it does not copy upstream bytes
into the packet.

`import-evidence` requires the packet and every derived file to be reviewed. It
copies only derived files into a fresh staging root outside `rules/`, refuses
overwrite, and never writes the canonical corpus. `stage-form` mirrors
`rules/forms/<id>` into a fresh external staging root for safe builder work and
also refuses overwrite. With `--packet`, it additionally verifies the exact
reviewed packet identity, digest, planned/null v2 source state, and census,
then emits a schema-valid one-form `skeleton` workspace plus deterministic
machine/human handoffs. All legacy records begin explicitly unresolved;
serialization has no invented artifact identities or targets and its
value-free occurrence ledger remains a blocking handoff gap.

Review is a two-state transaction, not a self-attestation. A candidate/null
ledger is first produced from the canonical external scaffold request. That
request supplies every capture provenance field, timestamp, attestation,
excerpt, and gap; the tool derives identities/digests/censuses and has no
review-approval field. Its schema is
`evidence-review-scaffold-request-v1.schema.json`. A candidate/null
ledger may only drive `stage-evidence-packet-review`, which writes an
inspectable but non-importable packet. After independent review, the reviewed
ledger binds the exact planned packet digest. Normal builders reject a null or
stale digest; `--dry-run` computes the deterministic digest without writing,
and neither dry-run nor a candidate packet can satisfy the aggregate checker.

Phase 1A acceptance:

- crate-local implementation, closed schema, command dispatch, documentation,
  README, and focused positive/adversarial tests land together;
- canonicalization, path, digest, review-status, attestation, content-safety,
  optional-vault, fresh-staging, and overwrite-refusal behaviors are tested;
- no real packet, corpus evidence, generated rules, `bir-rules`, `bir-core`, or
  application behavior changes in this foundation slice; and
- the recurring library gates remain green.

### Phase 1B — materialize the 43 packets

After the foundation is stable, assemble one packet per exact five-part form
identity. Each packet records:

- form code, printed revision, official package, candidate rule-set identity,
  and source digests;
- the v1 field, validation, calculation, workflow, serialization, fixture, and
  explicit-gap inventories needed for v2 review;
- an exact record census and the v1 source material needed for later
  reconciliation, without guessing executable operators from prose;
- stable source metadata and hashes, with machine-local paths non-operative;
  and
- exact per-file and packet digests plus the required review state and
  attestations.

The first real packet is a format proof, not permission to start 42 independent
implementations. Verify and import it into a disposable fresh staging root,
review the result, then settle the aggregate manifest and drift check before
materializing the remaining packets in index order.

Aggregate output is deterministic and value-free. It contains exactly one
entry per `rules/index.json` form, preserves that index order, rejects duplicate
identities or paths, binds every packet digest, and refuses to overwrite an
existing output tree. Capture timestamps, attestations, and independent review
state come only from the explicit review ledger; the builder never invents
them from the local clock, username, or host.

### Phase 1 gate

- exactly 43 packets reconcile one-to-one and in order with
  `rules/index.json`;
- packet verification without a vault is portable and honestly reports that
  full upstream verification did not occur;
- where an approved vault is available, every upstream hash and size verifies;
- canonical packet output is byte-stable and the aggregate manifest detects
  any missing, duplicate, reordered, or changed packet;
- every reviewed import lands only in a fresh, non-`rules` staging root;
- all unresolved source dependencies stay explicit;
- the v1 census is unchanged; and
- no v2 review status, registry, GPUI, core, filing, or capability state
  changes.

## Phase 2 — generic library capability closure

Use `operator-census` and `reconciliation` to drive generic work:

```sh
rtk cargo run --locked -p bir-rules-codegen -- operator-census --json
rtk cargo run --locked -p bir-rules-codegen -- reconciliation --json
rtk cargo run --locked -p bir-rules-codegen -- \
  audit --rule-set-id <id>
rtk cargo run --locked -p bir-rules-codegen -- \
  generate --rule-set-id <id>
rtk cargo run --locked -p bir-rules-codegen -- \
  check --rule-set-id <id>
```

`--rule-set-id` is only a focus assertion. Each command first audits the full
aggregate corpus; generation and checking still build and compare the complete
tree. An unrelated corrupt snapshot therefore fails a selected command, and
selected/unselected generation is byte-identical.

`operator-census` counts only structurally present v2 operators and reports the
remaining v1 records as untranslated. It must never infer an operator from v1
prose. `reconciliation` maps exact v1 record locators either to one or more v2
entities or to a source-backed intentionally-non-runtime classification. Do
not add a one-form special case and call it library support.

1. Produce a closed support matrix from observed operators and constructs to:
   v2 schema, audit, code generation, static validation, runtime evaluation,
   and fixture coverage.
2. During implementation, unresolved and unclassified records remain visible
   blockers. Before a form passes, each v1 record must be:
   - represented by an exact v2 entity and source locator; or
   - classified intentionally non-runtime with a source reference and a reason
     from the closed enum.
3. Extend the schema, code generator, and runtime only in the smallest
   evidence-backed slice, with positive, negative, boundary, determinism, and
   adversarial tests.
4. Reject unsupported executable content at schema/audit or generation time.
   There is no fallback interpreter and no prose execution.
5. Keep form-specific evidence in packets/candidates and form-agnostic
   semantics in the crates.
6. Add `ActiveLibrary` criteria for durable library deliverables. Never remove
   a criterion after it lands.

Phase 2 is iterative with candidate onboarding: the first ordered form may
expose a missing generic construct, which must be closed here before that form
can pass. Do not skip ahead to a form that happens to use an easier subset.

## Phase 3 — ordered 43-form candidate pass

Process forms strictly in `rules/index.json` priority order. If a form is
blocked, preserve the exact gap and stop the sequence until it is resolved or
the user explicitly changes the plan. Do not use parallel worktrees to bypass
the blocked form.

For each form:

```sh
rtk cargo run --locked -p bir-rules-codegen -- \
  integrate-form --staging-root <external-form-workspace> \
  --reviewed-packet <verified-packet-dir> \
  --review-ledger <reviewed-ledger.json> \
  --rule-set-id <id> [--apply]
```

Integration recomputes the packet digest, validates the independent reviewed
ledger, and binds both artifacts to the canonical v1 form identity and staged
candidate. A HANDOFF claim is never sufficient on its own.

1. verify its portable packet and exact revision/package identity;
2. create or re-audit one strict v2 snapshot with
   `review_status: candidate`;
3. reconcile every v1 record to exact v2 source references or a source-backed
   intentionally-non-runtime classification, leaving zero unclassified and
   zero unresolved legacy records;
4. encode only evidence-backed behavior supported end to end by the generic
   library;
5. add concrete fixtures for every executable branch, calculation, ordering
   rule, scope, and failure boundary;
6. prove deterministic code generation and emit the module only under
   `#[cfg(test)]`;
7. update packet/candidate/coverage manifests atomically and verify the exact
   census delta;
8. rerun the recurring gates; and
9. confirm all production guards remain closed.

Per-form acceptance does not require every rule or calculation to be
executable. It requires exhaustive record reconciliation and fail-closed,
source-backed classification of everything that is intentionally non-runtime.
Unresolved filing-safe profile branches do not count as unresolved legacy
records and remain deferred promotion work.

### Exact sequence

| # | Form identity |
| ---: | --- |
| 1 | `1701q-v2018` |
| 2 | `1601eq-v2018` |
| 3 | `1702q-v2018c` |
| 4 | `1601fq-v2018` |
| 5 | `1603q-v2018` |
| 6 | `1600vt-v2018` |
| 7 | `1600pt-v2018` |
| 8 | `2550m-v2007` |
| 9 | `1701ms-v2024` |
| 10 | `1701-v2018` |
| 11 | `1701a-v2018` |
| 12 | `1700-v2013` |
| 13 | `1702rt-v2018c` |
| 14 | `1604c-v2018` |
| 15 | `1604e-v2018` |
| 16 | `1604f-v2018` |
| 17 | `1702mx-v2018c` |
| 18 | `1706-v2018` |
| 19 | `1606-v2018` |
| 20 | `1800-v2018` |
| 21 | `1801-v2018` |
| 22 | `2000ot-v2018` |
| 23 | `2000-v2018` |
| 24 | `1602q-v2018` |
| 25 | `1600wp-v2010` |
| 26 | `1702ex-v2018c` |
| 27 | `1707-v2021` |
| 28 | `1707a-v2021` |
| 29 | `2552-v2018` |
| 30 | `2553-v1999` |
| 31 | `2200a-v2020` |
| 32 | `2200an-v2018` |
| 33 | `2200c-v2018` |
| 34 | `2200m-v2018` |
| 35 | `2200p-v2020` |
| 36 | `2200s-v2018` |
| 37 | `2200t-v2020` |
| 38 | `0605-v2003` |
| 39 | `0619e-v2018` |
| 40 | `0619f-v2018` |
| 41 | `1601c-v2018` |
| 42 | `2550q-v2024` |
| 43 | `2551q-v2018` |

2550Q is already the one landed candidate at the rebaseline. That is a
starting fixture, not a completed-form claim and not permission to skip to
priority 43. At position 42, reverify it against the final packet contract and
generic library surface.

## Phase 4 — 43-form candidate-library baseline

Close the objective only when:

- all 43 exact identities have verified packets and indexed v2 candidate
  snapshots;
- each candidate reconciles every legacy record with zero unclassified and
  zero unresolved records; every non-runtime record has a source-backed closed
  reason;
- every emitted candidate module is test-only and the reviewed registry is
  empty;
- filing-safe branches remain unresolved and serialization remains
  non-authorizing unless a later, separately approved objective changes them;
- the aggregate packet, candidate, operator, and coverage reports reconcile;
- deterministic generation, audit, runtime tests, status, and CI pass from
  tracked inputs; and
- a completion report records the actual executable validation/calculation
  counts, intentionally-non-runtime categories, and deferred profile gaps
  without calling the forms reviewed, promoted, filing-safe, or
  application-ready.

Stop for user review at this boundary. Do not automatically proceed into
promotion or application integration.

## Deferred until after the library baseline

The following are deliberately not in the active plan:

- filing-safe parity policy and the five 2550Q defect decisions;
- changing any candidate to `reviewed`;
- populating the reviewed registry or choosing a production default;
- `bir-core` adapters, GPUI validation, focus/summary wiring, persistence, or
  form-view changes;
- executable serialization artifacts, Final Copy, encryption, queueing,
  transport, or submission;
- official-package discovery through a filing path; and
- cleanup, creation, or reorganization of auxiliary worktrees.

These remain reported as boundaries or deferred promotion work. Their presence
does not make the candidate library less complete, and candidate-library
success does not make them safe to perform.

## Later worktree maintenance — macOS only

After Phase 4 is accepted, worktree cleanup may be proposed as a separate
maintenance task. It must run from macOS, where the authoritative Git metadata
paths exist. Before removing anything:

1. enumerate each worktree and resolve its absolute path;
2. inspect its branch, HEAD, dirty state, untracked files, and unique commits;
3. preserve or hand off every unique change;
4. obtain explicit user authorization for the exact removal targets; and
5. remove only those targets—never use broad pruning as a shortcut.

Do not attempt worktree cleanup from Windows, `R:\`, or the UNC view.

## Invariants throughout

- Never weaken a schema, validator, threshold, assertion, or criterion to make
  a gate pass.
- Never silently correct official behavior in the official profile.
- Never infer a rule from another revision, a typed application model, prose
  that lacks executable semantics, or memory.
- A missing packet category, candidate branch, fixture, or digest is an error,
  not an empty success.
- Source-set rolls are atomic checked transactions.
- The packaged runtime never reads `rules/`.
- No real taxpayer data, credentials, submission discovery, or live encrypted
  database access.
- No broad `git clean`, `git reset`, checkout/revert, worktree prune, or
  uninspected deletion from `tmp/` or `test-results/`.
