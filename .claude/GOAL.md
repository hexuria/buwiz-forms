# Goal: complete every macOS-achievable task in the validation-rules plan

Execute the approved plan at
`~/.claude/plans/do-you-even-consider-eager-rainbow.md`: consolidate the live
PowerShell tooling into `bir-rules-codegen`, close the 53-projection slice, and
finish the remaining work that does not require a Windows machine — without
opening any production filing authority.

Repo: `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity`
Branch: `codex/print-preview-parity`. Prefix shell commands with `rtk`.

The Wave 0 print-parity objective this file previously held is preserved at
`.claude/GOAL.wave0-print-parity.md`. Do not mix the two.

## Done when

```sh
rtk cargo run -q --locked -p bir-rules-codegen -- status
```

`status` is the single machine-checkable done-condition. It exits 0 only when
every **boundary** criterion still holds *and* every **slice** criterion is met.
It currently exits 1. As each plan deliverable lands, add its criterion to
`crates/bir-rules-codegen/src/status.rs` — the condition may only ever get
stricter.

Plus these, with no regressions:

```sh
rtk cargo run -q --locked -p bir-rules-codegen -- validate-v1
rtk npm run rules:check
rtk cargo test --locked -p bir-rules -p bir-rules-codegen
rtk cargo fmt --all -- --check
rtk git diff --check
```

Baselines that must not drop: `validate-v1` reports **43 / 659 (139 v2) / 9,592
/ 2,007 / 623 / 1,354 / 216** — these eight are invariant and a change means the
work is wrong.

The digests below **did** change, once, deliberately, as the verified
consequence of the P0.6 line-ending transaction. This is not a weakened gate:
the done-condition (`status`) is untouched, and every criterion in it got
stricter, not looser.

| digest | before | now |
| --- | --- | --- |
| `source_set_sha256` | `e5181902…` | `c58f88b5…` |
| `generated_output_sha256` | `d1fd97c5…` | `3033a60e…` |
| `normalized_source_sha256` | `67acef67…` | `e67fda80…` |

## Method

Work the plan's phases in order. Within a phase, take the smallest unit that can
be verified on its own, verify it, and revert anything that regresses.

- **Reproduce before changing.** Copy inputs to the scratchpad and run builders
  there with `-RepoRoot <scratch>`. Never let an exploratory run write into
  `rules/`.
- **Compare semantically when judging a builder.** `jq -S -c` equality is the
  test of whether evidence changed. A byte difference with `jq -S -c` equality
  is a formatting change (the point of P0.6), not an evidence regression.
  Confusing the two will burn the session.
- **After touching any generator source**, run `rtk npm run rules:generate` and
  confirm the manifest diff is confined to `generator.source_sha256`.
  `generated_output_sha256` and `normalized_source_sha256` must not move.
- **Add the criterion before or with the deliverable**, so `status` tightens as
  work lands rather than after.

## Constraints

Non-negotiable. These outrank speed and outrank finishing.

- **Never weaken a validator, threshold or assertion to make a gate pass.** If
  `validate-v1` changes any of the eight counts, the change is wrong.
- **Five switches stay exactly as they are** — each is one edit from a
  production filing path:
  `crates/bir-core/src/form_rules/form_2550q.rs:42-44` (returns `None`),
  `crates/bir-rules/src/generated/mod.rs:4` (`#[cfg(test)]`),
  `crates/bir-core/src/form_rules/form_2550q.rs:930` (`#[cfg(test)]`),
  `crates/bir-rules/src/generated/registry.rs:18-19` (empty),
  `crates/bir-core/src/form_rules/payload.rs:228-234` (always `Err`).
- 2550Q stays `candidate`; filing-safe stays `unresolved`; all three artifacts
  stay `documented_only`, node-less, `values_emitted: false`.
- No Final Copy, queue, transport, `QUEUE_SUBMISSION_SUPPORTED`,
  capability-flag, `release_ready`, renderer-status or print-threshold change.
- Preserve official defects in the official profile; safer behavior is a
  separately evidenced filing-safe decision, never a silent fix.
- Never use the official submission path for discovery, never real taxpayer
  data, never the live encrypted database.
- A source-set digest roll is a **123-file atomic transaction**. Partial rolls
  must fail. Use the P0.4 `roll-pin` command once it exists; until then, do not
  start one.
- No broad `git clean` / `git reset` / worktree pruning. Do not delete or commit
  `tmp/` or `test-results/` uninspected.

