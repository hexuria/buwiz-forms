# Executable validation-rules plan

> **Superseded as an execution queue (2026-07-26).** This document remains the
> architecture and phase-history record; its GPUI, Final Copy, promotion, and
> remaining-form sequencing is not the live plan. Continue from
> [execution-plan.md](execution-plan.md), which requires portable evidence
> packets and an ordered 43-form candidate library before any application or
> filing work resumes. Nothing in this historical plan authorizes a reviewed
> registry entry or a production path.

This plan introduces the extracted rules without changing any current form,
renderer, release status, capability flag, queue authorization, or submission
behavior until the applicable phase gate is satisfied.

## Current verified baseline

- The unchanged v1 corpus covers 43 forms, 9,592 fields, 2,007 validations,
  623 calculations, 1,354 negative fixtures, and 216 schemas.
- The strict v2 IR, deterministic code generation, evaluator, exact arithmetic,
  checked serialization, and stable group identity are implemented.
- Rule order is phase-local: Save and Validate may each use the official order
  numbers 1, 2, and 3, while collisions within the same or overlapping phases
  fail closed.
- The generated reviewed registry remains empty. No candidate, fixture, report,
  or generated test module is reviewed, promoted, release-ready, or authorized
  for production filing.
- Final Copy authorization, queue authorization, and transport authorization
  remain closed.
- The source-pinned 2550Q candidate identity is
  `e51819028a4b2199debac87ca024fbe05b3ccb518cccbe76a436e6f8e08d45e0`.
  It has four workflow states and four transitions; only Validate success and
  Edit are executable in candidate tests. The Final Copy and Submit landmarks
  are documented-only and filing-safe is unresolved. Serialization inventories
  separate package-specific editable-save, finalized-save, and
  encrypted-final-copy artifacts. Every official branch is documented-only,
  every filing-safe branch is unresolved, and all three have no nodes and
  cannot materialize. A value-free projection inventory binds all 160 observed
  plaintext occurrences, the 159-occurrence encrypted prefix, 28 unbounded
  families partitioned into seven groups, and artifact-specific codecs.
  All 159 live serialized-control occurrences have candidate v2 identities:
  119 static controls plus 40 materialized repeated-family occurrences.
  The executable candidate surface contains 66 singleton fields and 28
  repeated-family descriptors. Forty-four derived/alias controls and nine
  workflow/credential/UI-state controls remain identity-only documented
  bindings. Generated `dateFiled` now has a reviewed projection from the
  immutable `local-current-date` context snapshot, while its production clock
  and timezone provider remains unresolved.
- The current deterministic candidate generation is pinned at output digest
  `d1fd97c502377475c7ba4f12fdb7a02021ab48ad56ed629cefb7b2594f279e71`;
  schema digest
  `1a5371b00b430ae0869f14b8c25c687125aa46319450ac7ff630224594744aef`
  and normalized source digest
  `67acef674a481ec110c562a6ad72896500a433c74c986557f304c35af4b3ee54`.
  Its serialization-contract digest is
  `c623ab7883fed79c150c5b574ab7c6682ce2e5c79cb76ad3a3df3ee816196cae`.
  The strict generated-file check (`check --skip-runtime-tests`) passes, and
  the clean combined `bir-rules-codegen` plus `bir-rules` command passes 204
  tests across five suites. The separately executed focused 2550Q core suite
  passes 52 tests with one intentionally ignored test.
- The Windows `bir-desktop` check reaches the desktop crate without reporting
  errors in the validation, 2550Q raw-radio view, receipt, cron, or 2551Q view
  changes, then remains blocked in the pre-existing `platform/windows.rs`
  boundary by mixed `windows-core` versions and a missing `Win32_Security`
  feature. This is not a validation-rules promotion exception; the full
  desktop gate remains open.

## Phase 0 -- ownership scaffold

Status: **implemented**

- Keep the canonical corpus under `rules/`.
- Add `crates/bir-rules` to the Cargo workspace.
- Establish exact revision identity, validation phases, behavior profiles,
  ordered violations, raw field access, and a compiled-rule-set boundary.
