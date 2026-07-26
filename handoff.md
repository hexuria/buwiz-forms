# Validation rules integration handoff

> **Historical handoff — do not use its “Exact next task” as the live queue.**
> The 53-projection classification requested below has since landed. The
> 2026-07-26 rebaseline recorded local HEAD and
> `origin/codex/print-preview-parity` at
> `de828fd05ce27afa5c71ffd88c7a8bb2b3f9a8a5`. The active objective is now the
> portable, ordered 43-form candidate library in
> `docs/validation-rules/execution-plan.md`; GPUI, filing, promotion, and new
> worktrees remain frozen until that baseline exists. The rest of this file is
> preserved as the prior session’s evidence and rationale.

Prepared on 2026-07-25 for continuation by Claude Opus 5.

This is the repository-level handoff for the validation-rules objective. The
per-form `rules/forms/*/HANDOFF.md` files are earlier research handoffs and do
not replace this document.

## Resume contract

Before changing anything:

1. Read the repository-root `AGENTS.md` completely.
2. Read `AGENT.md` before changing GPUI/native UI code.
3. Read this file, then:
   - `docs/validation-rules/implementation-plan.md`
   - `docs/validation-rules/architecture.md`
   - `docs/validation-rules/2550q-adapter-map.md`
   - `rules/forms/2550q-v2024/audit.md`
   - `rules/forms/2550q-v2024/gaps.md`
   - `rules/UPDATING.md`
4. Confirm branch `codex/print-preview-parity`.
5. Inspect the dirty worktree without cleaning, resetting, reverting, staging,
   committing, or pruning anything.
6. Prefix repository commands with `rtk`, as required by `AGENTS.md`.

Do not work in the sibling `bir/` checkout. The authoritative checkout is:

- macOS:
  `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity`
