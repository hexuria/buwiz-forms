# macOS handoff — library-first validation rules

Recorded 2026-07-27 for continuation on macOS.

This is the live session handoff for the validation-rules objective. The older
`handoff.md` is historical. The authoritative objective and sequence remain:

- `.claude/GOAL.md`
- `docs/validation-rules/execution-plan.md`

Do not use this handoff to weaken either document.

## Decision: switch to macOS now

The immediate engineering work no longer requires Windows. Continue on macOS
from:

```text
/Volumes/goldcoders/reverse-engineer-ebir-forms/bir
```

on branch:

```text
main
```

**Superseded 2026-07-28.** This section previously pointed at a
`bir-print-parity` worktree on `codex/print-preview-parity` and told you not to
work in `bir`. That worktree no longer exists: its branch was fast-forwarded
into `main`, the worktree was removed, and `bir` - which has always held the
object database - is now the single checkout on the single branch. There is no
sibling checkout to avoid any more.

Not every future Windows observation is finished. Windows will be needed again
for targeted official-application evidence that cannot be derived from the
tracked corpus. That is not the next task, and it should be scheduled only
after the portable library tooling reports a concrete evidence gap.

## Exact repository state

At handoff:

- portable library-foundation checkpoint:
  `f2e78ef52ce1bc9ff9dabecea97590c09ce84c46`;
- local-artifact ignore checkpoint:
  `e772d390fd6bfacb983157cdb936517e400f73ed`;
- the documentation checkpoint is the commit containing this file;
- pre-publication upstream checkpoint:
  `de828fd05ce27afa5c71ffd88c7a8bb2b3f9a8a5`;
- the checkpoint series has since been merged into `main` and published to
  `hexuria/buwiz-forms`, where CI is green on all ten jobs across macOS, Linux
  and Windows; the draft PR referenced here was on the now-detached
  `codeitlikemiley/ebirforms` remote and is no longer the place to check state;
- the working tree retains unrelated user-owned mode changes after the
  checkpoint series;
- the explicit publication authority covers this checkpoint series only, not
  future unrelated commits or pushes; and
- worktree cleanup was explicitly authorized and carried out on 2026-07-28,
  ahead of the point the Worktree policy section below schedules it.

Confirm the checkout before doing anything:

```sh
cd /Volumes/goldcoders/reverse-engineer-ebir-forms/bir
rtk git rev-parse --show-toplevel
rtk git branch --show-current
rtk git rev-parse HEAD
rtk git status --short --branch
```

Expected top level and branch are the path and branch above. `bir/.git` is now a
real repository directory rather than a worktree pointer, so there is no
linked-worktree metadata left to repair.

## Honest progress against the library-first plan

### Corpus and candidate coverage

| Measure | Current state |
| --- | ---: |
| Indexed forms | 43 |
| v1 fields | 9,592 |
| v1 validations | 2,007 |
| v1 calculations | 623 |
| Portable evidence packets | 43/43 |
| Canonical v2 candidates | 1/43 |
| Executable validations | 27/2,007 |
| Executable calculations | 1/623 |
| Reviewed production rulesets | 0 |
| Production-resolvable snapshots | 0 |

The only canonical v2 ruleset remains 2550Q. It remains `candidate`,
test-only, unresolved for filing-safe policy, and not promoted. The 53
serialization projections are completely classified but intentionally
non-executable.

The packet phase being complete does **not** mean the candidate-library phase
is complete. The largest remaining body of work is still authoring and
verifying strict v2 candidates for all 43 forms, beginning with 1701Q.

### Portable packet factory

Completed in the working tree:

- 43 exact form identities have packets under
  `evidence/validation-rules/packets/v1`;
- the set contains 130 canonical JSON files;
- aggregate packet-set digest:
  `9fa6df6c6657166140b37b67d4d7ca382ce3dc4986fb13f4f6a8caf0e828ac74`;
- external reviewed-ledger SHA-256:
  `b79f9c547199b6261506588787f75fae861fe1480cfc6a0b2a34df4f62155a4f`;
- external vault-catalog SHA-256:
  `5692989438d3a92c4b9281f6726fa4c947694482bec8527d6efbc7bb82aa08e7`;
- portable verification correctly reports
  `full_upstream_verified: false` without a vault;
- verification reports `full_upstream_verified: true` only when checked
  against the explicit external vault;
- the importer refuses overwrite and imports reviewed derived material into a
  separate staging root rather than mutating the corpus; and
- the 1701Q format proof verified with and without the vault, then imported
  exactly two reviewed derived files into a fresh external staging root.

