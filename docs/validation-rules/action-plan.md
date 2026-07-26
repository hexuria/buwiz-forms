# Validation rules — action plan and status

> **Superseded for live sequencing (2026-07-26).** This file is retained as
> historical analysis of the earlier 2550Q promotion path. Do not execute its
> “Next: promotion” section. The active objective is the 43-form candidate
> library in [execution-plan.md](execution-plan.md); GPUI, filing, promotion,
> and new worktrees are frozen until that baseline exists. The 53-projection
> classification described here has landed, but no form is thereby reviewed,
> promoted, or complete.

Working tracker. Strategic layer above `execution-plan.md` (which covers the
tooling consolidation) and `handoff.md` (which records the prior session).

Last updated after the seam-opening work. Nine commits on
`codex/print-preview-parity`, nothing pushed.

## Where it stands

| Measure | Value |
| --- | --- |
| Forms with a v2 (executable) rule set | **1 of 43** — 2550Q, `candidate` |
| Forms resolvable at runtime | **0** — reviewed registry is empty |
| Executable validation rules | **27 / 2,007 (1.3%)** |
| Executable calculations | **1 / 623 (0.2%)** |
| Can a draft reach the evaluator? | **Yes** — as of this session |
| Extraction verified against the official package | **No — not possible here** |
| `status` criteria | **16 / 16** |
| Corpus census | 659 JSON, all eight `validate-v1` counts stable |

Provenance is traceable, not checkable: every rule cites a line range in an
`.hta` that is not in the repository, and no official asset is. That does not
change; it just has to be said out loud.

## Done

| ID | Item | Outcome |
| --- | --- | --- |
| **A0** | Commit everything | ✅ 9 commits. Verified from a **real clone**: `audit`, `validate-v1` and `status` all pass. |
| **A1** | Prove the loop end-to-end | ✅ Reached a different way — see below. A draft now produces an `EvaluationRequest`. |
| **A2** | Make the backlog visible | ✅ `coverage` subcommand. Objective, no heuristics. |
| **A5a** | Five failing `bir-desktop` tests | ✅ Fixed, and they were hiding a real defect. |
| — | Tooling consolidation (Phase 0) | ✅ 5 PowerShell scripts → cargo subcommands. |
| — | Clone reproducibility | ✅ 599 files LF-normalised, 12 sources re-pinned, digest rolled. |
| — | 53-projection slice (Phase 1) | ✅ Every occurrence classified; 53 = 44 + 5 + 4. |
| — | Durable instruction coverage | ✅ CLAUDE.md, AGENTS.md, new skill, CI on three OSes. |

### A1 did not need the harness

The planned replay harness could not have worked, and finding that out was the
result. `request_from_capture` refused on **any** capture gap, and four of the
seven repeated families emit a `NoLiveEditorControls` gap unconditionally
because they have no UI. No draft could ever produce an `EvaluationRequest`.

Two blockers were being counted as one: the empty registry, and a seam that was
shut upstream of it.

The fix was not to build UI for four families — that would have meant inventing
unobserved official behaviour. It was that the check was over-strict. Verified
against the rule set rather than assumed: every field group is `min_occurs: 0`
and all 27 rules are `singleton`-scoped, so a family contributing zero rows is a
*complete* capture. `Form2550QCaptureGap::is_blocking()` now separates gaps that
obstruct from gaps that merely inform.

A complete draft now yields **106 raw fields — 66 singletons + 40 materialized
repeated members**, the same 66/40 split the serialization binding inventory
records, reached independently from the application side.

### Two defects found by fixing tests

- **`clear_evaluation_state` never cleared `workflow_transition`.** A taxpayer
  could validate, edit a field, and the view would still report `validated`.
  `handoff.md:171` lists this invalidation as implemented; it was not. The
  broken fixtures had masked it since the `RuleExecution` refactor.
- **The audit did not survive a clone.** 12 declared sources were CRLF with
  hashes pinned to one working tree.

## Next: promotion of 2550Q — and it does not need Windows

This is the correction that reshapes the plan. The 27 executable rules
reference exactly **one** calculation, and it is executable. The audit passes,
so every reference in the snapshot resolves. **The current executable subset is
self-contained.** Windows is needed to *expand* the rule set, not to promote
what already exists.