- Re-export UI-safe report types from `bir-core`.
- Document the official-package update procedure and dependency direction.

Gate:

```powershell
rtk cargo fmt --all -- --check
rtk cargo test --locked -p bir-rules
rtk cargo check --locked -p bir-core
```

## Phase 1 -- executable rules IR v2

Status: **implemented as strict infrastructure; no reviewed ruleset has
landed**. The closed rules schemas, typed static interpreter and evaluator,
explicit profiled `evaluation_policy`, expression-local decimal division
policy, exact arithmetic, explicit singleton/per-group calculation and rule
scopes, instance-aware output/rule coverage, aggregate-over-expression
semantics, checked serialization model, and fail-closed completeness checks
exist. Stable group identity is preserved through derived outputs, rules,
violations, serialization projections, and v2 digest domains.

A distinct non-promoting `candidate` review state can pin and compile partially
executable snapshots for fixture review without making them packaged runtime
entries. The static provider validates the complete specification before
evaluation, including cross-references, profile/phase/scope dependency
availability, expression/predicate types, effect targets/types, and
serialization coverage. Candidate-local unresolved content remains explicit
and cannot enter the reviewed registry.

Create strict v2 schemas while retaining the current v1 corpus unchanged as
provenance.

Deliverables:

- `rules/schema/v2/` with closed enums and `additionalProperties: false`;
- typed predicate, expression, normalization, effect, workflow, decimal,
  rounding, coercion, field-group, evaluation-scope, derived-instance, and
  serialized-occurrence nodes;
- a typed evaluation result containing canonical inputs, derived outputs,
  ordered issues, exact group instances, input revision, and context
  fingerprint;
- a separate typed workflow-transition result bound to the exact evaluated
  request/result, current state, action, phase, transition, next state, and
  ordered notifications;
- explicit `official` and `filing_safe` branches, including independently
  reviewed effect-evaluation policy;
- `executable`, `documented_only`, and `unresolved` states;
- concrete input/output fixtures rather than prose-only negative cases;
- exact index/directory bijection and reference/dependency validation.

Gate:

- every field/group/rule/calculation/state/source reference resolves;
- calculation dependencies are acyclic and evaluation order is topological;
- both selected behavior profiles type-check independently;
- every executable expression, predicate and effect is statically
  reference- and type-valid for each phase in which it can run;
- unresolved executable content fails closed.

## Phase 2 -- deterministic compiler and registry

Status: **implemented for the audited safe subset; no reviewed form has
landed**. Audit, deterministic executable Rust emission, double-generation,
tracked drift checking, hashes, atomic output replacement and CI commands
exist. Reviewed synthetic snapshots compile to sealed
`StaticCompiledRuleSet`s and registry entries sorted by the complete revision
identity. Candidate snapshots emit deterministic modules only under
`#[cfg(test)]`; all of their evaluation fixtures execute with exact
`EvaluationResult` equality, while candidate metadata and providers are absent
from every reviewed production registry. The generated reviewed registry
remains empty; the landed 2550Q package is candidate/test-only.

Add `crates/bir-rules-codegen/` as a development tool. Do not use a Cargo build
script that reparses the full corpus whenever the desktop application builds.

Compiler output:

```text
crates/bir-rules/src/generated/
  mod.rs
  registry.rs
  manifest.json
  form_<code>_<revision>_p<package>.rs  # reviewed, or candidate under cfg(test)
```

The output manifest binds source-schema, generator, normalized-source, and
generated-file hashes. Generated modules contain Rust statics, not canonical
JSON for runtime interpretation. Root commands are:

```text
npm run rules:generate
npm run rules:check
```

`rules:check` generates twice, proves determinism, rejects tracked drift, runs
the v2 audit, and tests `bir-rules`.

