# Validation rules architecture

## Decision

Keep `rules/` at the repository root as the canonical evidence corpus. Add
`crates/bir-rules` as a small, UI-agnostic runtime crate containing only
reviewed executable contracts and generated snapshots.

The extracted JSON is not moved into the crate because it includes prose,
unresolved findings, source-locator paths, extraction utilities, and research
metadata. Shipping or dynamically interpreting that material would couple the
application to machine-local evidence and could execute an ambiguous rule.

## Ownership map

| Path | Owns | Must not own |
| --- | --- | --- |
| `rules/` | Official snapshots, hashes, source references, fixtures, audits, gaps, extraction tools | Packaged runtime logic or capability decisions |
| `crates/bir-rules-codegen/` | Strict v2 schema loading, evidence/reference checks, deterministic executable Rust emission, registry generation and drift checks | GPUI, database, transport |
| `crates/bir-rules/` | Rule identity, phases/profiles, static IR interpreter and validation, explicit request-bound workflow transitions, generated registry, exact calculations, ordered reports, closed serialization primitives, and sealed contract-owned materialization traces | Form persistence, GPUI widgets, XML transport, artifact bytes, or adapter-authored serialization authority |
| `crates/bir-core/src/form_rules/` | Generic exact-identity shadow/trusted evaluator and workflow-transition dispatch boundaries, checked plaintext rendering with an independent contract walk and parse-back verifier, and checked Final Copy occurrence-proof machinery; reviewed form adapters remain to be added | Widget rendering or independently selected serialization semantics |
| `crates/bir-core/src/db/form_rule_state.rs` | Versioned raw-draft state and immutable, v16 proof-bound Final Copy persistence | Queue/capability authorization or form-specific XML semantics |
| `crates/bir-desktop/src/components/form_validation/` | Pure stale-result-safe validation and accepted-workflow-result controller; GPUI summary, focus and production form wiring remain to be added | Tax formulas, independent validation rules, or implicit filing authorization |
| `docs/validation-rules/` | Architecture, rollout plan, review decisions | Executable source of truth |

The old `crates/bir-core/src/schema.rs` and `form.rs` modules are a small
string-formula prototype. They are not the extracted-rules runtime and must not
be extended into one. Remove them after compatibility review once the typed
rule adapters have replaced their public surface.

## Dependency direction

```text
official assets and observations
              |
              v
       rules/ snapshots
              |
        reviewed compiler
              |
              v
          bir-rules
              |
              v
           bir-core
              |
              v
      bir-desktop (GPUI)
```

There is one evaluator. GPUI is a local Rust frontend, so "client-side
validation" means calling that evaluator in-process on input/change/blur--not
copying rules into JavaScript or view code. At Final Copy and Submit,
`bir-core` calls the same compiled snapshot again from persisted/typed data.

## Current implementation boundary

The executable foundation exists, but the landed corpus remains deliberately
inert:

- `bir-rules` has a typed static interpreter for normalization, coercion,
  exact integer/decimal calculations, predicates, repeated groups, explicit
  singleton/per-group calculation and rule scopes, ordered rule evaluation,
  and complete instance-aware result coverage. Group aggregates evaluate a
  typed expression per stable instance, so a singleton total can consume
  reviewed current-row derived outputs without flattening row identity;
- before inventory or evaluation, the static provider validates the complete
  specification across both profiles and every declared phase: identifiers,
  group membership and cardinality, calculation order and dependency
  availability, all field/context/derived/group references, expression and
  predicate types, and effect targets/types. Unsupported effect kinds still
  fail closed after their static content is checked;
- workflow state is not inferred from a valid or empty evaluation report.
  `CompiledRuleSet::transition_workflow` requires the exact evaluated request
  and result, current state, action and phase; it returns a result bound to
  rule-set identity, validation context, input revision and context
  fingerprint. Each transition declares its evaluated phase; the runtime never
  infers one from the action name. This lets a non-validating Edit action
  consume the still-current successful Validate result without fabricating a
  Draft Preview evaluation. Before evaluating a guard it re-runs the compiled
  evaluator and requires full result equality, including repeated groups,
  derived values, rule coverage and violations. The
  static interpreter requires exactly one matching transition and state effect
  and carries exact notifications. Empty/custom providers remain unresolved;
- every v2 document has an explicit profiled `evaluation_policy`. A reviewed
  snapshot must independently bind both branches to either `apply-all` or
  `stop-effects-after-first-blocking-issue`; no runtime default exists;