Raw official packages remain external. Do not copy them into the repository.

### Generic v2 runtime and code generation

The working tree adds generic, form-independent support for:

- exact field-event programs;
- ordered calculation/effect interleaving;
- mutable calculation writeback;
- exact money and year normalization;
- finite-only selected calculation output; and
- additional schema, audit, census, reconciliation, coverage, generation, and
  status support required by those semantics.

2550Q is the regression fixture. No new behavior should be handwritten in
`bir-core`, GPUI, or generated Rust.

### External-input trust boundary

The shared filesystem capability layer now includes:

- `ApprovedExternalFile`;
- `ApprovedExternalRoot`;
- retained root and file handles;
- root revalidation;
- `read_external_bytes_under`;
- `read_external_tree_under`;
- `read_external_bytes_bound`;
- `read_external_tree_bound`;
- canonical absolute lexical normalization using one current-directory sample;
- path-escape, symlink, hardlink, special-entry, child/root replacement,
  mutation, addition/removal, type-swap, and late-failure defenses; and
- strict Windows identity behavior, including fail-closed handling for
  unsupported `FILE_ID_INFO` combinations.

The core tests passed on Windows:

| Suite | Result |
| --- | --- |
| `files::tests` | 25 passed |
| `verified_file::tests` | 11 passed |
| `bir-rules-platform` | 4 passed |

The Windows implementation uses 128-bit file identity for the final object and
every ancestor. Strict external reads reject Windows error 87. A narrower
tracked-checkout fallback is allowed only when every live ancestor and final
handle uniformly lacks that identity support; mixed support rejects.

The remaining documented Unix limitation is an adversarial same-inode,
same-size mutation that the platform cannot make impossible with these APIs.
Independent content digests remain mandatory.

### Reader migrations completed

Production external reads have been migrated in:

- `audit.rs`
- `schema.rs`
- `check.rs`
- `form_factory.rs`
- `evidence.rs`
- `evidence_set.rs`
- `evidence_review_scaffold.rs`
- `vault_acquisition.rs`
- `vault_source_discovery.rs`

These call sites retain an approved root or exact approved file and perform
child reads through the retained capability. Tracked reads remain separate.

The last interrupted Windows refactor in `vault_acquisition.rs` was repaired
before this handoff:

- the unused `read_external_tree_bound` import was removed; and
- the source-asset `hash_regular_file` call again supplies its fourth
  `approved_root` argument as `None`.

`git diff --check -- crates/bir-rules-codegen/src/vault_acquisition.rs` passes.
The complete tree has **not** been cargo-checked after the final
`vault_acquisition.rs` and `vault_source_discovery.rs` edits. That check is the
first macOS verification task.

### Reader migration still open

`crates/bir-rules-codegen/src/form_integration.rs` is the next implementation
target.

Its reviewed packet root still relies on transient `same_file::Handle`
checking through helpers such as `approve_external_directory`,
`read_tree_stably`, and `approved_packet_identity`. Its staging root is also
canonicalized transiently rather than retained from planning through capture.

Migrate both roots to retained `ApprovedExternalRoot` capabilities:

1. capture the exact reviewed-packet root and retain it through every packet
   read and final revalidation;
2. capture the exact staging root and retain it from plan construction through
   staged-tree capture;
3. use `read_external_bytes_under` or `read_external_tree_under` for all child
   inputs;
4. preserve all existing digest, canonicalization, overwrite, symlink, and
   path-containment checks;
5. add root/child substitution and mutation tests; and
6. do not change output publication semantics while migrating input trust.

Afterward audit all remaining sites:

```sh
rtk rg -n \
  'read_external_bytes\(|read_external_tree\(|same_file|Handle::from_path' \
  crates/bir-rules-codegen/src
```

`projections.rs` has a path-only external read under `#[cfg(test)]`; it is not a
production reader. Once production use is zero, make the path-only convenience
readers test-only/private or remove them. Separately audit remaining
`same_file` uses in output publication code before deciding whether they need
the same treatment; do not mechanically replace output-install logic with
input-read logic.

## First macOS verification sequence

Run these before further semantic work:

```sh
rtk cargo fmt --all -- --check
rtk cargo check --locked -p bir-rules-codegen --tests
rtk cargo test --locked -p bir-rules-codegen files::tests
rtk cargo test --locked -p bir-rules-codegen verified_file::tests
rtk cargo test --locked -p bir-rules-platform
rtk cargo test --locked -p bir-rules-codegen evidence::tests
rtk cargo test --locked -p bir-rules-codegen evidence_set::tests -- --test-threads=1
rtk cargo test --locked -p bir-rules-codegen evidence_review_scaffold::tests
rtk cargo test --locked -p bir-rules-codegen form_factory::tests
rtk cargo test --locked -p bir-rules-codegen schema::tests
rtk cargo test --locked -p bir-rules-codegen check::tests
```

