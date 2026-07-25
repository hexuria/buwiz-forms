# Validation-rules execution plan

Complements `handoff.md`; does not replace it. `handoff.md` remains the
authoritative record of what the previous session did.
`docs/validation-rules/implementation-plan.md` remains the phase-level plan
(Phases 0–7). This document states what to do next, in what order, and why.

Status: **Phase 0 in progress.** Live objective and progress are tracked in
`.claude/GOAL.md`; the machine-checkable condition is
`cargo run -p bir-rules-codegen -- status`.

## Verified state

Every numeric claim in `handoff.md` was independently re-counted: **20 of 20
reconcile**. Do not re-derive them.

| gate | result |
| --- | --- |
| `cargo test -p bir-rules -p bir-rules-codegen` | 213 passed (from a 204 baseline) |
| `cargo run -p bir-rules-codegen -- validate-v1` | **exit 0 on macOS**, all eight counts |
| `cargo run -p bir-rules-codegen -- status` | exit 1 — 8 boundaries held, 2 slice criteria open |
| `npm run rules:check` | exit 0, output digest `d1fd97c5…` unchanged |

Production boundaries confirmed closed by direct inspection and re-asserted on
every `status` run: empty reviewed registry
(`crates/bir-rules/src/generated/registry.rs:18-19`), `#[cfg(test)]` candidate
module (`generated/mod.rs:4`), `CheckedFinalCopyPayload::try_new` always
`Err(MissingSerializationContract)` (`form_rules/payload.rs:228-234`),
`reviewed_repo_default_designation()` → `None`
(`form_rules/form_2550q.rs:42-44`), all three serialization artifacts
`DocumentedOnly`/`Unresolved` and node-less, filing-safe `unresolved`.

## Why the tooling moved to Rust

`handoff.md`'s next-slice gate is *"the builder reproduces the tracked value-free
inventory."* That was unsatisfiable, for a tooling reason rather than an
evidence reason:

- The tracked artifacts were written by **Windows PowerShell 5.1**, whose
  `ConvertTo-Json` emits 4-space indent, two spaces after each colon, and nested
  objects aligned under the parent key's column. **No modern tool reproduces
  that** — not pwsh 7, not `serde_json`, not Python, not Node, not `jq`. So a
  one-time canonical reformat was unavoidable regardless of language, which
  freed the language choice.
- `.gitattributes` is `* text=auto eol=lf` while the working tree is CRLF, so the
  five `Get-FileHash` pins in `build-2550q-serialization-bindings.ps1:20-25`
  match the working tree and not the index. A fresh clone breaks the builder on
  every OS.
- Of 72 PowerShell scripts (20,944 lines), only **5 were live** (1,311 lines).
  The other 67 read an extracted installer, `.hta` files, `atcCodes.xml` or
  savefile XML — none tracked — so they cannot run from a clone on any OS. See
  `rules/tools/README.md`.

