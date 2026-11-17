# bir-rules-codegen

`bir-rules-codegen` is the offline compiler boundary between the canonical
`rules/ir/v2` corpus and tracked Rust files under
`crates/bir-rules/src/generated`.

It is a standalone development tool, not a runtime dependency and not a Cargo
build script. The packaged application must never read `rules/`.

## Commands

From the repository root:

```text
rtk cargo run --locked -p bir-rules-codegen -- audit
rtk cargo run --locked -p bir-rules-codegen -- generate
rtk cargo run --locked -p bir-rules-codegen -- check
rtk cargo run --locked -p bir-rules-codegen -- audit --rule-set-id 2550q-v2024-p7.9.6.0
rtk cargo run --locked -p bir-rules-codegen -- generate --rule-set-id 2550q-v2024-p7.9.6.0
rtk cargo run --locked -p bir-rules-codegen -- check --rule-set-id 2550q-v2024-p7.9.6.0
rtk cargo run --locked -p bir-rules-codegen -- status
rtk cargo run --locked -p bir-rules-codegen -- status --boundaries-only
rtk cargo run --locked -p bir-rules-codegen -- status --require-promotion
rtk cargo run --locked -p bir-rules-codegen -- discover-evidence-vault-sources --output ../validation-rules-evidence-input/maps/source-map.json --search-root C:/external/official-assets --dry-run
rtk cargo run --locked -p bir-rules-codegen -- discover-evidence-vault-sources --output ../validation-rules-evidence-input/maps/source-map.json --search-root C:/external/official-assets
rtk cargo run --locked -p bir-rules-codegen -- verify-evidence-vault-source-map --source-map ../validation-rules-evidence-input/maps/source-map.json
rtk cargo run --locked -p bir-rules-codegen -- write-evidence-capture-metadata --output ../validation-rules-evidence-input/capture/capture.json --capture-session-id static-package-7-9-6-0 --tool-commit <full-commit> --source-map-sha256 <verifier-map-sha256> --source-verification-sha256 <verifier-result-sha256> --command-arg bir-rules-codegen --command-arg verify-evidence-vault-source-map --command-arg --source-map --command-arg ../validation-rules-evidence-input/maps/source-map.json --capture-tool-version "bir-rules-codegen 0.1.0" --operating-system windows --windows-version "<exact version>" --official-app-version "7.9.6.0" --started-at-utc <UTC> --finished-at-utc <UTC> --write
rtk cargo run --locked -p bir-rules-codegen -- acquire-evidence-vault --source-map /absolute/external/validation-rules-evidence-input/maps/source-map.json --capture-metadata /absolute/external/validation-rules-evidence-input/capture/capture.json --vault-root /absolute/external/validation-rules-evidence-vault --dry-run
rtk cargo run --locked -p bir-rules-codegen -- acquire-evidence-vault --source-map /absolute/external/validation-rules-evidence-input/maps/source-map.json --capture-metadata /absolute/external/validation-rules-evidence-input/capture/capture.json --vault-root /absolute/external/validation-rules-evidence-vault
rtk cargo run --locked -p bir-rules-codegen -- scaffold-evidence-review-ledger --input /review/scaffold-request.json --vault-catalog /external/validation-rules-vault/vault-catalog.json --output /review/candidate-ledger.json --dry-run
rtk cargo run --locked -p bir-rules-codegen -- verify-evidence --packet tmp/packet
rtk cargo run --locked -p bir-rules-codegen -- import-evidence --packet tmp/packet --staging-root tmp/import
rtk cargo run --locked -p bir-rules-codegen -- stage-form --form-id 1701q-v2018 --packet tmp/reviewed-packet --staging-root tmp/form
rtk cargo run --locked -p bir-rules-codegen -- stage-evidence-packet-review --form-id 1701q-v2018 --review-ledger /review/candidate-ledger.json --vault-catalog /review/catalog.json --output-root tmp/packet-review
rtk cargo run --locked -p bir-rules-codegen -- build-evidence-packet --form-id 1701q-v2018 --review-ledger /review/ledger.json --vault-catalog /review/catalog.json --output-root tmp/packet --dry-run
rtk cargo run --locked -p bir-rules-codegen -- build-evidence-packet-set --review-ledger /review/ledger.json --vault-catalog /review/catalog.json --output-root evidence/validation-rules/packets/v1
rtk cargo run --locked -p bir-rules-codegen -- check-evidence-packet-set --packet-root evidence/validation-rules/packets/v1
rtk cargo run --locked -p bir-rules-codegen -- integrate-form --staging-root /external/form-work --rule-set-id 1701q-v2018-p7.9.6.0
```

`--official-app-version` is the exact package identity used by the form
manifests (for this corpus, `7.9.6.0`), not a display label with an
`Offline eBIRForms` prefix. Packet staging requires exact equality.

All commands accept `--repo-root PATH`. Source and schema overrides must be
portable repository-relative paths. Generated writes are deliberately locked
to `crates/bir-rules/src/generated`; a caller cannot redirect the atomic
tree publication at another repository directory. When an existing generated
tree is replaced, its exact prior directory is preserved beside the new tree;
the tool never recursively deletes it through a mutable path. The defaults
are:

```text
rules/ir/v2
rules/schema/v2
crates/bir-rules/src/generated
```

Directory publication is no-replace on every supported host. Linux and macOS
use `renameat_with(..., NOREPLACE)`; Windows uses the narrow safe wrapper in
`bir-rules-platform`, which calls `MoveFileExW` without replacement authority.
That helper is development-tool-only and is not a dependency of `bir-rules`.

`status` always reports three criterion classes. `Boundary` directly inspects
the switches that keep production filing closed. `ActiveLibrary` covers the
43-form library objective, including v2 snapshot coverage, exact legacy-record
reconciliation, representation of all 2,007 validations and 623 calculations,
and the test-only generated candidate catalog. `DeferredPromotion` records
policy or evidence work that remains visible but is not part of that
objective. Default `status` exits successfully only when every boundary and
active-library criterion passes; it remains nonzero while the aggregate
library gates are open. `status --boundaries-only` checks only the production
switches, while `status --require-promotion` additionally requires every
deferred-promotion criterion. Default success therefore does not claim
promotion readiness.

`audit` performs strict duplicate-key and closed-structure loading, safe path
resolution, index/directory bijection, identity and review-state checks,
reference coverage, explicit profiled `evaluation_policy` checks, calculation
dependency cycle checks, pinned legacy-evidence/count reconciliation, reviewed
fixture coverage, and the closed artifact/profile branches in the top-level
`serialization` contract. Serialization audit includes artifact identity,
global ordinal, repeated-key occurrence, indexed-group scope, value/formatter,
codec, presence-predicate, and source-reference checks. Executable branches
are audited even when no runtime caller currently selects their artifact.
For `audit`, `generate`, and `check`, `--rule-set-id` is a focus assertion
after the full aggregate audit, never an input filter. Selected and unselected
generation produce the same complete tree; corruption in any peer snapshot
still fails the selected command.

`generate` writes through a same-directory staging tree and rename. The
compiler emits reviewed snapshots as deterministic Rust statics backed by the
`bir-rules` static interpreter and registers sealed providers in complete
`FormRevisionKey` order. It may also emit a digest-pinned `candidate` snapshot
as a `#[cfg(test)]` module so its concrete evaluation fixtures execute against
the same static interpreter before human review. Candidate modules, metadata,
and providers are never added to the reviewed production registry. Metadata
and canonical JSON are not accepted as
packaged runtime behavior, and the generated runtime performs no JSON parse.
Each generated provider also exposes a borrowed serialization contract with an
independent SHA-256 of the canonical JSON `serialization` subtree. The statics
describe reviewed plaintext nodes only; they do not select an artifact,
materialize bytes, infer a codec/default, or model compression, encryption,
signing, or transport framing.
The current unreviewed 2550Q candidate therefore continues to produce an empty
reviewed registry and only a `#[cfg(test)]` form module; it cannot become
runtime behavior by merely existing in the v2 index.

Both `official` and `filing_safe` `evaluation_policy` branches of a reviewed
snapshot must be source-bound, reviewed and executable. A candidate requires
at least one matching executable profile/policy branch; every non-executable
branch remains explicitly documented-only or unresolved and never falls back.
The supported evaluation emission subset covers
typed normalization/coercion, exact expressions and calculations,
repeated-group operations, logical/comparison/presence/membership predicates,
and `emit-issue`. Workflow transitions use a separate explicit, request-bound
result channel. The compiler emits typed states, action-bound transitions, an
explicit evaluated phase per transition, guards, `set-workflow-state`, and
exact notifications, and candidate fixture audit requires complete expected
transition coverage. The result carries the exact validation context; an
action name never implicitly selects or invents an evaluation phase. Generation rejects
unsupported semantics explicitly:

- regex `matches` has no audited offline packaged matcher backend;
- decimal binary `divide` is rejected until expression IR carries the reviewed
  division scale and rounding policy;
- `set-derived` and `normalize-field` are rejected because post-calculation
  mutation can leave dependent outputs inconsistent.

`set-workflow-state` remains invalid inside an ordinary evaluation rule. It is
accepted only in an explicit workflow transition branch whose output is a
`WorkflowTransitionResult`, never inferred from an empty `EvaluationResult`.

`check` audits, generates independently twice, compares the exact file set and
bytes, compares that result with the tracked output, runs
`cargo fmt --package bir-rules -- --check`, and then runs the `bir-rules`
tests. `--skip-runtime-tests` is available only for focused codegen tests: it
skips `cargo test`, but not the generated/runtime Rust formatting gate. It also
does not bypass audit, deterministic generation, drift, policy, fixture, or
unsupported-node failures.

## Deterministic JSON

The compiler uses `bir-json-c14n-v1`, a deliberately small deterministic
equivalent to RFC 8785 for this schema:

- input is parsed once with duplicate object keys rejected at every depth;
- object keys are ordered by their UTF-8 byte sequence;
- arrays retain source order;
- strings, booleans, nulls, and finite JSON numbers use `serde_json`'s stable
  minimal encoding;