Known prior results:

- `form_factory::tests`: 3 passed;
- `schema::tests`: 20 passed;
- `evidence::tests`: 24 passed;
- `evidence_set::tests`: 13 of 14 completed successfully on Windows;
- its long
  `exact_43_packet_set_is_byte_stable_and_vault_checkable` test was stopped
  during the machine switch with no failure reported;
- a previous rerun was prevented only by a stale Windows test executable
  holding the output file; the stale processes were identified and killed;
  and
- the final full cargo check was not rerun after the last test-fixture and
  vault-reader edits.

Treat the Mac results as authoritative for the current working tree. Fix code,
not expectations, if a safety test fails. Do not weaken Windows-specific
identity tests merely because they are compiled out on Unix.

After `form_integration.rs` is migrated and the production-reader scan is
clean, run the full library gates:

```sh
rtk cargo run -q --locked -p bir-rules-codegen -- status
rtk cargo run --locked -p bir-rules-codegen -- validate-v1
rtk cargo test --locked -p bir-rules-codegen \
  landed_v1_corpus_reconciles_its_published_counts
rtk cargo run --locked -p bir-rules-codegen -- audit
rtk cargo run --locked -p bir-rules-codegen -- coverage --json
rtk cargo run --locked -p bir-rules-codegen -- operator-census --json
rtk cargo run --locked -p bir-rules-codegen -- reconciliation --json
rtk cargo run --locked -p bir-rules-codegen -- roll-pin --all --dry-run
rtk cargo run --locked -p bir-rules-codegen -- check
rtk cargo test --locked \
  -p bir-rules -p bir-rules-codegen -p bir-rules-platform
rtk cargo test --locked -p bir-core form_rules
rtk cargo test --locked -p bir-desktop form_validation
rtk cargo fmt --all -- --check
```

Default `status` is expected to remain nonzero while active-library criteria
are incomplete. Boundary criteria must all remain green. Do not delete,
reclassify, or weaken a criterion to obtain exit code zero.

Codegen source changes will likely require a tool-owned source-set digest and
generated-manifest refresh for the 2550Q regression fixture. Inspect the
dry-run transaction first. Use only the atomic codegen command; never hand-edit
or partially apply the pin set.

## 1701Q: exact current state

The first ordered form is `1701q-v2018`. Its external authoring workspace maps
to:

```text
/Volumes/goldcoders/reverse-engineer-ebir-forms/validation-rules-form-work/1701q-v2018-candidate-authoring-20260727
```

The corresponding Windows path was:

```text
X:\reverse-engineer-ebir-forms\validation-rules-form-work\1701q-v2018-candidate-authoring-20260727
```

Do not move the external workspace into the repository.

### Reconciled base

- skeleton tree digest:
  `0309add72b8066b36693a05211bd6728ba950ce855f83a44d211760b1e1a0f97`
- 172 fields accounted for;
- 40 validation records reconciled as 33 executable plus 7 source-backed
  classifications;
- 19 calculations accounted for;
- 2 workflow states;
- 1 executable workflow transition;
- 7 workflow classifications;
- TIN checksum, selected-index/RDO state, the documented official no-op,
  ordered JavaScript rounding, and all three item-46 tax schedules are backed
  by portable reviewed evidence, including the pre-2018 branch.

### Event inventory

- source SHA-256:
  `be131046...`
- 204 event attributes / 203 pairs;
- 79 direct programs: 51 Blur and 28 Click;
- 69 ordinary direct editable programs;
- 10 unavailable programs;
- `cmdEdit` contains 15 mixed segments;
- `processATC` occurs 18 times;
- 88 spouse calculation 27–30 locators are exact;
- money normalization: 46 total / 40 ordinary / 6 exceptional;
- year normalization: 96; and
- dynamic selector `.chkTin input` remains unresolved.

The abbreviated hash is copied from the external audit notes. Recompute and
record the full hash from the file before source-locking it.

### Finite-domain supplement

- draft source SHA-256:
  `21e80ff...`
- canonical digest:
  `9c9f1aec...`
- verifier SHA-256:
  `f5a7e4...`
- gap-report SHA-256:
  `a9f7be...`