The emitted evaluation subset includes typed normalization/coercion, exact
expressions and calculations, policy-bound decimal division, repeated-group
operations, comparison/presence/membership/logical predicates, and
`emit-issue` effects. Generated static specifications are fully validated for
references, types and effect targets before inventory/evaluation. Generation
fails explicitly for:

- regex `matches`, until codegen has an audited packaged matcher for the
  declared dialect;
- `set-derived` and `normalize-field`, because post-calculation mutation can
  make generated outputs internally inconsistent.

Workflow emission is a separate safe subset: typed states, action-bound
transitions with an explicit evaluated phase, guards, one exact
`set-workflow-state` effect, and exact ordered notifications.
`set-workflow-state` in an ordinary evaluation rule remains an error; a
successful evaluation is never itself a state transition. Transition dispatch
re-runs the exact compiled evaluation and requires full result equality before
a guard can consume canonical or derived values. The output carries the exact
validation context, so Edit can consume a still-current Validate result without
an invented Draft Preview evaluation.

Closed artifact/variant primitives now distinguish editable save, finalized
save, encrypted Final Copy, submission payload, and historical-import
compatibility. Exact formatter and raw/legacy-JavaScript-escape/RFC
3986-unreserved UTF-8 percent codec primitives are also covered by 17 focused
tests. The v2 schema now requires a closed serialization subtree; audit and
codegen validate it, bind its canonical digest, emit borrowed Rust statics, and
expose it read-only through each generated provider. Artifact selection and
materialization are separate exact operations with no fallback; the generic
sealed materializer and independently checked byte path are described in
Phase 5.

Gate:

- no raw machine-local evidence path appears in generated output;
- the packaged runtime does not read `rules/`;
- both `evaluation_policy` branches are source-bound, reviewed and executable;
- unsupported executable nodes fail generation instead of falling back;
- the generated registry contains only explicitly reviewed snapshots selected
  by the complete `FormRevisionKey`.

## Phase 3 -- 2550Q shadow integration

Status: **candidate/test-only integration implemented; trusted production
integration remains blocked**. The generic exact-identity shadow/trusted
dispatcher and the raw 2550Q adapter exist. Package `p7.9.6.0` is a
non-promoting candidate with 66 raw singleton fields, 28 raw repeated-group
fields, one required local-date context value, three official Save rules,
all twenty-four alerting official
Validate rules, one Validate-only parsing/date calculation, and 121
source-pinned fixtures.
All 66 candidate singleton identities have closed core/GPUI raw bindings.
Amended-return Yes/No and short-period Yes/No are four independent raw-backed
controls: explicit clicks capture exact mutually exclusive pairs before
updating the two typed booleans, and imports restore exact raw values without
typed fallback.
profile-derived TIN, branch, RDO, name, address, ZIP, contact, and email values
remain blank at the raw-authority boundary until restored from reviewed import
evidence or explicitly edited.
Its source-set identity is
`e51819028a4b2199debac87ca024fbe05b3ccb518cccbe76a436e6f8e08d45e0`.
The Save slice covers an empty `TIN + branch`, RDO code `000`, and an empty
taxpayer name. The Validate slice first covers the required year-type radio
choice at official order 1 and quarter radio choice at official order 2,
reproduces the JavaScript future-period check and hard-coded exception at
official order 3, then reuses the identity bindings for Items 7-9 at orders
4-6. Orders 7-10 reproduce the raw empty-string checks for address, ZIP,
contact number, and email, including whitespace acceptance and the official
acceptance of arbitrary non-email text on Item 12. Orders 11-12 reproduce the
four-radio Item 13 classification requirement and the Item 14 rule that checks
only Treaty Yes plus an exact blank Item 14A; fixtures preserve its
whitespace acceptance and its failure to require either treaty radio. The
slice preserves exact official wording and first-error behavior, proves that
RDO `000` passes Validate even though it fails Save, and fixtures JavaScript
prefix parsing, Invalid Date continuation, overflow normalization, numeric
years 0-99, all three exception arms, and ordinary future rejection.

Pilot 2550Q because the existing core model and extracted package target the
same April 2024 printed revision.