`bir-rules-codegen` already owned every primitive the port needed: canonical
JSON (`json.rs`), domain-separated SHA-256 (`hash.rs`), symlink-rejecting tree
walks and atomic writes (`files.rs`), and platform-neutral path resolution
(`path.rs`, which rejects `\` outright — this is what eliminates the defect class
structurally rather than patching three instances of it). Its `schema.rs`
validator was verified to support **every keyword the v1 schemas use**, so the
corpus validator needed zero new validation code.

**Canonical JSON for re-emitted evidence:** sorted keys, 2-space indent, exactly
one trailing `\n`, LF, UTF-8 without BOM, temp-file-then-rename. Arrays keep
their order, so occurrence ordinals are unaffected. Sorted keys match
`bir-json-c14n-v1`'s BTreeMap ordering.

## Phase 0 — consolidate live tooling

| id | deliverable | state |
| --- | --- | --- |
| P0.1 | `validate-v1` subcommand | **done** |
| P0.5 | `status` subcommand | **done** |
| P0.2 | `build-2550q-bindings` subcommand | in progress |
| P0.3 | `project-2550q` subcommand, group before static | pending |
| P0.6 | one-time canonical reformat of the three tracked artifacts | pending |
| P0.4 | `roll-pin` — atomic 123-file digest roll | pending |
| P0.7 | `main.rs` / `package.json` wiring | rolling |
| P0.8 | retire `run-full-audit-background.ps1`, add `rules/tools/README.md` | **done** |
| P0.9 | cross-platform CI, run `audit`, extend branch triggers | **done** |
| P0.10 | supersede stale plan files | **done** |

P0.6 is accepted only after `jq -S -c` equality against the current files. The
five `Get-FileHash` pins are then recomputed over LF content once, and
`rule-set.json` joins the pinned input set — the builder reads it today
(`:18`) but does not pin it. `source_set_sha256` is computed over *parsed* JSON
(`audit.rs:5970`, `hash.rs:13-35`) and is EOL-immune, so none of this cascades
into a digest roll.

## Phase 1 — the 53-projection slice

`handoff.md:422-470` (ten steps), `:471-484` (nine-point gate). **Completes
fully on macOS**: `handoff.md:452-454` authorises leaving derived projections
`documented_only` when the missing calculation evidence is named.

- **P1.1** Per-entry `classification` on all 160 occurrence bindings. Today the
  executable/documented-only split exists **only as a set difference** against
  `rule-set.json`, and nothing pins that join, so "the 53" can drift silently.
- **P1.2** Classify all 53 into the six buckets at `handoff.md:432-446`.
  44 derived/alias + 9 workflow/credential/UI, plaintext ordinals 32–159.
  `driveSelectTPExport` is the only non-`text` control.
- **P1.3** Credential guard. `ebirOnlineSecret`, `ebirOnlineUsername`,
  `ebirOnlineConfirmUsername`, `txtEmail` carry a `candidate_v2_field_id` equal
  to their key while
  `v2-candidate-static-surface-projection-review.md:72-76` forbids capturing
  credentials into the tax draft. `status` already asserts they are absent from
  the executable field set; make it explicit in the evidence too.
- **P1.4** Codegen assertions in `generate.rs:871`
  (`landed_2550q_serialization_binding_inventory_is_value_free_and_complete`).
- **P1.5** One generated reconciliation table replacing the four hand-maintained
  partitions of the same 119 static occurrences (119/40, 87/32/28, 66/44/9,
  34/44/9) spread across five documents.
- **P1.6** New source-pinned review document, then one atomic pin roll via P0.4.

## Phase 2 — remaining macOS-completable work

- **P2.1** Clock/timezone provider review for `local-current-date`.
- **P2.2** Filing-safe profile and official-defect review (7
  `incorrect-official-behavior` + 1 `official-bug-compatible`, plus
  `rules/shared/official-bugs.md`). Needs domain input, not runtime observation.
- **P2.3** GPUI seam: create
  `crates/bir-desktop/src/components/form_validation/summary.rs` (the only one
  of four planned files that does not exist); explicit Validate/Final Copy
  actions; normalised raw control events; one `FormAction` pipeline; checked
  bijection across the three key namespaces; `enableAllControl()`'s
  **asymmetric** updates (not a generic enable-all).
- **P2.4** Shadow-report difference dimensions — `form_rules/shadow.rs` is 71
  lines and holds only `EvaluationStamp` and `ShadowEvaluationOutcome`.
- **P2.5** Verify-and-assert only: raw-sync/IO separation is already true
  (`form_2550q_view.rs:820` is pure; `save_draft` is reached only from explicit
  actions). Do not rebuild it.
- **P2.6** Builder staging guard — `rules/UPDATING.md:33-36` records that
  builders write directly into the canonical corpus with no staging root and no
  fail-if-target-exists guard. Un-inventoried elsewhere; blocks the 42-form
  rollout.

### Do not schedule — already done but still listed as deliverables

`implementation-plan.md` declares each of these complete in a Status paragraph
and then re-lists it as an open bullet:

| claim | verified |
| --- | --- |
| Print Preview implicit Save removed (`:368-369` vs `:408-409`) | `form_2550q_view.rs:1709` never calls `save_draft` |
| v1/v2 CI jobs exist (`:532-534` vs `:548-550`) | now superseded by P0.9 |
| V17 pins the full `FormRevisionKey` (`:426-429` vs `:515-516`) | corroborated at `architecture.md:22` |
| Phase 3 "pilot 2550Q" (`:243-244`) | the phase already did it |

`architecture.md` says **v16** where `implementation-plan.md` says **v17** for
the same persistence. Resolve before trusting either.

## The macOS ceiling

Everything above finishes here. The ceiling is **Phase 5's artifact-node step** —
`handoff.md:573-574` forbids modelling artifact nodes before complete occurrence
*and* byte coverage exists.

The boundary is structural: every official asset is pinned by SHA-256 at a
Windows machine-local path and is not copied into the repo. Any task needing a
*new read of official source lines or a new runtime observation* is
Windows-blocked; any task consuming an *already-derived* inventory or quoted
excerpt is not.

Reachable here: the 53 are classified, the GPUI seam is complete, the shadow
reporter works, the clock provider is reviewed, filing-safe is decided, and
every remaining blocker is an explicitly named, machine-asserted gap.

Not reachable: an executable artifact, a reviewed registry entry, Final Copy,
promotion.

## Phase 3 — Windows-gated (batch into one observation session)

**W1** the 44 derived calculations and their scopes (`calculations.json` holds
prose formulas; `UPDATING.md:106` forbids executing prose; also needs a decision
on emulating `Math.floor(value*100+0.50000000001)`). **W2** stable-instance
order versus official live DOM/display order — static control order is already
captured in `runtime-control-inventory-v796.json`, the dynamic Add/Delete order
is not. **W3** separator, newline, encoding, non-ASCII, marker, omission,
filename, overwrite and custody behavior. **W4** the four unbounded
additional-item families (Items 19/42/47/56). **W5** `Encrypt.exe`.

Transitively blocked: executable artifact nodes → checked plaintext at Final
Copy → encryption/container binding → queue/transport authorization. Transport
outcomes may be **permanently unobservable by policy** (`handoff.md:313`,
`UPDATING.md:40-41`).

## Phase 4 — promotion and rollout

Snapshot promotion in a dedicated evidence-only commit; Phase 6 remainder
(reviewed default selector, draft migration preview, reopen-time resolution,
migration-diff UI, executable historical snapshot); then additive v2 snapshots
for the remaining 42 forms in `FORM_BUILD_PRIORITY.md` order — each needing its
own Windows extraction, and each blocked on P2.6.

## Process and documentation

- **D1** The validation-rules objective has **zero durable instruction
  coverage**. No mention of `bir-rules`, `rules/` or `form_rules` in
  `CLAUDE.md`, `AGENTS.md` or `.codex/skills/`. "The 2550Q candidate is
  test-only, never promote" exists in no instruction file.
- **D2** `CLAUDE.md` and `AGENTS.md` are ~95% duplicates with five divergences,
  including `AGENTS.md` contradicting itself on criterion status (`:22-23` vs
  `:127`) and `CLAUDE.md:101` falsely claiming `AGENTS.md` is legacy-only.
- **D3** `.claude/GOAL.md` held the Wave 0 print objective; it is now the
  validation-rules objective and the print one is preserved at
  `.claude/GOAL.wave0-print-parity.md`. `/goal` still supports only one active
  objective at a time.
- **D4** New `.codex/skills/ebirforms-validation-rules/` matching the contract
  enforced by `quick_validate.py`; extend
  `.codex/skills/tests/test_skill_routing.py`, whose closed two-skill route
  table sends rules prompts to `unresolved` with no test failing.
- **D5** Status/design split for `architecture.md` (~45% is an append-only
  status log at `:56-207`) duplicated in `implementation-plan.md`. The `status`
  command replaces the prose.

## Constraints

Unchanged from `handoff.md`, `CLAUDE.md` and `AGENTS.md`, and re-asserted
mechanically by `status`:

- **Never weaken a validator, threshold or assertion to make a gate pass.** If
  `validate-v1` changes any of the eight counts, the change is wrong.
- Reviewed registry stays empty; 2550Q stays `candidate`; filing-safe stays
  `unresolved`; all three artifacts stay `documented_only` and node-less.
- No Final Copy, queue, transport, capability-flag, `release_ready` or
  print-threshold change.
- Preserve official defects in the official profile; safer behavior is a
  separately evidenced filing-safe decision.
- A digest roll is a 123-file atomic transaction; partial rolls must fail.
- No broad `git clean` / `git reset` / worktree pruning. Do not delete or commit
  `tmp/` or `test-results/` uninspected.