- Windows view of the same external-drive checkout:
  `\\mac\goldcoders\reverse-engineer-ebir-forms\bir-print-parity`
  (mounted as `R:\` during the last session)

The current Git HEAD underneath the uncommitted work is
`a337b85f88d5db2f949e634b2f6b70c8a98152a3`. The working branch and all
uncommitted files are the actual continuation state.

## Critical shared-drive warning

The Windows and macOS paths refer to the same files on the external drive.
Never run Codex/Claude builds, generators, formatters, or Git mutations against
this worktree from both operating systems at the same time. Before continuing
on macOS:

- stop Windows-side Cargo, Node, PowerShell, and Codex tasks using this repo;
- confirm no generator or audit is still writing;
- do not create a second checkout over the existing directory;
- do not run `git clean`, `git reset --hard`, broad checkout/revert commands,
  or worktree pruning.

The worktree is intentionally very dirty and contains changes outside the
validation-rules effort. Treat every pre-existing modification or untracked
file as user-owned. In particular, do not touch unrelated print renderers,
calibration artifacts, `tmp/`, or `test-results/`.

Much of the new rules implementation is still untracked as a whole, including
`rules/`, `crates/bir-rules/`, `crates/bir-rules-codegen/`, and parts of
`crates/bir-core/src/form_rules/`. Consequently, ordinary `git diff` does not
show all existing work. Use direct inspection plus `git status --short`.

No commit or push was made, and none is authorized by this handoff.

## Original objective

The original goal was to extract, organize, verify, and audit the official
Offline eBIRForms validation rules for all 43 prioritized forms, preserving:

- exact form revision and package identity;
- every official XML/DOM key and unknown field;
- static and runtime validation behavior;
- calculations and dependency order;
- Save, Validate, Final Copy, and Submit differences;
- exact messages and first-error ordering;
- official bugs separately from recommended application behavior;
- positive, negative, calculation, serialization, and workflow evidence;
- historical compatibility when official packages change.

The 43-form v1 evidence corpus is now complete and audited. The user then asked
how to maintain those rules in the application and use the same engine for
GPUI client-side validation and trusted filing boundaries. That led to the v2
IR, deterministic compiler, runtime crate, core adapters, and GPUI seam
described below.

Do not confuse these two meanings of “complete”:

- v1 research `complete` means the evidence inventory is complete or its gaps
  are explicit;
- it never means a form is executable, filing-safe, release-ready, or
  authorized for Final Copy or submission.

The original priority order remains recorded in `FORM_BUILD_PRIORITY.md` and
`rules/index.json`. All 43 have v1 entries. Only 2550Q currently has a v2
candidate, and it is test-only.

## Verified corpus baseline

The last full strict audit reported:

| Measure | Value |
| --- | ---: |
| Forms | 43 |
| Legacy/v1 JSON files | 520 |
| Total JSON files | 659 |
| v2 JSON files | 139 |
| Fields | 9,592 |
| Validations | 2,007 |
| Calculations | 623 |
| Negative fixtures | 1,354 |
| Schema documents | 216 |
| Structural audit | pass |
| JSON Schema validation | pass |

The canonical evidence remains under `rules/`; it must not be moved into or
loaded dynamically by the packaged app.

## Implemented architecture

The dependency direction is:

```text
official assets and observations
              |
              v
       rules/ evidence snapshots
              |
              v
    bir-rules-codegen audit/compiler
              |
              v
        bir-rules static runtime
              |
              v
       bir-core trusted boundary
              |
              v
         bir-desktop (GPUI)
```

Ownership:

| Location | Responsibility |
| --- | --- |
| `rules/` | Revision-specific evidence, v1 corpus, strict v2 input, fixtures, audits, gaps, and extraction/update tools |
| `rules/schema/v2/` | Closed v2 JSON Schemas |
| `rules/ir/v2/` | Audited rule-set index, candidate/reviewed snapshots, and concrete evaluation fixtures |
| `crates/bir-rules-codegen/` | Cross-reference/schema audit, source hashing, deterministic Rust generation, and drift checks |
| `crates/bir-rules/` | UI-agnostic typed evaluator, exact arithmetic, ordered reports, workflow transitions, serialization primitives, and generated registry |
| `crates/bir-core/src/form_rules/` | Exact-identity shadow/trusted dispatch, form adapters, checked serialization, Final Copy proof, and submission-preflight boundaries |
| `crates/bir-desktop/src/components/form_validation/` | GPUI stale-result-safe report/workflow controller; no independent tax formulas |
| `docs/validation-rules/` | Architecture and rollout decisions; not executable authority |

There is one rules engine. GPUI “client-side validation” means calling the
Rust evaluator in-process on raw input/change/blur/action events. It does not
mean copying rules into the view or JavaScript. Final Copy and Submit must
reconstruct and rerun the same exact revisioned rules through `bir-core`.

The strict infrastructure already includes:

- exact `FormRevisionKey` identity;
- official-compatibility and filing-safe profiles;
- phase-local rule ordering;
- raw/canonical values and stable repeated-group identities;
- exact integer/decimal calculation and explicit rounding/division policy;
- deterministic v2 audit/code generation;
- candidate modules under `#[cfg(test)]`;
- an empty reviewed runtime registry;
- checked serialization primitives and independent core verification;
- versioned draft/final-copy rule-state persistence;
- a fail-closed submission preflight that cannot pass with the empty registry;
- GPUI stale-result protection and accepted-workflow-result invalidation.

## Current 2550Q v2 candidate

Exact identity:

| Property | Value |
| --- | --- |
| Rule-set ID | `2550q-v2024-p7.9.6.0` |
| Form revision | `2024-04-01` |
| Official package | `7.9.6.0` |
| Review status | `candidate` |
| Official profile | executable candidate subset |
| Filing-safe profile | unresolved |
| Source-set identity | `e51819028a4b2199debac87ca024fbe05b3ccb518cccbe76a436e6f8e08d45e0` |
| Normalized audit digest | `67acef674a481ec110c562a6ad72896500a433c74c986557f304c35af4b3ee54` |
| Schema digest | `1a5371b00b430ae0869f14b8c25c687125aa46319450ac7ff630224594744aef` |
| Serialization-contract digest | `c623ab7883fed79c150c5b574ab7c6682ce2e5c79cb76ad3a3df3ee816196cae` |
| Generated-output digest | `d1fd97c502377475c7ba4f12fdb7a02021ab48ad56ed629cefb7b2594f279e71` |

Candidate coverage:

- 66 executable singleton field identities;
- 28 repeated-family descriptors partitioned into seven groups;
- 121 source-pinned evaluation fixtures;
- three official Save rules;
- all 24 alerting official Validate rules;
- one executable Validate parsing/current-date calculation;
- explicit Validate-success and Edit-after-validation transitions;
- two documented-only Final Copy/Submit workflow landmarks;
- three documented-only, node-less serialization artifacts:
  editable save, finalized save, and encrypted Final Copy.

Serialization evidence:

- 160 plaintext pseudo-div occurrences;
- the encrypted Final Copy pseudo-div sequence is the exact first 159;
- `dateFiled` is the plaintext-only pseudo-div suffix and is standalone
  metadata after the marker in encrypted staging;
- 119 static live-control occurrences plus 40 observed repeated-family
  occurrences have exact candidate identities;
- all 160 observed occurrences now have a candidate value-source identity;
- artifacts still have no executable nodes and cannot materialize.

The remaining 53 value projections are:

- 44 derived/alias controls;
- nine workflow, credential, metadata, or UI-state controls:
  - `driveSelectTPExport`
  - `ebirOnlineConfirmUsername`
  - `ebirOnlineSecret`
  - `ebirOnlineUsername`
  - `frm2550qv2024:txtCurrentPage`
  - `frm2550qv2024:txtMaxPage`
  - `txtEmail`
  - `txtEnroll`
  - `txtFinalFlag`

These 53 have stable candidate identities but remain deliberately
`documented_only`; they are not executable value projections.

## Last completed slices

### Exact amended/short-period radio authority

The four official radio controls are now independent raw-backed values:

- `frm2550qv2024:amendedReturnYesNo5`
- `frm2550qv2024:amendedReturnNo5`
- `frm2550qv2024:OptShortPrd1`
- `frm2550qv2024:OptShortPrd2`

Explicit GPUI clicks atomically capture exact mutually exclusive `true`/`false`
pairs before updating the two typed booleans. XML import restores the exact
four values. There is no typed fallback.

All 66 singleton identities now have core/GPUI bindings:

- 27 raw-only buffers;
- 39 ordinary live controls.

Profile-backed raw authority remains deliberately absent until restored from
reviewed import evidence or explicitly edited. Do not synthesize TIN, branch,
RDO, name, address, ZIP, contact, or email raw values from typed profile state.

Primary files:

- `crates/bir-core/src/form_rules/form_2550q.rs`
- `crates/bir-core/src/forms/form_2550q.rs`
- `crates/bir-core/src/forms/form_2550q_xml.rs`
- `crates/bir-desktop/src/views/form_2550q_view.rs`
- `rules/tools/update-2550q-v2-static-projections.ps1`
- `rules/forms/2550q-v2024/v2-candidate-static-surface-projection-review.md`

### Generated `dateFiled` projection

`dateFiled` now projects from the already-required immutable
`local-current-date` evaluation context.

Important semantics:

- it is generated metadata, not an editable candidate field;
- it never falls back to `Form2550QDraft.date_filed`;
- it never reads the system clock implicitly inside serialization;
- validation and future serialization must reuse the same context snapshot and
  fingerprint, preventing a second clock read across local midnight;
- absence, duplication, or a non-date context value fails closed;
- no production clock, timezone, daylight-boundary, or custody provider has
  been approved.

This closes the last observed occurrence value-source identity without making
an artifact executable.

Primary files:

- `rules/forms/2550q-v2024/v2-candidate-date-filed-context-projection-review.md`
- `rules/forms/2550q-v2024/fixtures/serialization-binding-inventory-v796.json`
- `rules/tools/build-2550q-serialization-bindings.ps1`
- `rules/ir/v2/2550q-v2024-p7.9.6.0/rule-set.json`
- `crates/bir-rules-codegen/src/generate.rs`

The reproducible inventory now asserts
`candidate_v2_bound_occurrence_count = 160` and maps `dateFiled` to
`local-current-date`.

## Production boundaries that must remain closed

Do not “make tests pass” by changing any of these:

- generated reviewed registry: it must remain empty;
- `review_status`: keep 2550Q as `candidate`;
- filing-safe profile: keep unresolved;
- official serialization artifacts: keep documented-only and node-less until
  the complete contract is genuinely reviewed;
- Final Copy, queue, transport, and online submission authorization;
- migration capability flags, release evidence, or renderer status;
- `QUEUE_SUBMISSION_SUPPORTED` for 2550Q;
- trusted provider/default selection;
- submission preflight behavior;
- current 1% print threshold or any print migration gate.

Never use the official online submission path for discovery. Never use real
taxpayer data. Never touch the live encrypted database named in `AGENTS.md`.

Official behavior and filing-safe behavior are separate. Preserve known
official defects in the official profile and make any safer behavior an
independently evidenced filing-safe decision. Never silently “fix” the
official branch.

## Verification completed at handoff

The following evidence was green after the radio and `dateFiled` slices:

- `rtk cargo test --locked -p bir-core form_2550q`
  - 52 passed
  - one intentionally ignored
- strict generated-file check
  - four generated files
  - output digest `d1fd97c502377475c7ba4f12fdb7a02021ab48ad56ed629cefb7b2594f279e71`
- `bir-rules`
  - 102/102 passed
- `bir-rules-codegen`
  - all 99 tests effectively passed
  - on Windows, one full run reported 98/99 because the candidate probe had
    already compiled and executed but Windows refused to delete its temporary
    directory (`Access is denied`, OS error 5)
  - the isolated test then passed; this is a Windows cleanup race, not a
    semantic candidate failure
- combined documented total
  - 204 tests across five suites
- full `rules/validate.ps1 -RequireJsonSchema`
  - 43 forms
  - 659 total JSON files
  - structural audit passed
  - JSON Schema validation passed
  - stderr empty
- `git diff --check`
  - passed

The Windows `bir-desktop` test reached the desktop crate and reported no
2550Q-view error, but the crate-wide build is blocked by pre-existing Windows
platform issues:

- missing `Win32_Security` for `CreateMutexW`;
- mixed `windows-core`/`windows-strings` versions.

Do not describe the GPUI test itself as passed. The accurate statement is that
the build reached only the pre-existing platform boundary and emitted no
`form_2550q_view.rs` diagnostic.

## macOS implications

The Windows-only temporary toolchain under
`C:\Users\uriah\.codex\tmp\bir-rules-toolchain` is not part of the repository
and must not be reproduced on macOS. It existed only because the Windows
Codex host was ARM64, Git vanished from `PATH` after restart, and x86_64
GNU/LLVM/OpenSSL builds needed an isolated workaround.

On macOS:

1. Use the normal repository toolchain and lockfiles.
2. Confirm `rtk`, Rust/Cargo, Node/npm, and required native dependencies are
   available.
3. If `rtk` is missing, fix the environment before running repository
   commands; do not silently bypass the repository instruction.
4. Run normal macOS Cargo commands. Windows `windows-core` errors should be
   cfg-excluded; a macOS failure is a separate result and must be reported
   honestly.
5. PowerShell audit commands require PowerShell 7 (`pwsh`).
6. Do not use
   `rules/tools/run-full-audit-background.ps1` on macOS: it hard-codes
   `powershell.exe` and `C:\Users\...\Temp`.
7. Run the audit directly instead:

   ```sh
   rtk pwsh -NoProfile -File rules/validate.ps1 -RequireJsonSchema
   ```

   If the v1 audit proves Windows-dependent on the current Mac environment,
   do not alter evidence or weaken the validator. Record that environmental
   limitation and run the v2 cross-platform gates on Mac; rerun the v1 audit
   from Windows when available.
8. Preserve UTF-8 bytes and existing line endings in evidence files. Source
   hashes are byte-sensitive. Do not run broad line-ending normalization.

Suggested macOS orientation:

```sh
cd /Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity
rtk git branch --show-current
rtk git status --short
rtk git rev-parse HEAD
rtk cargo metadata --no-deps --format-version 1
rtk npm --version
```

If Git reports a worktree metadata problem, inspect it; do not reinitialize or
replace the checkout. The Windows UNC problem was environmental, not repository
corruption.

## Exact next task

Continue with one bounded 2550Q slice:

> Classify and bind the 53 identity-complete but documented-only
> derived/workflow serialization value projections, while keeping all three
> artifacts node-less and non-executable.

Do not jump to another form or promote 2550Q before this slice and the later
serialization gates are complete.

### Recommended execution sequence

1. Reproduce the current counts and classifications from:
   - `docs/validation-rules/2550q-adapter-map.md`
   - `rules/forms/2550q-v2024/fields.json`
   - `rules/forms/2550q-v2024/calculations.json`
   - `rules/forms/2550q-v2024/workflow.json`
   - `rules/forms/2550q-v2024/fixtures/runtime-control-inventory-v796.json`
   - `rules/forms/2550q-v2024/fixtures/serialization-binding-inventory-v796.json`
   - current core/XML/view implementation.
2. Create a new source-pinned review document for the 53 projections. Separate:
   - true derived calculation outputs;
   - aliases of canonical/raw fields;
   - artifact-specific constants/defaults;
   - imported/restored metadata;
   - workflow/UI state;
   - credentials or transport-only state that must not become form data.
3. For every projection record:
   - exact serialized key and occurrence;
   - exact semantic source identity;
   - artifact applicability;
   - semantic formatter;
   - body codec;
   - absent/blank behavior;
   - source references and review status;
   - explicit reason when it remains documented-only.
4. Use only value-projection kinds already supported by the closed v2 schema
   and runtime. Do not invent a fallback or encode an unsupported meaning as a
   string constant.
5. Derived outputs may use a derived projection only when the corresponding
   calculation, scope, instance selector, type, rounding, and fixture coverage
   are already exact. Otherwise leave the projection documented-only and name
   the missing calculation evidence.
6. Treat the nine workflow/metadata controls artifact-specifically. For
   example, do not assume one `txtFinalFlag` value applies to editable save,
   finalized save, and encrypted Final Copy.
7. Extend the reproducible builder
   `rules/tools/build-2550q-serialization-bindings.ps1`; do not hand-edit only
   the generated inventory.
8. Add focused assertions to `bir-rules-codegen` for the new classification and
   counts.
9. Keep:
   - `values_emitted: false`;
   - every artifact `documented_only`;
   - every artifact node list absent;
   - filing-safe unresolved;
   - reviewed registry empty.
10. Update evidence, audit, gaps, adapter map, architecture, and implementation
    plan together.

### Gate for this next slice

The slice is complete only when:

- all 53 identities are classified exactly once;
- no credential is exposed as ordinary field authority;
- no derived value is accepted without its calculation/scope proof;
- artifact-specific differences remain distinct;
- the builder reproduces the tracked value-free inventory;
- v2 audit and deterministic generation pass;
- all affected fixtures and tests pass;
- the full 43-form audit still passes;
- no production authority changes.

## Digest and regeneration procedure

The v2 candidate pins its source-set identity in:

- `rules/ir/v2/2550q-v2024-p7.9.6.0/rule-set.json`;
- `rules/ir/v2/index.json`;
- all 121 evaluation fixtures.

At the current fixture count, that is 123 JSON files. A source change requires
one atomic pin roll across that complete set.

The last session used this safe sequence:

1. Update evidence/review files.
2. Compute their SHA-256 hashes.
3. Update the corresponding `sources` entries in `rule-set.json`.
4. Rebuild derived inventories with their checked builder scripts.
5. Update the rebuilt inventory hash in `rule-set.json`.
6. Mechanically replace the old source-set pin with a 64-zero placeholder in
   exactly the rule set, index, and fixture identities.
7. Run the v2 audit. Its mismatch message reports the newly computed
   `source_set_sha256`.
8. Replace the placeholder in the same complete file set with that digest.
9. Rerun the audit; it must pass.
10. Run generation and capture the new generated-output and normalized audit
    digests.
11. Update audit/plan prose only after those values are final.

Do not confuse:

- `identity.source_set_sha256`, currently `e5181902...`;
- normalized audit/corpus digest, currently `67acef67...`;
- generated output digest, currently `d1fd97c5...`;
- serialization subtree digest, currently `c623ab78...`.

Verify the current fixture count before expecting 123 changed pins. Preserve
UTF-8 without BOM. A partial digest roll must fail rather than be patched
around.

## Commands for continuation

Cross-platform v2 and Rust gates:

```sh
rtk cargo fmt --all -- --check
rtk cargo test --locked -p bir-core form_2550q
rtk cargo test --locked -p bir-rules
rtk cargo test --locked -p bir-rules-codegen
rtk npm run rules:generate
rtk npm run rules:check
rtk cargo check --locked -p bir-core
rtk cargo check --locked -p bir-desktop
rtk git diff --check
```

Legacy/full corpus audit on macOS with PowerShell 7:

```sh
rtk pwsh -NoProfile -File rules/validate.ps1 -RequireJsonSchema
```

Rebuild the 2550Q value-free serialization inventory:

```sh
rtk pwsh -NoProfile -File \
  rules/tools/build-2550q-serialization-bindings.ps1 \
  -RepoRoot "$PWD"
```

`npm run rules:check` already invokes Cargo codegen and runtime checks. Running
the underlying crates separately remains useful for focused diagnosis.

Do not run multiple Cargo/codegen commands against the same target/output
directory concurrently. The codegen tests compile temporary candidate probes
and are intentionally expensive.

## Work after the 53-projection slice

Proceed in this order, one evidence-backed slice at a time:

1. Bind the relation between app-owned stable repeated-group order and the
   official live DOM/display serialization order.
2. Resolve all remaining calculations/scopes required by the 44 derived
   serialization values.
3. Pin separator, newline, encoding, non-ASCII, marker, omission, filename,
   overwrite, and path/custody behavior.
4. Review a production clock/timezone provider for `local-current-date`.
5. Model executable artifact nodes only after complete occurrence and byte
   coverage exists. Partial emission is forbidden.
6. Independently review filing-safe behavior and every official defect.
7. Complete production GPUI report/focus/action integration without duplicating
   rules in the view.
8. Revalidate and independently verify checked plaintext at Final Copy.
9. Bind outer encryption/container behavior and durable proof reconstruction.
10. Add queue/transport authorization only after exact trusted revalidation.
11. Promote a snapshot only in an explicit reviewed evidence-only step.
12. After 2550Q is a safe precedent, create additive v2 snapshots for the
    remaining forms in the documented priority/risk order. Never rewrite v1
    history.

## Key traceability files

Start here:

- `docs/validation-rules/implementation-plan.md`
- `docs/validation-rules/architecture.md`
- `docs/validation-rules/serialization-contract.md`
- `docs/validation-rules/2550q-adapter-map.md`
- `rules/README.md`
- `rules/UPDATING.md`

2550Q evidence:

- `rules/forms/2550q-v2024/manifest.json`
- `rules/forms/2550q-v2024/fields.json`
- `rules/forms/2550q-v2024/validations.json`
- `rules/forms/2550q-v2024/calculations.json`
- `rules/forms/2550q-v2024/workflow.json`
- `rules/forms/2550q-v2024/evidence.md`
- `rules/forms/2550q-v2024/audit.md`
- `rules/forms/2550q-v2024/gaps.md`
- `rules/forms/2550q-v2024/fixtures/serialization-binding-inventory-v796.json`
- `rules/forms/2550q-v2024/v2-candidate-serialization-binding-inventory-review.md`
- `rules/forms/2550q-v2024/v2-candidate-date-filed-context-projection-review.md`

Executable candidate:

- `rules/ir/v2/index.json`
- `rules/ir/v2/2550q-v2024-p7.9.6.0/rule-set.json`
- `rules/ir/v2/2550q-v2024-p7.9.6.0/fixtures/`
- `crates/bir-rules/src/generated/`
- `crates/bir-rules-codegen/src/`

Core and GPUI:

- `crates/bir-core/src/form_rules/form_2550q.rs`
- `crates/bir-core/src/forms/form_2550q.rs`
- `crates/bir-core/src/forms/form_2550q_xml.rs`
- `crates/bir-desktop/src/views/form_2550q_view.rs`
- `crates/bir-desktop/src/components/form_validation/`

## Final integrity checklist

At every handoff or stopping point, report:

- exact rule-set/revision/package/source identity;
- counts of fields, groups, fixtures, and occurrence projections;
- official and filing-safe branch states;
- artifact states and whether any nodes exist;
- reviewed registry contents;
- verification commands and exact results;
- environmental blockers separately from semantic failures;
- remaining gaps and the next bounded slice;
- confirmation that no submission, commit, push, release flag, or production
  authorization occurred.

If any evidence is missing on macOS, preserve the explicit gap. Do not infer an
official rule from memory, from the typed model alone, or from a different
form/package revision.