Validate orders 13-15 now reproduce Item 19's exact raw-string and JavaScript
`parseFloat` behavior, including longest-prefix parsing, NaN, Infinity,
signed zero, underflow, overflow, whitespace-only descriptions, and exact
alert wording. The silent Item 19 order-16 return is documented but excluded
from execution because the earlier order-14 predicate always returns first
and the branch has no observable effect.

Validate orders 16-18 reproduce Item 42's separate exact raw-string and
JavaScript `parseFloat` block. The candidate preserves the leading spaces in
orders 16 and 18, the trailing space in order 17, missing terminal periods,
longest-prefix parsing, signed zero, Infinity, whitespace-only descriptions,
and first-error precedence after Item 19. The official disabled
`otherSpecify42` control's modal/additional-row population workflow remains
unresolved; only its exact validation read is modeled.

Validate orders 19-21 reproduce Item 47's exact raw-string and JavaScript
`parseFloat` block, including longest-prefix parsing, signed zero, Infinity,
the leading spaces in orders 19 and 21, the official `is required field`
grammar at order 20, and first-error precedence after Item 42. The official
disabled `otherSpecify47` control's additional-row and Item 47B 12-percent
calculation workflow remains unresolved; only its exact validation read is
modeled.

Validate orders 22-24 reproduce Item 56's distinct dual raw/parsed-value
logic. Exact blank amount plus blank description passes, whitespace is
nonblank and fails as NaN, a nonblank description plus exact blank amount
fails order 24, and longest-prefix, signed-zero, Infinity, and first-error
behavior remain source exact. The official disabled `otherSpecify56` control's
additional-row workflow remains unresolved.

This candidate does not represent a complete 2550Q ruleset. Its
`filing_safe` behavior, complete workflow, serialization artifacts, and
remaining calculations remain unresolved. All alerting Validate branches are
now executable in the test-only candidate. Its successful official branch is
an explicit `validate-success` transition from `edit` to `validated`, permitted
only for the exact valid Validate request/result and carrying the exact
`Validation successful. Click on Edit if you wish to modify your entries.`
alert. An empty violation report alone cannot activate it. The official
`edit-after-validation` transition consumes that same unchanged Validate
request/result, returns from `validated` to `edit`, and emits the exact
`You can now modify your entries.` alert. The transition does not invent a
Draft Preview evaluation, and the source's asymmetric control changes remain a
separate GPUI state-mapping obligation rather than a generic enable-all
assumption.

The combined Final Copy/Submit call graph is now pinned through the loaded
JavaScript/VBScript and external-helper identities. Two additional candidate
landmarks record `validated -> submission-enrollment` on action `final-copy`
with the prior Validate phase, and
`submission-enrollment -> submission-attempted` on action `submit` with a
fresh Save phase. The pinned connectivity helper always returns true, so its
source-present local no-connection branch is unreachable. Both official
transitions are `documented_only`, both filing-safe branches are unresolved,
and runtime selection rejects them. A package-specific encrypted-final-copy
identity is now inventoried with a `documented_only` official branch, an
`unresolved` filing-safe branch, and no nodes. Final Copy materialization,
Upload/Submit outcomes, encryption,
transport, and filing-safe transitions therefore remain unavailable. The
generated reviewed
registry is empty, so there is no trusted production evaluator selection or
promotable mismatch report. The raw adapter, checked XML path, and load-time
refusal for unchecked 2550Q XML are implemented; the unchecked public 2550Q XML
path has been removed.

Implemented foundation:

```text
crates/bir-core/src/form_rules/
  mod.rs
  registry.rs
  form_2550q.rs
```

The candidate defines 66 raw singleton values and one required
`local-current-date` context value captured once per evaluation. All 66
candidate singleton values have live GPUI `InputState` bindings and semantic
capture paths. Twenty-seven are raw-only buffers; the remaining 39 reuse
ordinary live controls. The checked XML boundary continues to require its
independently reviewed 26-key raw-authority subset.
Choice inputs
use atomic exact `true`/`false`
group updates. Raw-only text bindings start missing, restore only persisted raw
state, never format typed or profile state into authority, and advance the
input revision once per explicit edit.