- code generation audits a reviewed snapshot and deterministically emits Rust
  statics plus a sealed `StaticCompiledRuleSet`. The packaged runtime never
  parses the evidence JSON;
- the emitter's currently promotable effect subset is `emit-issue`. It emits
  the remaining reviewed normalization, coercion, expression, calculation and
  predicate subset, including decimal `divide` only when the expression binds
  an explicit scale and rounding mode. Schema validation, code generation and
  static validation reject missing, misplaced, or out-of-range division
  policy. Regex `matches` remains rejected until an audited packaged matcher
  exists, as do `set-derived` and `normalize-field` effects until their
  ordering/output channels are safe. `set-workflow-state` remains rejected in
  ordinary rules and is supported only inside an explicit workflow transition
  result channel;
- `bir-rules::serialization` has closed targets for editable save, finalized
  save, encrypted Final Copy, submission payload, and historical-import
  compatibility, each paired with an exact variant ID. It also has explicit
  absent/blank handling, exact text/boolean/integer/decimal/date formatting,
  and deterministic raw, legacy JavaScript `escape`, and RFC 3986-unreserved
  UTF-8 percent body codecs. Seventeen focused tests cover this primitive
  layer;
- the mandatory v2 `serialization` subtree now has a closed schema,
  fail-closed cross-reference/type/ordering audit, a canonical subtree digest,
  deterministic generated borrowed Rust statics, and read-only exposure
  through the sealed `CompiledRuleSet`. It describes exact artifact identities,
  profiled branches, ordered pseudo-XML fields, metadata, reviewed literals,
  dynamic groups, value/key/occurrence projections, semantic formats,
  presence, and body codecs. Derived projections explicitly select singleton,
  current-group, or one stable group instance;
- the sealed `bir-rules` materializer now selects one exact artifact/profile/
  phase branch, re-evaluates the bound request, validates the complete plan
  even when a dynamic group has zero instances, and emits a complete ordered
  accounting trace. The trace binds sources, group paths, projections,
  formatting, codecs, omissions, contract/raw/evaluation/manifest digests, and
  the exact artifact identity. It deliberately owns neither artifact bytes nor
  filing authorization. Group-scoped derived source identity is included in
  the trace, and the evaluation and record-manifest digest domains are
  versioned at v2 so pre-scope proofs cannot be reinterpreted;
- `bir-core::form_rules::CheckedSerializationArtifact` re-resolves the exact
  packaged provider, re-evaluates the request, independently walks the selected
  generated plan, resolves every trusted semantic source, recomputes formatting
  and codecs, renders the complete plaintext, and parses every emitted byte
  back against the expected ordered records. It binds the contract, artifact,
  request, context, record manifest, plaintext length, and plaintext digest.
  There is no public constructor or deserializer, and it cannot construct a
  Final Copy, encrypted container, queue entry, or submission;