## Traps found by review — do not re-discover these

- **`Join-Path` normalizes `\` → `/` on macOS.** `validate.ps1:32` and `:53` are
  therefore correct. The real breakages were `:5-6`, `:97`, `:112`. Already
  fixed by the Rust port; do not "fix" them again in PowerShell.
- **`ConvertTo-Json` output differs between Windows PowerShell 5.1 and pwsh 7**
  (two spaces after `:`, nested objects aligned under the parent key's column).
  The tracked evidence was written by 5.1 and no modern tool reproduces it. A
  one-time canonical reformat is unavoidable; the content is fine.
- **The five `Get-FileHash` pins match the CRLF working tree, not the LF index.**
  `source_set_sha256` is computed over parsed JSON and is EOL-immune, so fixing
  this does not cascade into a digest roll.
- **The 53 documented-only projections exist only as a set difference** against
  `rule-set.json`; the inventory pins five inputs but not the rule set.
- **The two v2 projectors are order-dependent** (group asserts 60 fields, static
  asserts 94) with nothing encoding the order.

## In flight

- **Background coder agent porting the static projector** to
  `crates/bir-rules-codegen/src/projections.rs` as `project-2550q`. Fidelity
  port; the oracle is idempotency — running it against a scratch corpus copy
  must leave `rule-set.json` and all 121 fixtures semantically identical. It
  owns `projections.rs`, `lib.rs`, `main.rs`, `generate.rs`, `package.json`.
  Do not edit those concurrently, and do not touch `rules/ir/v2/**` while it
  runs. Verify the oracle yourself before trusting its report.

## P0.3 was re-scoped by evidence

Both projectors were run against a scratch copy of the current corpus:

- **`update-2550q-v2-group-projections.ps1` is spent.** It asserts 7 groups and
  **60** total candidate fields; the corpus has **94**, so it throws
  immediately. It was a one-shot migration (32 singletons + 28 family
  descriptors = 60) that the static projector then extended to 94. It can never
  run again — it is archaeology, now labelled as such in `rules/tools/README.md`.
  Do not "fix" its assertion; that would re-partition an already-projected rule
  set.
- **`update-2550q-v2-static-projections.ps1` is idempotent** and already works
  on macOS under pwsh 7: re-running reproduces `rule-set.json` and all 121
  fixtures semantically identically.

So the group-before-static ordering hazard recorded earlier is **moot** — there
is no ordering, because only one of the two can execute. Its formatting is also
inert: `source_set_sha256` is over parsed canonical JSON and the generated Rust
embeds canonical JSON, so neither depends on file layout.

## The done-condition was tightened, not satisfied

After P1.1 landed, `status` passed — **and that was wrong**. It only covered the
2550Q slice, while P0.3, P1.5, P1.6, P2.3 and D4 were still outstanding. A
done-condition satisfiable before the work is finished is worse than none, so
five machine-checkable criteria for the remaining plan deliverables were added.
`status` is now 11 met / 5 open.

Per the loop's own rules: the condition may only ever get stricter. Never remove
a criterion to make it pass.

## RESOLVED — the audit now survives a clone

The blocking discovery below is **fixed**. Verified by copying the worktree as a
clone would receive it and running against that copy: `audit` exits 0, and
`build-2550q-bindings` reproduces the tracked inventory exactly. `status` now
reports `declared-sources-clone-reproducible` as met — 30 declared sources, none
CRLF.

What it took, in order, as one transaction:

1. Normalized 599 CRLF files under `rules/` to LF. Verified byte-identical to
   what git already stored: `git diff --name-only -- rules/` reported exactly
   one file, the one deliberately deleted earlier. **No content changed.**
2. Re-pinned the builder's five input hashes in `bindings.rs` over LF content
   (the builder failed first with the predicted
   `Pinned 2550Q serialization binding input changed`, confirming the diagnosis).
3. Rebuilt the inventory. Semantic delta versus the tracked file was **exactly
   and only** the five `input_sha256` values; all 160 occurrence bindings,
   7 dynamic groups, 3 artifact boundaries and 7 unresolved boundaries identical.
4. Ran `roll-pin`, which re-pinned all 12 declared sources and rolled
   `source_set_sha256` across 123 files / 246 sites atomically. `roll-pin` is
   now idempotent — a second dry run reports "already consistent".
5. Regenerated. `audit` passes; the clone simulation passes.

`schedule-family-inventory` was a second, independent hazard: a builder input
pinned in `bindings.rs` but **not** an audit-declared source, so the
`declared-sources-clone-reproducible` criterion alone would not have caught it.
Normalizing all of `rules/` closed both.

## Original blocking discovery (kept for the record)

**`cargo run -p bir-rules-codegen -- audit` fails on any fresh clone, on every
OS.** Proven, not derived: materializing `rules/` from the git index (which is
LF-normalized by `.gitattributes`) and running the audit against it gives

```text
error: source `v1-manifest` hash mismatch for `forms/2550q-v2024/manifest.json`:
  expected 32e4062f…, found 9fcea99e…
```

**12 of the 30 declared sources** in `rule-set.json` are CRLF files whose pinned
SHA-256 matches this working tree only: `v1-manifest`, `v1-fields`,
`v1-validations`, `v1-calculations`, `v1-workflow`, `v1-negative-fixtures`,
`v1-calculation-fixtures`, `v1-validation-function-inventory`,
`v1-runtime-control-inventory`, `v1-encrypted-field-audit`,
`v1-plaintext-field-audit`, `v1-serialization-binding-inventory`. The 18
markdown review documents are already LF and are fine.

It went unnoticed because these files are staged but never committed, and the
branch had no CI until P0.9 extended the triggers. The next CI run would have
gone red on `validation-rules-v2`.

`status` now asserts this mechanically as
`declared-sources-clone-reproducible`, so it cannot be forgotten again.

### Consequence: the plan's ordering was wrong

P0.6 (canonical reformat) **cannot precede P0.4 (`roll-pin`)**, and its scope is
larger than "the three tracked artifacts". Normalizing any declared source
changes its hash → `sources[]` in `rule-set.json` must be re-pinned →
`rule-set.json`'s canonical content changes → `source_set_sha256` changes → the
**123-file atomic digest roll** fires. All of it is one transaction.

Corrected order: **P0.4 → P0.3 → P0.6**.

## Progress

**Done and verified:**

- **P0.1 `validate-v1`** — v1 corpus audit ported to Rust. Runs on macOS (the
  PowerShell original exits 1). All eight counts reconcile with the Windows
  baseline. All 216 schema documents pass the broader Rust validator; no new
  rejections. Needed zero new validation code.
- **P0.5 `status`** — machine-checkable done-condition. Exits 1 with 8 boundary
  criteria held and 2 slice criteria open (`inventory-pins-rule-set`,
  `occurrence-classification-complete`). Independently re-derived the 53
  documented-only projections from live data and confirmed 53 = 44 + 9.
- Closed a latent gap: `GENERATOR_SOURCES` in `generate.rs` is now covered by a
  test that fails when the list drifts from `src/*.rs`.
- `rules:validate-v1` and `rules:status` wired into `package.json`.
- Suite at **213 passed** (204 baseline + 9 new); `rules:check` exit 0 with the
  output digest unchanged; manifest diff confined to `generator.source_sha256`.

- **P0.8** — `rules/tools/README.md` written: names the 5 live scripts, explains
  why the other 65 are provenance records that cannot run from a clone on any
  OS, and records the projector ordering and the `ConvertTo-Json` trap.
  `run-full-audit-background.ps1` deleted (hardcoded `R:\`,
  `C:\Users\uriah\...\Temp`, `powershell.exe`); still recoverable from the index.
- **P0.9** — CI: `validation-rules-v1` now runs the Rust `validate-v1` on
  ubuntu-22.04, macos-14 **and** windows-latest (it was a Windows-only
  PowerShell job, which is precisely why three path defects went undetected);
  `rules-codegen audit` added, having never run in CI; `push` trigger extended
  to `codex/**` so this branch gets CI at all. YAML re-parsed and verified.
- **P0.10** — `docs/validation-rules/execution-plan.md` rewritten against the
  Rust decision; the redundant `.claude/GOAL.validation-rules.md` removed.
- **D1** — validation-rules section added to **both** `CLAUDE.md` and
  `AGENTS.md`: the subsystem had *zero* durable instruction coverage, and "the
  2550Q candidate is test-only, never promote" existed in no instruction file.
  Both now name the five guards and the corpus baseline.
- **D2** — instruction-file drift closed: `AGENTS.md` no longer contradicts
  itself on criterion status; `CLAUDE.md` regained the six-component table and
  the `## Key commands` block; `AGENTS.md` regained the
  `--require-clean-source` rule; `CLAUDE.md`'s false claim that `AGENTS.md` is
  legacy-only corrected to `AGENT.md` (singular). Remaining differences are
  line-wrapping only.

- **P0.2 `build-2550q-bindings`** — `build-2550q-serialization-bindings.ps1`
  (441 lines) ported to `crates/bir-rules-codegen/src/bindings.rs`. **Fidelity
  verified independently**, not taken on trust: rebuilding to scratch and
  diffing `jq -S -c` against the tracked inventory gives an empty diff. Emitted
  file has 0 CR bytes, one trailing LF, no BOM, 2-space indent, sorted keys. The
  tracked inventory is untouched (`A `, not `AM`).
  - Findings reported by the port, preserved rather than fixed: `fields.json` is
    pinned and its hash published but the parsed document is never used;
    `rule_set` is read and materially affects 40 emitted values yet is **not**
    pinned (this is the `inventory-pins-rule-set` criterion); one `elseif` branch
    is dead; PowerShell's case-insensitive `-in`/`-match`/`ContainsKey` differ
    from the ordinal port but were verified inert on the pinned corpus; the
    prefix sort was unstable in PowerShell but cannot disambiguate anything here.
- **Suite at 217 passed** (204 baseline + 13). `rules:check` exit 0 with output
  digest unchanged; manifest diff still confined to `generator.source_sha256`.

- **P0.4 `roll-pin`** — atomic re-pin of `source_set_sha256` and the declared
  source hashes. Enumerates 123 files / 246 sites, asserts every site count
  before writing, and restores every touched file if any write fails —
  `handoff.md:524` demanded that and nothing enforced it.
  - **The handoff's procedure was unnecessary.** It prescribes a 64-zero
    placeholder, a deliberately-failed audit, and scraping the digest from the
    error text. `snapshot_source_digest` nulls the pin fields *before* hashing,
    so the digest never depended on the current pin values and is computable
    directly. `roll-pin` does that instead.
  - Substitution is textual with exact per-file occurrence assertions, so a roll
    changes only digests and leaves formatting alone.
- **P0.6 line-ending transaction** — see the RESOLVED section above.

- **P1.1 + P1.2 + P1.3** — every occurrence binding now carries an explicit
  `classification`, so "the 53" is no longer a set difference nobody can
  reproduce. Distribution, asserted by the builder itself and independently
  confirmed in the emitted file:

  | classification | count |
  | --- | ---: |
  | `executable-singleton` | 66 |
  | `executable-group-field` | 40 |
  | `documented-only-derived-or-alias` | 44 |
  | `documented-only-workflow-or-ui` | 5 |
  | `documented-only-credential` | 4 |
  | `generated-context-metadata` | 1 |

  Documented-only totals **53**, matching the published split. The credential
  and workflow/UI members are the nine enumerated verbatim at
  `v2-candidate-static-surface-projection-review.md:62-70` — source-backed, not
  invented. The builder fails if any count drifts.

- **The rule-set join could not be pinned the obvious way.** Pinning
  `sha256(rule-set.json)` into the inventory is **circular**: the inventory is
  itself a declared source of the rule set, so the pin changes the inventory,
  which changes the hash the rule set must declare for it, which changes the
  rule set. The loop never settles. The inventory instead pins
  `rule_set_field_ids` — a digest over the sorted executable field-id surface,
  which is the part it actually depends on, changes exactly when that surface
  drifts, and is stable across `source_set_sha256` rolls. The `status` criterion
  was corrected to match, with the circularity documented in place.

- **D4 skill** — `.codex/skills/ebirforms-validation-rules/` created and it
  passes the repo's own enforced validator (`quick_validate.py`): frontmatter
  exactly `{name, description}`, name equal to the directory, every markdown link
  resolving inside the skill, mandatory `agents/openai.yaml` with a
  `$ebirforms-validation-rules` default prompt. Carries the five guards, the
  eight invariant counts, and the four traps that have already cost sessions,
  plus a `references/boundaries.md` explaining *why* the corpus is fail-closed.
  - `.codex/skills/tests/test_skill_routing.py` extended: its `expected_route`
    table was a closed two-skill world that sent every rules prompt to
    `unresolved`. Rules routing is now decided **before** the print-parity verbs,
    because "Fix the bir-rules source-set digest" would otherwise have matched
    the bare verb "fix" and routed to the renderer skill. A new test pins the
    promotion boundary into the skill text. **31 tests pass**, and both existing
    skills still validate.

- **P0.3 static projector ported** to `crates/bir-rules-codegen/src/projections.rs`
  (`project-2550q`). Idempotency verified independently on a fresh scratch copy:
  **122 files compared, 0 differing**. The corpus was byte-identical to a
  pre-run backup, so no exploratory run ever touched it.
  - **A reported claim was wrong and worth recording**: running the projector
    for real does *not* force a pin roll. Proven by auditing the reformatted
    copy — `audit` exits 0 and `source_set_sha256` is unchanged, because the
    digest is computed over parsed canonical JSON with pin fields nulled. It is
    format-immune. The reformat was then applied to the real corpus: 122 files,
    0 semantic differences, audit green, `roll-pin` reports already consistent.
- **P1.5 reconciliation table** — `build-2550q-bindings` now also emits
  `static-occurrence-reconciliation-v796.json`, generated from the occurrence
  classifications, with all four partitions closing arithmetically and the
  builder failing if they do not. Replaces the four hand-maintained partitions
  previously spread across five documents.
- **P1.6 review document** — `v2-candidate-occurrence-classification-review.md`
  in the established style, recording the classification decision, the
  circularity that forced the field-surface pin, and an explicit
  non-executable boundary.
- **P2.3 `summary.rs`** — the last missing GPUI validation file. Pure
  view-model, no GPUI types, matching the module's stated contract. It
  deliberately does **not** re-sort issues: `ValidationReport::try_new` already
  enforces strictly increasing official order, and sorting by severity would
  send the taxpayer to a different field than the package does. A test pins
  that. It reaches its types through `bir_core`'s re-exports rather than adding
  a `bir_rules` dependency to `bir-desktop`.

### Two guardrails fired on the final run, and both were right

- **`landed_v1_corpus_reconciles_its_published_counts` failed: 659 → 660.**
  Writing the P1.5 reconciliation table into
  `rules/forms/2550q-v2024/fixtures/` had moved the published corpus census.
  The table was **moved out of `rules/`** to
  `docs/validation-rules/generated/` rather than updating the baseline: it is a
  derived report, not evidence (not a declared source, no `$schema`, adds no new
  fact), while `659` is quoted in ten documents including `handoff.md` and other
  historical records. Updating the number would also have taught the next agent
  that a moved count is acceptable — the exact erosion this rule exists to
  prevent. Census restored to 659; the inventory hash is unchanged.
- **`slice_criteria_are_still_open` failed**, exactly as designed — it existed so
  completion had to be acknowledged deliberately rather than drifted into. It is
  replaced by `every_declared_criterion_is_still_evaluated`, which pins all 16
  criterion ids so the condition can no longer be satisfied by *removing* a
  criterion.

**The done-condition passes: 16 of 16 criteria met, suite at 223.** Phase 0 and
Phase 1 are complete. See `## Blocked` for what remains.

## Blocked

- **Five pre-existing test failures in `bir-desktop`**, in
  `components::form_validation::state::tests`, all with
  `Error("unknown field \`rule_id\`, expected \`execution\` or \`order\`")`. A test
  fixture deserializes a `RuleViolation` from JSON in a shape the current type
  no longer accepts. `state.rs` is staged `A ` — untouched by this session's
  work — so these arrived with the uncommitted corpus. **Not fixed
  deliberately**: the correct repair depends on whether the fixture or the type
  encodes the reviewed behavior, and guessing would either weaken a test or
  silently change what a violation means. Needs a decision.
- **Phase 2 is not done** and two of its items are user decisions, not work:
  P2.1 needs an approved production clock/timezone/custody provider for
  `local-current-date`; P2.2 needs independent domain/legal evidence for the
  filing-safe profile and for each of the confirmed official defects. P2.4
  (shadow difference dimensions), P2.6 (builder staging guard) and D5 (status
  /design doc split) remain and are ordinary work.

- **Windows + the Offline eBIRForms package is unavailable/unconfirmed.** Phase
  3 (44 derived calculations, dynamic row order, byte/envelope contract, the
  four additional-item families, `Encrypt.exe`) and everything transitively
  downstream of it — executable artifact nodes, Final Copy, promotion, the
  42-form rollout — cannot be completed here. Each becomes an explicitly named,
  machine-asserted gap instead. This is the plan's documented macOS ceiling, not
  a failure.
- Transport outcomes may be permanently unobservable by policy: `handoff.md:313`
  and `rules/UPDATING.md:40-41` forbid using the official submission path for
  discovery.