- 19 envelopes;
- 40 ordinary inputs;
- 37 outputs;
- 5 fixture groups.

This material remains draft-unlocked:

```text
candidate_generation_authorized = false
source_lock = null
```

Do not replace abbreviated hashes with guesses. Resolve the complete values
from the external workspace and verify them before review.

### Candidate-generation guard

The external generator and determinism script now invoke the authorization
guard before their first mutation.

| Artifact | SHA-256 |
| --- | --- |
| `verify-candidate-generation-authorization.ps1` | `ef9c56227532eb6d145591d73269d018e65360abc74e895f1760a6c2f1a50491` |
| guard verifier | `cac7efcbde650ee43a12b42cb9bbead3b5a55ad6f8ef2f17bce38168cf28dabe` |
| `generate.ps1` | `1640946b78decc10486e718d3aea51fbe314da76c3dc96f2d8078cea5156fc96` |
| determinism script | `3b75f749480ab6cc45909e860b66e6fb93fc6436e538e2b71f6cc5c0683d2f77` |

The guard requires reviewer identity, reviewed decision, UTC review timestamp,
explicit candidate-generation authorization, exact dependency pins, and a
complete source lock. Its verifier proves the present drafts are rejected
without changing protected artifacts.

Protected artifact hashes:

- authoring manifest: `4c0d0f878...`
- source-lock verifier: `82e7bcf4...`
- existing external candidate: `059e2597...`
- locked review: `445fb9...`

Resolve full values from the external records before relying on them. Do not
generate or import the 1701Q candidate while the guard rejects the review
state.

### Recovered official asset

The official 1701Q HTA was independently recovered and proven byte-exact
against the executable resource:

- `BIRForms.exe` SHA-256:
  `de8ef...`
- executable size: 57,506,304 bytes;
- recovered HTA SHA-256:
  `5f164dde...`
- HTA size: 372,180 bytes;
- XOR preimage SHA-256:
  `96d2be...`;
- embedded offset: 13,863,016; and
- byte mismatches: zero.

The unlocked recovery receipt is:

```text
/Volumes/goldcoders/reverse-engineer-ebir-forms/validation-rules-form-work/1701q-v2018-candidate-authoring-20260727/VAULT_RECOVERY_1701Q_20260727.UNLOCKED.md
```

The short hashes here are navigation aids, not source locks. The external
receipt contains or derives the exact identities.

### What can be done on macOS for 1701Q

After the reader migration and all library gates are stable:

1. materialize executable fixtures from the 19 finite-domain envelopes;
2. cover endpoint values and just-outside-envelope rejection;
3. verify exact rounding, event ordering, writeback, invalid finite text, and
   no-partial-sibling-write semantics in the generic runtime;
4. complete all source locators that are derivable from the recovered HTA and
   packet;
5. regenerate the gap report;
6. keep any observation-only gap explicitly evidence-blocked; and
7. prepare the two draft records for independent review without granting
   authorization.

The official `round(this, 2); compute...` sequence maps ordinary invalid finite
text and overprecision to `0.00` before arithmetic. Signed Infinity produces an
official malformed non-decimal string. Do not replace this with the
provisional `on_invalid=error` shorthand, and do not silently map non-finite
values to zero or absence.

### Later targeted Windows evidence for 1701Q

Windows is still required to observe official-application behavior for gaps
that the static HTA and portable evidence cannot prove:

- the dynamic `.chkTin input` selector;
- unresolved conditional event branches;
- four intentionally unavailable anonymous handlers;
- Save and reopen behavior;
- external-DOM overflow behavior;
- no-partial-sibling-write behavior where static reasoning is insufficient;
- drag/drop disposition; and
- any exact modal/order behavior left by the refreshed gap report.

Use only dummy data. Do not use real taxpayer information, credentials, the
official online submission path, or the live encrypted database.

This should be one narrow evidence session driven by the Mac-generated gap
report, not an open-ended Windows development session.

## Subsequent form order

Do not create new worktrees and do not advance past an unresolved form. The
required order is:

```text
1701Q → 1601EQ → 1702Q → 1601FQ → 1603Q → 1600VT → 1600PT →
2550M → 1701-MS → 1701 → 1701A → 1700 → 1702RT → 1604C → 1604E →
1604F → 1702MX → 1706 → 1606 → 1800 → 1801 → 2000OT → 2000 →
1602Q → 1600WP → 1702EX → 1707 → 1707A → 2552 → 2553 → 2200A →
2200AN → 2200C → 2200M → 2200P → 2200S → 2200T → 0605 → 0619E →
0619F → 1601C → 2550Q → 2551Q
```