Promotion requires `review_status: reviewed`, which the audit gates on: the
digest pinned (already true), both profiles `Executable`, and **zero
`"state": "unresolved"` anywhere in the rule set**. There are 135. Note the
audit rejects only `unresolved` — the 6 `documented_only` states (node-less
artifacts, Final Copy/Submit transitions) **do not block promotion**.

The 135 split into three unequal piles.

### P1 — Mechanical: 116 states, no judgement

94 `fields[].behavior.filing_safe` + 22 `verified-correct`
`rules[].profiles.filing_safe`. Filing-safe mirrors official.

Derive and encode with a generator, plus an audit assertion that filing-safe
genuinely mirrors official wherever the v1 assessment is `verified-correct`. No
hand-editing, no invented behaviour, checkable after the fact.

### P2 — Five real decisions

| rule | classification |
| --- | --- |
| `2550q-save-tin` | incorrect-official-behavior |
| `2550q-save-name` | incorrect-official-behavior |
| `2550q-validate-tin` | incorrect-official-behavior |
| `2550q-validate-email` | incorrect-official-behavior |
| `2550q-validate-future-period` | official-bug-compatible |

`rules/shared/official-bugs.md` says filing-safe should "fail closed until
separately reviewed". For a **blank-field check** that is backwards: failing
closed means filing-safe silently stops checking blank TIN, name and email —
worse than official, not safer.

So the choice per rule is:

- **(a) corrected-executable** — filing-safe runs the check properly: TIN
  blankness without the `999999999` bypass, email actually validated.
- **(b) fail closed** — filing-safe refuses to evaluate the rule.

Recommendation: **(a) for the four identity/blank rules, (b) for
future-period**, whose hard-coded date exception nobody should inherit.

**This is the one thing blocking that only you can decide.**

### P3 — Eleven leftovers

4 workflow transitions, 3 artifact filing-safe branches, 1 calculation,
`evaluation_policy`, `profile_status`, 4 `legacy_v1` mappings. Mostly follow
from P1 and P2.

### Promotion gate

`review_status: reviewed`, both profiles executable, zero unresolved, digest
rolled, evidence-only commit. `status` gains a criterion so the registry cannot
be populated without it.

## After promotion

Only then does the engine validate anything: a reviewed snapshot resolves, and
the seam already carries a request to it. Wiring the view to call it is the
remaining GPUI work (`FormValidationState` and `summary.rs` both exist and are
unused).

## Still open, lower priority

| ID | Item |
| --- | --- |
| **A5b** | P2.6 builder staging guard — blocks any multi-form rollout (`UPDATING.md:33-36`) |
| **A5c** | D5 doc split — `architecture.md` is ~45% status log, duplicated in `implementation-plan.md`; already caused the v16/v17 contradiction |
| **P2.4** | Shadow difference dimensions — `shadow.rs` holds only two types |
| — | `.codex/skills/gpui-testing/` is untracked and uninspected; keep or remove |

## Needs Windows + the official package

For **expanding** coverage, not for promoting 2550Q:

- 222 of 623 calculations still phrased as prose (`UPDATING.md:106` forbids
  executing prose)
- dynamic Add/Delete row order versus stable-instance order
- byte-level save / Final Copy contract
- the four additional-item families' modal workflow
- `Encrypt.exe` container behaviour

Transport outcomes may be permanently unobservable by policy —
`handoff.md:313` and `UPDATING.md:40-41` forbid using the official submission
path for discovery.

## Scope — still your call

Now better informed. Promoting 2550Q is macOS-reachable; every *additional*
form needs the Windows trip.

1. **Prove one form properly.** Promote 2550Q, wire it to the view, ship a
   working validator for one form. Research evidence for the other 42.
2. **Filing-volume subset.** Then batch one Windows session for 1601C, 0605,
   2551Q, 1701Q.
3. **Tiered rigor** by filing risk.
4. **All 43 at current rigor** — at 2550Q's demonstrated cost, not reachable.

Recommendation: **1, then reassess.** It converts "built but validates nothing"
into a working validator, and the cost is now measurable rather than open-ended.

## Unchanged

Every production boundary stays closed and is re-asserted by `status` on each
run: reviewed registry empty, 2550Q `candidate`, filing-safe `unresolved`, all
three artifacts `documented_only` and node-less,
`CheckedFinalCopyPayload::try_new` always failing. Nothing above promotes
anything without an explicit, separately reviewed step.
