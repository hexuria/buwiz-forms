# The three stages

Decided with the user on 2026-08-06, after a day in which two producer fixes
cost six full gate runs. Today the pipeline is ONE stage: every correction has
to be expressed as a change to the generator, so a fact true of one form is
paid for by regenerating all 53, re-pinning four census files, and a 60-minute
gate. Stage 2 breaks that coupling.

    STAGE 1  GENERATE   pinned PDF -> IR -> lattice -> emit -> HTML
    STAGE 2  CORRECT    declared per-form corrections, applied after generation
    STAGE 3  MAP        fields -> eBIRForms XML payload keys

## What belongs in which stage

The dividing line, and it is the whole design:

> **Stage 2 is for facts the source CANNOT tell us. Stage 1 is for us
> misreading a source that is correct.**

A stage-1 bug moved into stage 2 buys speed now and pays forever: 53 forms of
hand-maintained corrections that must be re-verified on every regeneration,
while the underlying bug still ships to every new form.

| Symptom | Stage | Why |
| --- | --- | --- |
| TIN 3 -> 5 branch digits | **2** | The 2007 PDF is correct AND out of date. No rule derives "BIR widened this in 2018" from 2007 artwork. |
| Wrong/missing field boxes | **1** | Traced to producer bugs: collapsed comb heights (4,474 cells), walls not bounding cells (95 cells). One function fixes thousands. |
| Merged compartments (4 year boxes -> 1) | **1** | Ticks ending 0.36pt short are misclassified. Systematic. |
| Lines that should not exist | **1**, pending measurement | Extraction or emission; not yet diagnosed. |
| Grey spacers drawn as real boxes | **1**, pending measurement | Tone classification; `gray = 0.8509` decoration must not become structure. |
| Grey spacers made FILLABLE | **1**, pending measurement | A taxpayer typing into decoration. Closest to the C6 hazard. |

## Rules the user set

1. **A correction never hides a divergence.** Fidelity checks still compare
   against the official PDF and still FAIL on a corrected field -- the report
   says `diverges by declared override <id>, authorised by <authority>`. The
   divergence stays visible forever. An override must never become the way to
   silence an inconvenient check; that is the exact failure this project keeps
   finding.
2. **Fix the generator; override only the residue.** Overrides stay a short
   reviewable list, never a parallel corpus.
3. **Declared and independently verified.** A correction is a data record
   carrying its reason, its authority (regulation / release note), and its
   EXPECTED EFFECT. A verifier re-derives the effect from the corrected output
   and fails if it does not match what was declared. A correction that cannot
   state its effect in advance cannot land.

## Why rule 3 matters here specifically

Every integrity defect found today had the same shape: a checker that shared an
assumption, a code path, or a source of truth with the thing it checked. The
`?debug=fields` overlay compared inputs to their own geometry and reported
233/233 OK on a page the user could see was wrong. A correction system that
verifies itself would be the largest instance of that defect yet built, because
it would sit between the generator and everything downstream.

## Stage 3 readiness (recorded, not started)

`rules/forms/*/fields.json` already carries 43 forms and 9,592 field names
harvested from the official HTA runtime, with `serialized_key` values like
`frm2550m:txtBranchCode`. The naming problem is solved. The blocker is on our
side: emitted inputs are named positionally (`p1c9`) and renumber whenever the
lattice reclassifies a cell -- which happened twice on 2026-08-06 alone. Field
identity must be frozen before mapping starts. Per the user: stage 3 begins
after field geometry settles.