For every form: pin evidence, reconcile every v1 record, emit a gap report,
request only necessary official-app evidence, author the official-compatible
v2 profile, add fixtures, classify every non-runtime record with a source,
audit, generate, test, dry-run the digest transaction, and emit a form
handoff.

## Production boundary freeze

All five production switches must remain closed:

1. `crates/bir-core/src/form_rules/form_2550q.rs` returns no reviewed default;
2. generated candidate modules remain test-only;
3. the core candidate evaluator remains test-only;
4. `crates/bir-rules/src/generated/registry.rs` remains empty; and
5. `CheckedFinalCopyPayload::try_new` continues to fail with
   `MissingSerializationContract`.

Also frozen:

- `bir-core` application adapters;
- GPUI and `bir-desktop` production validation call sites;
- persistence authority;
- Final Copy;
- encryption/container binding;
- queueing and transport;
- submission;
- 2550Q filing-safe policy and promotion;
- reviewed-registry entries; and
- application capability flags.

Read-only tests may strengthen these boundaries. No production integration may
land during the 43-form library objective.

## Working-tree ownership and preservation

Intentional validation-library changes currently span:

- `.claude/GOAL.md`;
- `docs/validation-rules/`;
- `evidence/`;
- `rules/schema/v2/`;
- `crates/bir-rules-codegen/`;
- `crates/bir-rules-platform/`; and
- `crates/bir-rules/`.

`crates/bir-rules-codegen/src/canonicalize_json.rs` and the repository
`evidence/` tree are intentional untracked additions. Inspect and preserve
them.

The following changes/artifacts are unrelated or not yet proven disposable.
Do not modify, delete, stage, or commit them as part of validation work:

```text
scripts/linux_candidate_collector.py
scripts/verify_offline_form_renderer.py
.claude/settings.local.json
.codex/skills/ebirforms-convert-form-to-html/scripts/__pycache__/
.codex/skills/tests/__pycache__/
apps/form-calibration/dist/
scripts/__pycache__/
scripts/tests/__pycache__/
470
crates/bir-rules/src/generated.bir-rules-codegen-backup-10236-1/
crates/bir-rules/src/generated.bir-rules-codegen-backup-16768-1/
crates/bir-rules/src/generated.bir-rules-codegen-backup-17112-1/
crates/bir-rules/src/generated.bir-rules-codegen-backup-20028-1/
crates/bir-rules/src/generated.bir-rules-codegen-backup-21356-1/
crates/bir-rules/src/generated.bir-rules-codegen-backup-24488-1/
```

Do not run broad clean, reset, checkout, restore, prune, or deletion commands.
Do not assume generated backup directories are disposable until their content
and provenance have been inspected.

## Worktree policy

**Amended 2026-07-28.** Cleanup was authorized early and has been done. The
original rule - no cleanup during the objective - is superseded by that
decision, not by drift.

What was carried out, following this section's own safety requirements: all 41
worktree directories removed except `bir`; every branch pushed to
`codeitlikemiley/ebirforms` at its exact commit and verified SHA-for-SHA before
any deletion; a `rescue/<branch>` tag created for all 32 branches carrying
commits not reachable from `main`; the inventory recorded in
`docs/validation-rules/worktree-cleanup-inventory-20260728.md`. No branch was
deleted for lack of a worktree - branches were deleted only after their backup
was verified.

Going forward: still no new worktrees before the complete 43-form
candidate-library baseline. Work happens directly in `bir` on `main`. Any future
app-integration worktree must branch from the eventual immutable
validation-library baseline.

## Completion definition

Do not call the library objective complete until:

- 43/43 exact revisions have strict v2 candidate snapshots;
- all 9,592 field records are reconciled;
- all 2,007 validations and 623 calculations are represented;
- every reachable local rule/calculation is executable under the official
  profile;
- every non-executable record has one source-backed closed-reason
  classification;
- unclassified and unresolved legacy-record counts are zero;
- every form's fixtures use the same `bir-rules` evaluator;
- generation is deterministic on macOS, Linux, and Windows;
- the non-default candidate catalog compiles;
- all production boundaries remain closed; and
- the final report records measured executable totals without inferring filing
  safety, promotion, application support, or release readiness.

The immediate next task on macOS is therefore:

> Verify the current tree, finish the retained-capability migration in
> `form_integration.rs`, eliminate production path-only external readers, run
> the full library gates, and only then resume 1701Q finite-domain fixtures and
> gap closure.