Missing raw state renders no authoritative selection/value, and checked XML
rejects missing, partial, malformed, or typed-incoherent candidate authority
instead of synthesizing the typed default. Item 4 `RtnPeriodToNo4` and Item 2
now have live raw controls; exact XML import also seeds the four Item 10-12 raw
texts without using profile fallback. A production-supplied current-date
context remains a capture gap. The adapter preserves the wider reviewed editor
boundary: 56 v1 singleton controls, seven repeated groups with 28 member
descriptors and stable row identities, plus 20 named local-print buffers that
remain explicit capture exclusions. It does not supply reviewed production
context, complete serialization artifacts, or production-authorized
calculations.

Split raw synchronization from I/O before promotion. In particular, input
callbacks must not reconcile profiles, lock the database, or save drafts.

Shadow reports record differences by:

- rule ID, order, field and exact message;
- calculation ID, inputs, output and rounding;
- official versus filing-safe profile;
- XML key/count/default/occurrence coverage.

No shadow mismatch changes UI behavior or capability state.

Gate:

- exact revision/package/source identity;
- complete reviewed field binding;
- positive, negative and calculation-boundary fixtures;
- lossless XML round-trip and unknown-field policy;
- old handwritten and selected compiled behavior agree, or every difference
  has an approved decision.

## Phase 4 -- GPUI client-side validation

Status: **Stage 1 fail-closed seam implemented; trusted report integration
remains blocked**. A pure controller rejects stale results by exact
ruleset/phase/profile/input/context identity, and Print Preview no longer
performs its former implicit Save. The 2550Q seam requires a separately
reviewed, explicitly designated full identity for the exact April
2024/package 7.9.6.0 registration; adding a reviewed provider alone cannot
activate it. An unavailable or incomplete ruleset remains unavailable, with no
candidate or trusted fallback. Raw edits advance the input revision once,
duplicate semantic focus targets fail closed, and focus selection never skips
past the first blocking issue. Validate performs no persistence, while Save
remains independent and permissive. An actual trusted validation report and
live first-error focus remain blocked by the empty reviewed registry.
Summary rendering, normalized control events, and unified actions remain
outstanding.

Add:

```text
crates/bir-desktop/src/components/form_validation/
  mod.rs
  state.rs
  summary.rs
  field_focus.rs
```

Update `FormViewTrait` with explicit Validate and Final Copy actions. Each form
view provides:

- a raw field-value adapter;
- semantic-field-to-`FocusHandle` mapping;
- one `ValidationReport` state;
- stable IDs for repeating schedule-row instances;
- event calls for input, blur/change, navigation, save, validate, and final
  copy.

First migrate 2550Q. Replace duplicated error-summary rendering only after its
snapshot passes Phase 3. Do not turn invalid numeric text into `0` before input
rules see it.

Normalize `currency_input.rs`, `date_input.rs`, `tin_input.rs`, and custom
select controls around raw Change/Blur/Focus/Enter events. Move all toolbar and
keyboard actions through one `FormAction::{Save, Validate, FinalCopy, Submit}`
pipeline. Remove the current Print Preview behavior that implicitly saves a
draft first.

Gate:

- identical results for GPUI Validate and core Validate against the same raw
  snapshot;
- first-error focus follows official order;
- advisory Save behavior remains permissive;
- stale debounced reports cannot replace a newer result;
- no formula or filing rule is duplicated in a view;
- raw synchronization performs no database or network I/O.

## Phase 5 -- trusted Final Copy and Submit enforcement