- the only landed v2 entry is a source-digest-pinned, still-unreviewed 2550Q
  candidate. Its executable surface currently covers 66 singleton fields,
  28 repeated-family descriptors, one required local-date context value, and
  27 observed validations (Save
  orders 1-3 and all alerting Validate orders 1-24), and one Validate-only
  parsing/date calculation. One hundred twenty-one concrete fixtures exercise
  that subset against the
  static interpreter, including JavaScript prefix parsing, Invalid Date
  continuation, overflow normalization, years 0-99, all three hard-coded
  future-period exception arms, exact raw-blank behavior for Items 10-12, the
  official acceptance of non-email text on Item 12, exact Item 13 radio
  selection, Item 14A's whitespace behavior, the missing Item 14 selection
  requirement, Items 19, 42, 47, and 56 JavaScript `parseFloat`
  longest-prefix, NaN, Infinity, signed-zero, whitespace and
  strict-empty-string behavior, exact message whitespace/punctuation, Item
  56's distinct exact-blank exemption, and cross-block first-error ordering.
  The silent Item 19 order-16 return is preserved as documented, effectless,
  and provably unreachable. Every alerting branch in official `validate()` is
  now represented. The successful branch is also represented as an explicit
  `edit` to `validated` transition after the exact request produces a valid
  Validate result, with the exact official success alert. The official Edit
  handler is represented as `validated` back to `edit`, consuming that same
  unchanged Validate result and emitting its exact alert. Neither transition
  can be inferred from an empty report or an action-to-phase convention. The
  source's asymmetric Edit control behavior is pinned for the GPUI state
  mapping and is not treated as a generic enable-all operation. The combined
  Final Copy/Submit source path adds two typed but non-executable landmarks:
  `validated -> submission-enrollment` is bound to the prior Validate result,
  while `submission-enrollment -> submission-attempted` declares the fresh
  Save phase that precedes editable-save staging, Final Copy staging,
  encryption, and transport. Both official branches are
  `Branch::DocumentedOnly`; both filing-safe branches are unresolved. The
  pinned connection helper always returns true, so its no-connection
  local-copy branch is unreachable. Runtime selection cannot execute either
  edge. Final Copy materialization, Upload/Submit outcomes, executable
  serialization behavior, remaining calculations, and the filing-safe profile
  remain unresolved or absent. The required serialization subtree inventories
  separate package-specific editable-save, finalized-save, and
  encrypted-final-copy identities as documented-only with no nodes, so it
  grants no materialization authority. A separately pinned value-free
  inventory binds all 160 observed plaintext occurrences, the exact
  159-occurrence encrypted prefix, artifact-specific codecs, and 28 unbounded
  families partitioned into seven `assigned-stable-id` groups. All 159 live
  serialized-control occurrences have candidate identities: 119 static
  controls plus 40 materialized repeated-family occurrences. The executable
  field surface is intentionally narrower at 66 singleton fields plus 28
  repeated-family descriptors; 44 derived/alias and nine
  workflow/credential/UI-state controls remain identity-only documented
  bindings. Generated `dateFiled` is projected from the same immutable
  `local-current-date` context snapshot used by validation; the production
  clock and timezone provider remains unresolved. The
  relation between stable-instance order and official live DOM/display order
  stays an explicit execution blocker;
- the generator emits that candidate as a `#[cfg(test)]` module so its
  fixtures compile and execute, while the generated reviewed registry remains
  empty. `bir-core` has a raw-input adapter and an inert diagnostic path for
  the candidate, but the review-controlled default designation remains
  `None`, queue submission remains unsupported, and no reviewed provider or
  production filing caller can select it. The GPUI validation controller can
  retain an exact accepted semantic workflow result and clears it on edits,
  context changes, replacement evaluations, or unavailable/incomplete state;
  it does not itself mutate controls or authorize filing. All 66 executable
  candidate singleton values have live GPUI `InputState` bindings and semantic
  capture paths: 39 ordinary controls plus 27 raw-only buffers. Amended-return
  and short-period use four independent raw-backed choices; explicit clicks
  atomically capture exact mutually exclusive pairs before updating the two
  typed booleans. Missing
  raw state never falls back to typed draft or profile defaults; an explicit
  radio click materializes the full mutually exclusive group as exact
  `true`/`false` values, text edits capture exact live text, and import restores
  exact persisted text. Each user action advances the input revision once, and
  the visible controls expose semantic focus targets. Checked XML rejects
  missing, partial, malformed, or typed-incoherent candidate authority.
  Production current-date capture and all identity-only derived/workflow
  controls still have explicit execution or capture gaps.

`bir-core` also parses BIR pseudo-XML into a globally ordered,
duplicate-preserving occurrence sequence. Each field ID receives its own
one-based occurrence number and each encoded body remains exact. The existing
`checked-final-copy-payload-v2` proof machinery uses that representation. Its
test-only manifest bridge checks contiguous occurrence numbering, global
order, exact manifest/XML bijection, semantic sources against a filing-safe
Final Copy `TrustedEvaluation`, and identity, request, context, manifest,
proof, length and XML digests. Database migration v16 can store and revalidate
that proof JSON and digest and fails closed on missing, legacy, mismatched, or
tampered proof data. The newer checked plaintext artifact is intentionally not
restored from stored parts and is not yet wired into that persistence type.

Production construction is deliberately unavailable:
`CheckedFinalCopyPayload::try_new` returns
`MissingSerializationContract`. The only constructor that turns
`FinalCopyFieldCoverage` into a proof is compiled for tests. This preserves
coverage/proof regression tests without allowing an adapter-supplied semantic
value and independently supplied encoded value to become production
authority.

The generic checked-plaintext gate now exists, but the production filing gate
remains closed. The only landed candidate declares no executable serialization
artifact, the reviewed registry is empty, and its separate diagnostic adapter
cannot become a reviewed provider. There is also no conversion from
`CheckedSerializationArtifact` to `CheckedFinalCopyPayload`, no reviewed
outer-container/encryption stage bound to the plaintext digest, and no
queue/transport reauthorization path. Phase 5 Final Copy production wiring and
queue authorization therefore remain closed.

