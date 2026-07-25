# Goal: complete every remaining actionable task in the action plan

Execute `docs/validation-rules/action-plan.md` to the point where 2550Q is ready
for promotion, stopping only at work that genuinely needs a decision from the
user or a Windows machine.

Repo: `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity`
Branch: `codex/print-preview-parity`. Prefix shell commands with `rtk`.

The Wave 0 print objective is preserved at `.claude/GOAL.wave0-print-parity.md`.

## Done when

```sh
rtk cargo run -q --locked -p bir-rules-codegen -- status
```

Exits 0 only when every boundary still holds **and** every slice criterion is
met. Currently 17 of 19. As deliverables land, add criteria — the condition may
only ever get stricter, and a criterion may never be removed to make it pass.

Plus these, with no regressions:

```sh
rtk cargo run -q --locked -p bir-rules-codegen -- validate-v1
rtk cargo run -q --locked -p bir-rules-codegen -- audit
rtk npm run rules:check
rtk cargo test --locked -p bir-rules -p bir-rules-codegen
rtk cargo test --locked -p bir-core form_2550q
rtk cargo test --locked -p bir-desktop form_validation
rtk cargo fmt --all -- --check
```

Baselines: `validate-v1` reports **43 / 659 (139 v2) / 9,592 / 2,007 / 623 /
1,354 / 216** and those eight never move. Suites: bir-rules + codegen **225**,
bir-core form_2550q **54 + 1 ignored**, bir-desktop form_validation **15**.

## Method

Smallest verifiable unit first; verify each; revert anything that regresses.

- **Reproduce before changing.** Copy inputs to scratch, run with
  `--repo-root <scratch>`. Never let an exploratory run write into `rules/`.
- **`jq -S -c` decides whether evidence changed.** A byte difference with
  semantic equality is formatting, not a regression.
- **After touching a generator source**, run `rules:generate` and confirm the
  manifest diff is confined to `generator.source_sha256`.
- **Any change under `rules/` that alters a declared source needs `roll-pin`**,
  which is a 123-file / 246-site atomic transaction. Never roll by hand.
- **Commit per deliverable**, with the gate output before and after.

## Constraints

- **Never weaken a validator, threshold or assertion to make a gate pass.** If
  `validate-v1`'s eight counts move, the change is wrong.
- **Five switches stay closed**: `form_2550q.rs:42-44` (`None`),
  `generated/mod.rs:4` (`#[cfg(test)]`), `form_2550q.rs:930` (`#[cfg(test)]`),
  `generated/registry.rs:18-19` (empty), `payload.rs:228-234` (always `Err`).
- 2550Q stays `candidate` until an explicit, separately reviewed promotion step.
  **Promotion is not part of this goal** — it needs the five decisions below.
- Preserve official defects in the official profile. Filing-safe may differ only
  with recorded justification.
- Never use the official submission path for discovery, real taxpayer data, or
  the live encrypted database.
- No broad `git clean` / `git reset` / worktree pruning. Do not delete or commit
  `tmp/` or `test-results/` uninspected.

## In flight

- Background gate sweep (`b6izu7fgy`) covering the **builder staging guard**,
  which is written but **not yet committed**. Read its output first on wake and
  confirm every gate before committing — a commit made ahead of its suite
  earlier this session shipped code that did not compile under test.
- Working tree holds: `projections.rs` (`--staging-root` plus a
  fail-if-target-exists guard), `main.rs` wiring, `rules/tools/STAGING.md`, and
  a regenerated manifest. Verified by hand: staging writes 122 files, leaves the
  canonical corpus untouched, and a second run into the same root refuses.

## Progress

Ten commits on this branch, nothing pushed. Verified from a **real clone**:
`audit`, `validate-v1` and `status` all pass.

Complete: tooling consolidated into five cargo subcommands; clone
reproducibility fixed (599 files LF-normalised, 12 sources re-pinned, digest
rolled); the 53-projection slice; durable instruction coverage; the `coverage`
command; the five `bir-desktop` fixtures — which were masking a real
workflow-invalidation defect; and the capture seam, which was shut upstream of
the empty registry and now passes a 106-field request (66 singletons + 40
repeated members, matching the inventory's split from the application side).

**Also done: `shadow-difference-dimensions`.** `ShadowDifferenceKind` separates
issue identity, calculation, profile and serialization coverage;
`has_behavioural_difference()` distinguishes a correctness divergence from
surface the compiled set simply does not model yet. Reports are ordered by axis
then subject so a diff between runs means behaviour changed, not iteration
order. Still purely observational — an empty report permits nothing.

One self-inflicted error worth recording: that commit was made before its suite
finished and shipped a duplicated criterion list that did not compile under
test. Repaired in the next commit. **Wait for the suite before committing.**

**Open (2 criteria):**

1. `builder-staging-guard` — builders write straight into the canonical corpus
   with no staging root and no fail-if-exists guard (`UPDATING.md:33-36`).
   Blocks any multi-form rollout.
3. `filing-safe-mirrors-verified-official` — 94 field branches and 22
   `verified-correct` rule branches are `unresolved`. Mirroring official is the
   null decision where official was reviewed and found correct. Needs a
   generator, a source-pinned review document, an audit assertion that mirroring
   happened **only** where the v1 assessment is `verified-correct`, and a digest
   roll. This is an evidence change — do it as its own commit with a fresh,
   careful pass, not tacked onto something else.

## Blocked

- **Five filing-safe decisions, and only the user can make them**:
  `2550q-save-tin`, `2550q-save-name`, `2550q-validate-tin`,
  `2550q-validate-email` (all `incorrect-official-behavior`) and
  `2550q-validate-future-period` (`official-bug-compatible`).
  `rules/shared/official-bugs.md` says filing-safe should "fail closed", which
  is backwards for a blank-field check — it would stop checking blank TIN, name
  and email, making filing-safe worse than official. Recommendation:
  corrected-executable for the four identity rules, fail-closed for
  future-period. Until these are decided, 2550Q cannot be promoted and the
  reviewed registry stays empty.
- **Expanding beyond 2550Q needs Windows + the official package**: 222 of 623
  calculations are still prose, plus dynamic row order, the byte-level
  artifact contract, the four additional-item families and `Encrypt.exe`.
  Promoting 2550Q itself does **not** need Windows — its 27 rules reference one
  calculation and it is executable.
