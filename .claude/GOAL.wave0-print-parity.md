# Goal: finish Wave 0 of the eBIRForms HTML migration

Bring all ten converted forms through the Wave 0 retrofit checklist so their
boxes, lines, spacing and layout match the official PDFs — then hand back for a
Wave 1 decision.

Repo: `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity`
Branch: `codex/print-preview-parity`. Prefix shell commands with `rtk`.

## Done when

```sh
rtk python3 scripts/wave_status.py            # must exit 0
```

Plus all three of these, with no new failures:

```sh
FORM_VISUAL_NON_PROMOTING_ALLOW_DIRTY_SOURCE=1 npm run test:forms:visual
rtk npm run test:forms
rtk npm run typecheck:forms
```

The visual suite baseline is **99 passed / 10 failed**, where all ten failures
are the known `matches the complete official page(s)` complete-page gates. Any
other failure means the work is not done. A **clipping** failure
(`renders every fixture as stable unclipped pages`) is never acceptable — it
cuts off a taxpayer's data.

## Method

Work **one form at a time**, in filing-cadence order (highest real-world touch
first). A parallel nine-form batch already took the suite from 99 passed to 74,
including clipping on five forms, and had to be reverted wholesale.

1. **1601C, 0605, 0619E, 0619F** — monthly / filed with every payment
2. **2550Q, 1701Q** — quarterly (2551Q is already green)
3. **1701, 1702RT, 1702MX** — annual, last

Per form:

- **See it.** `node scripts/make_section_crops.mjs --form CODE --page N` emits
  side-by-side bands — official on top, ours below — cut at the form's own black
  rules. **Read every band image** before and after fixing. Defects that no
  metric reports are found this way and only this way.
- **Confirm before editing.** Check each candidate against
  `packages/form-specs/geometry-contracts/<stem>.json`. Rules carry `gray`:
  `gray ≈ 0.8509` (tone ≈ 217) is invisible grey decoration — we correctly draw
  nothing there, and painting black over it makes the form visibly wrong while
  *improving* the recall metric. This check rejected 25 of 41 findings in one
  sweep. Your eye finds candidates; the contract decides.
- **Verify after every change** by running that form's spec. Revert anything
  that regresses, immediately.
- **Look again** at the re-cut crops. If the defect is not visibly gone, the fix
  did not work, whatever the numbers say.
- **Commit per form**, recording what was seen in which band, what the contract
  said, and the before/after suite counts.

## Constraints

From `CLAUDE.md`, non-negotiable:

- Never weaken the 1% threshold, any component threshold, or any assertion.
- Never report a masked, structure-only or text-excluded number as parity. The
  complete-page percentage stays computed and reported always.
- Official rasters are calibration-only — never runtime assets or backgrounds.
- Trusted-producer registries stay empty frozensets; `release_ready` and
  capability flags stay false.
- Never truncate a value. Never change a charbox count without official divider
  evidence (a committed pass already corrected capacities — do not undo it).
- No broad `git clean`/`git reset`/worktree pruning. Do not touch the live DB.

## In flight

- Workflow `wf_b824f1b3-ebf` — per-form driver, serial, priority order.
  Resume with `Workflow({scriptPath, resumeFromRunId: "wf_b824f1b3-ebf"})`;
  script at `~/.claude/projects/-Volumes-goldcoders-reverse-engineer-ebir-forms-bir-print-parity/2f33946c-af61-491f-a875-16f498502f50/workflows/scripts/wave0-per-form-driver-wf_b824f1b3-ebf.js`.

## Progress

- Done: chromium references, spec retargeting, geometry contracts (tracked),
  capacity review (23 corrections), static-text manifests, structure diffs.
  Weight corrections committed for 0605, 0619E, 1601C. 2551Q fully green.
- Open: `region_table` on nine forms; `weight_reviewed` on 0619F, 1701, 1701Q,
  1702MX, 1702RT, 2550Q.
- Known defects seen in crops, not yet fixed: 1601C renders `WW010` as envelope
  data although the official PDF pre-prints it (9.96pt bold), so it vanishes
  under blank comparison and no content assertion binds it; 1601C checkboxes are
  visibly larger than official.
- The reverted weight batch is stashed as `wave0-weight-batch-REGRESSED`. Treat
  it as a **hint list only** — re-derive each finding from the contract and
  apply one at a time with a suite run after each.

## Blocked

- Criterion §8.1 cross-machine drift needs a **second machine or CI runner**.
  This blocks *pinning baselines*, not finishing Wave 0 — baselines stay
  unpinned in reporting mode. Surface it when reporting completion.
