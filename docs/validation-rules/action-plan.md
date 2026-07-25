# Validation rules — action plan

Supersedes the tactical `execution-plan.md` at the strategic level. That
document describes how the tooling was consolidated; this one describes what to
do about the fact that the engine still validates nothing.

## Position, stated honestly

| | |
| --- | --- |
| Forms with an **executable** rule set | **1 of 43** (2550Q, `candidate`, test-only) |
| Forms with research-only v1 evidence | 43 |
| Validation rules extracted | 2,007 |
| Calculations extracted | 623 — of which **223 (36%) are English prose** |
| Call sites of the engine in the app | **0** |
| Reviewed rule sets resolvable at runtime | **0** (`REVIEWED_RULE_SET_ENTRIES = vec![]`) |
| Extraction verified against the official package | **No — and not possible here** |

Two facts drive everything below.

**The corpus is self-consistent but its fidelity is unverified.** Every gate
proves the corpus agrees with itself. None proves it agrees with BIR. Provenance
is a line range — `official-hta-runtime#validate:L8407-L8620` — in a file that
is not in this repository. No `.hta`, `BIRForms.exe`, `atcCodes.xml` or
`eBIRTools.vbs` exists in the checkout. The extraction is *traceable*, not
*checkable*.

**Prose is not executable, and 36% of calculations are prose.**
`rules/UPDATING.md:106` forbids executing it. Converting it requires reading the
official HTA, which requires Windows with the package installed. That is the
throughput bottleneck, and it cannot be automated away from macOS.

At 2550Q's demonstrated cost — 19 review documents, 121 fixtures, still not
promoted — 43 forms at this rigor is not reachable. The decision this plan
exists to force is not "what next" but "what rigor, for how many forms".

## A0 — Commit. Risk, not work.

1,049 staged-but-never-committed files plus 142 with new content changes exist
only as a working tree on an external drive that `handoff.md` warns is mounted
by two operating systems. There is no commit anywhere holding this.

Five reviewable commits: prior corpus · tooling · line-endings+pin-roll ·
classification slice · docs/CI/skill. Gates re-run between each so a bisect
lands somewhere meaningful.

**Needs from you:** the first commit is the prior session's corpus, not mine.
Say whether it is mine to make.

## A1 — Prove the loop end-to-end, offline. Highest information per unit effort.

The engine is built, tested and disconnected. Before widening to more forms or
spending a Windows session, establish that the architecture *works on real
input at all*.

**Build a replay harness**: take a real 2550Q draft in the app's raw-capture
format, run it through the candidate evaluator, and print the ordered report.

Deliberately **not** an in-app wiring, because that would require compiling the
candidate module into a non-test build and that is one of the five production
guards. A harness needs no guard change, no promotion, no Windows and no
decision from you.

**What it answers:** does a real draft produce a sane, correctly-ordered report?
Do the 27 rules fire when they should? Does the raw-capture format actually feed
the evaluator? If any answer is no, everything downstream is premature — and we
find out in hours instead of after a Windows trip.

**Verification:** report matches the official first-error order for at least one
constructed failing draft and one clean draft.

## A2 — Make the backlog visible instead of anecdotal.

Nobody can currently see how far from executable the corpus is without ad-hoc
scripting. Add a `coverage` subcommand to `bir-rules-codegen` reporting, per
form: validation rules, calculations, prose calculations, whether a v2 snapshot
exists, and review status.

This converts "we're stuck" into a measurable backlog and makes the cost of any
scope decision explicit before it is taken. Cheap — the data is already in the
corpus.

## A3 — Decide the scope. This is yours, and it gates A4.

Four honest options:

1. **Prove one form properly.** Finish 2550Q through promotion; treat the other
   42 as research evidence only. Smallest, most defensible.
2. **Filing-volume subset.** Executable rules for the handful of forms that
   carry real filing volume (1601C, 0605, 2550Q, 2551Q, 1701Q…); research for
   the rest.
3. **Tiered rigor.** Full evidence chain for high-risk filings; a lighter
   reviewed-adapter path where v1 evidence plus a handwritten adapter is
   proportionate.
4. **All 43 at current rigor.** Honest cost: a Windows observation session and a
   full review cycle per form.

I recommend **2**. It puts a working validator in front of taxpayers for the
forms that matter, without pretending 43 is reachable.

## A4 — One batched Windows session, scoped by A3.

Per form in scope, the trip must produce: HTA function bodies for every prose
calculation; Add/Delete row handler behaviour; byte-level save/final-copy
samples; and the `Encrypt.exe` container behaviour. Batching matters — the
package is installed once and every form's extraction reuses it.

**Prerequisite:** A2's coverage report defines the exact shopping list, so the
session is not exploratory.

## A5 — Hygiene, any time

- **Five failing `bir-desktop` tests.** Now known mechanical: `state.rs:522-539`
  fixtures use the flat `{"rule_id", "order"}` shape; `RuleViolation` requires
  `{"execution": {...}, "order": {...}}`. The type is right — `generate.rs` and
  `audit.rs` already use the new shape. ~30 minutes. Matters because those tests
  cover the stale-result guard in the GPUI seam.
- **P2.6 builder staging guard.** `UPDATING.md:33-36` records builders writing
  directly into the canonical corpus with no staging root and no
  fail-if-target-exists guard. Blocks any multi-form rollout.
- **D5 doc split.** `architecture.md` is ~45% append-only status log duplicated
  in `implementation-plan.md`; that duplication already produced the v16/v17
  contradiction.

## Blocked on decisions, not effort

- **Production clock/timezone/custody provider** for `local-current-date`. None
  approved; not mine to pick.
- **Filing-safe profile and each confirmed official defect.** Needs independent
  domain/legal evidence. This is a tax-correctness judgement.

## What does not change

Every production boundary stays closed and is re-asserted mechanically by
`status` on each run: reviewed registry empty, 2550Q `candidate`, filing-safe
`unresolved`, all three artifacts `documented_only` and node-less, and
`CheckedFinalCopyPayload::try_new` always failing. Nothing in this plan promotes
anything.

## Suggested order

**A0 → A1 → A2 → A3 (you) → A4 (if scope > 1 form).** A5 fits anywhere.

A0 removes the risk of losing everything. A1 tells us whether the architecture
works before more is invested in it. A2 makes A3 an informed decision instead of
a guess.