Status: **partial fail-closed foundation only**. An opaque filing-safe
`TrustedEvaluation`, a sealed generated-contract materializer, an independently
verified `CheckedSerializationArtifact`, the ordered duplicate-preserving
pseudo-XML occurrence parser, checked-payload v2 proof machinery, and v17
draft/finalization persistence exist. V17 pins the complete
`FormRevisionKey`—form code, revision, official package, rule-set ID, and
source digest—plus behavior profile in drafts, Final Copies, and migration
audits. The raw 2550Q adapter can use the checked
XML boundary, unchecked public 2550Q XML generation has been removed, and load
refuses unchecked XML. A read-only active-Final-Copy submission preflight now
reconstructs a Submit/FilingSafe request solely from persisted raw inputs,
context, storage revision, and full rule identity; resolves only the generated
reviewed registry; requires one unambiguous `SubmissionPayload` artifact; and
returns an opaque, non-cloneable, non-serializable token with no public payload
bytes. With the empty reviewed registry it deterministically fails before
materialization and performs no mutation. There is still no reviewed form
caller, production Final Copy bridge, queue admission, encryption, claim, or
transport wiring.

`FormRuleEvaluator::materialize_checked` re-resolves the exact registered
provider and re-evaluates the trusted request. The rules materializer owns
artifact/profile/phase selection, source projection, group expansion,
formatting, codecs, omissions, and a complete digest-bound accounting trace.
`bir-core` then independently selects and walks the generated plan, resolves
the trusted values again, recomputes formatting and codecs, renders the
plaintext, parses every emitted byte back, and binds the contract, artifact,
request, context, record manifest, byte length, and plaintext digest. The
opaque result has no public constructor or deserializer and cannot be converted
to a Final Copy, queue item, encrypted container, or submission payload. Its
plaintext bytes are crate-internal so callers cannot pair the proof directly
with raw transport.

Adversarial proof tests now mutate the complete revision identity, every bound
digest, trace binding, and occurrence sequence independently. Omitting an
earlier duplicate occurrence cannot advance a later plaintext occurrence, and
no public plaintext accessor or proof constructor was added.

The older test-only checked-payload path proves a manifest against exact
ordered occurrences and the evaluation's canonical inputs, derived outputs,
reviewed constants/defaults, identity, phase/profile, revision, context, and
bound hashes. V17 stores and revalidates that proof JSON/digest and fails
closed for missing legacy or tampered proof data; it does not restore or
authorize the newer checked plaintext artifact.

Production construction is intentionally blocked:
`CheckedFinalCopyPayload::try_new` always returns
`MissingSerializationContract`. The manifest coverage constructor remains
`#[cfg(test)]`; an adapter cannot use it to promote independently supplied
semantic and encoded values.

Generic `save_form_draft_v2` persistence now rejects `Queued` for every form,
including transport-capable forms, so only dedicated queue APIs can create
that state. The raw FTP upload function is crate-internal; existing legacy
workers still require a future sealed claimed-submission token before this
boundary is complete.

Existing handwritten 2551Q transport persistence is independently hardened
without granting rules-engine authorization. Generic and imported writes
reject 2551Q; dedicated exact-JSON CAS transitions own
Draft→Queued→Submitted→Confirmed→Paid, cancellation, and receipt attachment. A
durable claim binds both token and timestamp and has no lease; post-claim
retry/exhaustion remains an unresolved network outcome rather than
resubmitting. Orphaned token/timestamp pairs fail closed for both 2551Q and
1601C. Malformed retry timestamps stop before XML, encryption, claim, or
network I/O. Receipt confirmation transactionally binds the persisted receipt
ID, audited `2551Qv2018`/`2551Q` aliases, TIN, period, literal `.xml` filename,
the exact stripped submitted filename, and a non-predating timestamp. UI and
email notifications report confirmation only after the authoritative
Submitted→Confirmed transition persists.

Public persisted rule-state Debug output is redacted to identifiers, digests,
and byte lengths. It omits raw/editor/canonical/derived values, reports,
evaluation traces, XML, context values, and checked plaintext proofs.