## Runtime identities and profiles

A draft must eventually persist:

- `rule_set_id`;
- source-set digest;
- behavior profile;
- last validated phase and timestamp (diagnostic only).

Selection uses form code, printed revision, official package version, and
source-set digest. The app must never select rules by form code alone.

Each compiled rule has two independent branches:

- `OfficialCompatibility`: exact observed Offline eBIRForms behavior, including
  confirmed defects;
- `FilingSafe`: separately reviewed application behavior.

An unresolved branch cannot be silently replaced by the other branch. It stays
non-executable or advisory according to an explicit review decision.

Effect handling is also profiled evidence, not an interpreter preference.
`evaluation_policy.official` and `evaluation_policy.filing_safe` each require
their own source references and review decision before a reviewed snapshot may
be emitted.

Ordinary editing, Validate, Final Copy and Submit use `FilingSafe`.
`OfficialCompatibility` is limited to diagnostics, regression tests and
explicit historical/import compatibility; it can never authorize a trusted
filing action.

## GPUI interaction model

Each form view keeps its raw `InputState` values and exposes them through a
form-specific adapter implementing `FieldValueSource`. Repeating schedule rows
use stable row-instance IDs, not vector indexes. A rule evaluation owns the
raw-to-canonical parse, derived values, and validation report so calculations
cannot run from a fabricated zero or stale typed value.

| Event | Evaluation | UI behavior |
| --- | --- | --- |
| Keystroke/input | Cheap field-local input rules; no lossy numeric fallback | Inline syntax/length hint |
| Blur/change | Normalization, parsing, dependent field rules, recompute | Inline issue and refreshed computed values |
| Page navigation | Page and cross-field rules for fields being left | Focus first blocking issue on that page |
| Save Draft | Malformed-storage checks; semantic incompleteness may remain | Save locally and show advisory summary |
| Validate | Full ordered validation | Show shared summary and focus first blocking field |
| Final Copy | Full rules, calculation consistency, XML completeness | Fail closed; create immutable review output only on success |
| Submit | Re-load/recompute/revalidate at the trusted boundary | Queue only the exact validated ruleset/digest |

GPUI stores the latest evaluation and `ValidationReport`, not its own booleans
for business rules. A shared component maps semantic field IDs to
`FocusHandle`s, renders inline messages, and renders the ordered form summary.
Form-specific views still own layout and control construction.

For rapid input, the controller may debounce cross-field evaluation and tag
each request with a monotonically increasing generation. Stale results must be
discarded. Final Copy and Submit are always synchronous against the current
snapshot and never use a debounced result.

## Calculation and XML boundaries

- Calculations use explicit decimal/coercion/rounding semantics. Decimal
  division is executable only with expression-local scale and rounding policy.
  The official profile may emulate JavaScript behavior where evidence requires
  it.
- GPUI displays calculated values returned by the core adapter; it does not
  recompute formulas independently.
- Raw-to-typed synchronization is pure. It must not acquire the database,
  reconcile taxpayer profiles, or save a draft from an input/change callback.
- External profile/rate/election refresh is a separately versioned context
  update. Late results are ignored when their input/context revision is stale.
- XML unknown fields remain preserved.
- Repeated official keys use the ordered occurrence parser. A
  `BTreeMap<String, String>` remains only a compatibility view for forms whose
  uniqueness is proven.
- The checked plaintext artifact derives each encoded occurrence from the
  selected generated contract and independently verifies the complete
  plaintext and accounting manifest. Production Final Copy construction
  remains closed until a reviewed form contract is registered and the checked
  plaintext proof is bound through the complete envelope and queue boundary.

## Security and release boundary

Client-side validation improves feedback but is not authorization. Checked XML
export, Final Copy, queue creation, and pre-transport submission each require a
fresh blocking validation report from `bir-core`.

Print Preview must not implicitly save a draft or masquerade as Final Copy. A
permissive draft preview can remain available and visibly marked; an official
Final Copy is produced only from the frozen canonical snapshot that passed its
phase gate.

Rule-corpus completeness remains independent from form capability and release
evidence. Code generation, a green rules test, or a `complete` research
manifest must never modify a capability flag automatically.

The current empty registry, absence of a reviewed/generated form, and
production `MissingSerializationContract` result mean none of the runtime,
compiler, codec, checked-payload, or v16 persistence work authorizes a form
for Final Copy or submission.
