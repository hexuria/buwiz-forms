# bir-rules-codegen

`bir-rules-codegen` is the offline compiler boundary between the canonical
`rules/ir/v2` corpus and tracked Rust files under
`crates/bir-rules/src/generated`.

It is a standalone development tool, not a runtime dependency and not a Cargo
build script. The packaged application must never read `rules/`.

## Commands

From the repository root:

```text
cargo run --locked -p bir-rules-codegen -- audit
cargo run --locked -p bir-rules-codegen -- generate
cargo run --locked -p bir-rules-codegen -- check
```

All commands accept `--repo-root PATH`. Source and schema overrides must be
portable repository-relative paths. Generated writes are deliberately locked
to `crates/bir-rules/src/generated`; a caller cannot redirect the atomic
replacement at another repository directory. The defaults are:

```text
rules/ir/v2
rules/schema/v2
crates/bir-rules/src/generated
```

`audit` performs strict duplicate-key and closed-structure loading, safe path
resolution, index/directory bijection, identity and review-state checks,
reference coverage, explicit profiled `evaluation_policy` checks, calculation
dependency cycle checks, pinned legacy-evidence/count reconciliation, reviewed
fixture coverage, and the closed artifact/profile branches in the top-level
`serialization` contract. Serialization audit includes artifact identity,
global ordinal, repeated-key occurrence, indexed-group scope, value/formatter,
codec, presence-predicate, and source-reference checks. Executable branches
are audited even when no runtime caller currently selects their artifact.

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