Closed artifact identities, exact semantic formatters, and raw/legacy
JavaScript-escape/UTF-8-percent codec primitives are now wired through the v2
serialization schema, fail-closed audit, deterministic codegen, canonical
contract digest, static IR, generated-provider exposure, sealed materializer,
and independently checked plaintext proof. The 2550Q candidate now inventories
three node-less `documented_only` artifact identities and a value-free binding
plan for all 160 observed occurrences and seven dynamic groups. It authorizes
no artifact and is absent from the empty reviewed registry. Still missing are
53 identity-complete but documented-only derived/workflow value projections,
the production clock/timezone provider for `local-current-date`, the reviewed
relation between stable group order and official live DOM/display order, an
executable reviewed form contract, conversion into
`CheckedFinalCopyPayload`, outer-container binding, persistence/reconstruction
policy for the new proof, and queue/transport reauthorization. Final Copy,
queue, and transport authorization remain closed.

- Add a distinct Final Copy state transition; do not alias it to Submit.
- Recompute and validate immediately before checked XML generation.
- Store `rule_set_id`, source digest, and profile with the draft/final-copy
  record.
- Re-load and revalidate within the atomic queue transition.
- Revalidate again before transport if the queued payload can change.

Gate:

- missing/unregistered ruleset, digest mismatch, unresolved blocking rule,
  calculation mismatch, missing generated artifact contract, formatter/codec
  mismatch, unaccounted envelope bytes, materialization mismatch, or ordered
  occurrence coverage mismatch prevents Final Copy/queueing;
- existing capability and release-evidence checks remain required;
- crash/retry and queue-claim tests demonstrate the validated payload is the
  payload transported.

## Phase 6 -- official update and historical compatibility

Status: **partial**. The additive update procedure, v17 exact-identity
persistence schema, transactional legacy migration, explicit reviewed repair
for pre-v17 projected identities, and dedicated v1/v2 CI jobs exist. Projected
legacy rows fail closed until a reviewer supplies the complete identity; repair
does not infer from a registry, preserves the raw snapshot hash, invalidates
the old active Final Copy, increments storage revision, and appends the full
audit tuple. No executable historical snapshot, reviewed default selector,
reopen-time resolution, or migration-diff UI exists.

- Follow `rules/UPDATING.md` for every official package release.
- Add snapshots rather than overwriting them.
- Keep all rule sets referenced by reopenable drafts.
- Select a default snapshot for new drafts in a reviewed registry.
- Offer explicit draft migration when the applicable form revision changes;
  show validation differences before accepting the migration.
- Run old and new official-compatibility fixtures on every update.
- Add a dedicated CI rules job: validate v1 on Windows while the official
  extraction workflow remains PowerShell-based, then run cross-platform v2
  codegen/determinism and `bir-rules` tests.

Gate:

- old drafts reopen under their stored rule-set identity;
- new drafts select the reviewed current identity;
- filing-safe changes have independent legal/domain evidence;
- release notes identify behavior changes by stable rule/calculation ID.

## Phase 7 -- remaining forms

Status: **not started**. The runtime and emitter can execute the supported
subset, but no landed v2 snapshot is reviewed or registered. The 2550Q
candidate remains test-only and cannot serve as a rollout precedent until its
unresolved filing-safe profile, workflow, remaining calculations, and
serialization artifacts are completed and reviewed.

Roll out in evidence/risk order, not by corpus `complete` status alone.

For each form:

1. Confirm exact core/rules revision and package identity.
2. Bind every field and serialized occurrence.
3. Bind and fixture every executable calculation.
4. Run shadow validation.
5. Review every official-versus-filing-safe difference.
6. Migrate GPUI diagnostics.
7. Enable Final Copy and Submit enforcement only after all prior gates.

Forms whose core model revision differs from the extracted snapshot require a
new core model or a matching rules snapshot before integration.

## Completion definition

The migration is complete only when:

- one versioned corpus can regenerate every packaged ruleset deterministically;
- GPUI and trusted filing boundaries consume the same evaluator;
- old drafts retain reproducible official compatibility;
- new official releases can be added without rewriting history;
- unresolved research cannot become executable by omission;
- no rules change automatically promotes a form or weakens an existing release
  gate.