- no insignificant whitespace is emitted.

The identifier is recorded in the generated manifest. A change to these rules
requires a new identifier and generator review.

## Portable evidence packets

`discover-evidence-vault-sources` creates a source map only when all 43
manifests' acquirable declarations resolve to exact hash/size-verified bytes.
It never emits a partial map, never follows symlinks/reparse points, and never
opens metadata-only, zero-size, taxpayer/save, credential, or submission-shaped
locations. Search roots are explicit, external, and may be repeated.

The capture transaction is mandatory and ordered. First run
`verify-evidence-vault-source-map` from the repository with a `/`-separated
external path beginning in `../`; retain both digests it reports and the exact
start/finish UTC timestamps. Then write capture metadata with the exact
four-argument portable command
`bir-rules-codegen verify-evidence-vault-source-map --source-map ../...json`,
both reported digests, the checkpoint commit, Windows version, and official
application version. The metadata writer is no-write unless `--write` is
explicit. Acquisition independently resolves that recorded locator to the
source map it consumes and rejects a same-bytes map at any other path.

`acquire-evidence-vault` is the only tool-owned path from explicitly mapped
machine-local source files into a fresh external content-addressed vault. It
requires a canonical source map and capture-provenance record, verifies every
declared hash and size before and during copy, rejects symlinks/reparse points,
and installs the vault atomically. Its three filesystem arguments must be
absolute, lexically normalized external paths; the portable `../...` locator is
used only by the provenance-bound verifier command recorded in capture
metadata. Its closed disposition table never reads or copies dummy save,
final-copy, taxpayer-shaped, or zero-size provenance assets; those declarations
remain metadata-only gaps. `--dry-run` performs all source verification without
creating the vault.

Fresh packet, staging, metadata, and review outputs never overwrite an existing
path. If construction fails, the incomplete fresh output is deliberately left
for explicit inspection instead of being removed by pathname; this prevents a
concurrent path substitution from turning cleanup into data loss.

`verify-evidence`, `import-evidence`, and `stage-form` are an offline evidence
transport and staging boundary. They do not generate rule sets, change review
status, write canonical `rules/`, or create runtime authority.

`verify-evidence --packet DIR [--vault DIR]` validates the closed
`bir-evidence-packet-v1` contract. The manifest and every derived payload are
exact canonical UTF-8 JSON; packet paths are relative `/` paths; directory
walks reject symlinks; and the declared sizes, SHA-256 values, packet digest,
review metadata, and four required attestations are fail-closed. The packet
inventory is bijective, so an undeclared file is an error. Derived content
shaped like taxpayer values, credentials, request/payload/transport material,
or online submission is forbidden. Original upstream bytes remain outside the
packet. A successful verification without `--vault` deliberately reports
`full_upstream_verified: false`.

`import-evidence --packet DIR --staging-root DIR` requires both the packet and
every derived file to be `reviewed`. It copies only declared derived files to a
brand-new staging root, refuses overwrite, and refuses a destination beneath
canonical `rules/`.

`stage-form --form-id ID --staging-root DIR` mirrors
`rules/forms/<form-id>/` into a brand-new staging root, retaining the corpus
layout for tools that accept a different repository root. Supplying
`--packet DIR` requires an exact reviewed, planned-source packet and creates a
complete external one-form skeleton workspace: the exact v1 mirror, v2 schemas,
a skeleton rule set/index, and deterministic `HANDOFF.json`/`HANDOFF.md`.
Every legacy record starts unresolved, serialization artifacts remain empty,
and the handoff records the value-free occurrence ledger as a blocking gap.
Neither mode overwrites or writes canonical `rules/`.

The normative contract and digest construction are documented in
`docs/validation-rules/portable-evidence-packets.md`; its reviewable JSON Schema
is `docs/validation-rules/evidence-packet-v1.schema.json`.

`scaffold-evidence-review-ledger` consumes the canonical external request
defined by
`docs/validation-rules/evidence-review-scaffold-request-v1.schema.json`. The
request supplies every capture fact and attestation explicitly. The command
recomputes all 43 v1 source digests and censuses, binds the exact vault catalog,
and always emits candidate/null review state; it has no approval input.

The deterministic Phase 1B packet-set factory is documented in
`docs/validation-rules/evidence-packet-set-factory.md`. It requires a canonical
review ledger and external content-addressed vault catalog. Candidate packets
are staged explicitly for inspection and cannot be imported or accepted by the
aggregate checker. Reviewed construction then supports a no-write
digest-planning pass with `--dry-run`, refuses overwrite, and checks the exact
`rules/index.json` order/bijection with `check-evidence-packet-set`.

`integrate-form` is currently a no-write add-only proposal checker after a
packet-backed external workspace has become a strict candidate. It preserves
every existing snapshot byte (including 2550Q), requires filing-safe policy to
remain unresolved, and runs full aggregate audit and deterministic generation
twice in an external proposal. `--apply` fails closed until a portable
non-cooperating-writer-safe directory transaction exists; the command never
writes canonical rules, generated runtime, or application files.
