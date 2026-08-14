# The three stages

Decided with the user on 2026-08-06, after a day in which two producer fixes
cost six full gate runs. Today the pipeline is ONE stage: every correction has
to be expressed as a change to the generator, so a fact true of one form is
paid for by regenerating all 53, re-pinning four census files, and a 60-minute
gate. Stage 2 breaks that coupling.

    STAGE 1  GENERATE   pinned PDF -> IR -> lattice -> emit -> HTML   (forms/, batch-versioned)
    STAGE 2  CORRECT    forms-corrected/ = copy(batch vN) + declared correction records
    STAGE 3  MAP        fields -> eBIRForms XML payload keys, bound to forms-corrected/

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

## Batch-versioned immutability (reconciled with the user, 2026-08-08)

The user's instinct: stage-1 artifacts that pass are not to be churned; fixes
produce ANOTHER batch; a form needing no correction is copied as-is into the
corrected tree. The counter-check that amended one clause of it, agreed after
review:

- **"Passing" was hollow until the field-layer assertions existed.** 137 of 138
  visual-review defects sat on pages the numeric audit scored 100%. Freezing
  the first "passing" batch would have frozen unusable 2.6pt comb fonts,
  typeable statutory ATC codes, and an amended-return checkbox that could not
  be ticked. A batch is only worth freezing after a SIGHTED gate has scored it.
- **A source-misreading fix belongs in the generator, never in a correction
  record** (the stage-1/stage-2 dividing line above). Routing tone/clip/cap
  misreadings through stage 2 would have produced 53 hand-patched forms and a
  generator that repeats the mistake on form 54.
- **Frozen artifacts are never re-measured.** The r22 regressions were not
  caused by regeneration -- they were CAUGHT by it, on the run that introduced
  them. Never-regenerate would have prevented their detection, not their
  existence.

What survives of the instinct, as binding rules:

1. **A batch is immutable once published.** Every gate-verdict commit is a
   batch version (the git history is the ledger; tag `corpus/rN` at each
   scored gate run). A generator fix never mutates a published batch in place;
   it produces the NEXT batch, and the reviewable artifact is the v(N) ->
   v(N+1) diff -- which is exactly what the determinism check and the
   tracked-file guard already enforce commit-by-commit.
2. **One generator version per batch.** A corpus mixing producer versions has
   no provenance; determinism and the audit's application-scope binding both
   assume (and verify) this. This is why "do not regenerate passing forms"
   cannot hold form-by-form, only batch-by-batch.
3. **Stage 2 consumes a named batch.** `forms-corrected/` is built by a small
   applier: for each form, byte-copy from the named stage-1 batch, then apply
   that form's correction records, if any. No record -> byte-identical copy
   (the user's rule). The applier's manifest names the source batch, every
   record applied, and the sha256 of input and output -- so "what changed and
   under whose authority" is one file, not an investigation.
4. **The gate runs on BOTH trees.** Stage-1 checks are unchanged. On
   forms-corrected/, fidelity must fail ONLY at the declared divergences, each
   named per rule 1 above; an undeclared diff between the trees is a build
   failure, not a shrug. Stage 3 binds to forms-corrected/ and to nothing
   else, so the mapping moves only when a correction record moves.

Current stage-2 ledger: exactly ONE true correction is known -- the TIN branch
code 3 -> 5 on 2550M (see the table above). The applier gets built when stage 1
closes, against this section.

## Rule 4, wired at r27 — the checkable half

`gate.py` gained a `corrected-tree` check. It is the minimum honest version and
its branches are stated here so that a later widening is a visible change:

| State | Verdict |
| --- | --- |
| `forms-corrected/` absent | **PASS** — stage 2 is unbuilt; nothing downstream reads a tree that is not there |
| present, no manifest | **FAIL** — bytes nobody can re-derive from a named batch |
| a manifest `correct.py --verify` cannot re-derive | **FAIL** — the check demands that verification rather than re-implementing it, so the second opinion does not share this file's assumptions |
| verified, no divergence declared | **PASS** |
| a declared divergence, no fidelity report | **FAIL** |
| a declared divergence the report does not name | **FAIL** — this is rule 1's silent override, and it is the branch that must never go green |

All seven branches are fixtures in `gate.self_test`. The four interesting ones
were also proven end to end at r27 against a real corrected tree built from
`corpus/r27`: no report FAILs; a report naming the sentence PASSes; a report
that paraphrases it into "the tree matches the official form" FAILs; and one
byte edited by hand after the applier ran FAILs, naming the file. That tree was
removed afterwards — this round does not land stage 2.

One defect the end-to-end proof caught that the fixtures had not: the divergence
sentence `build_manifest` generates contains double quotes, so a raw substring
search against a JSON report reported a sentence as absent from a report that
plainly contained it. The check now decodes a JSON report's strings before
looking, and falls back to raw text otherwise, so it does not depend on a report
format that has not been decided.

**What is still missing, precisely.** There is no fidelity run over
`forms-corrected/`, so `build/corrected-fidelity.json` does not exist. Until it
does, declaring any correction turns the gate red — the fail-closed direction,
and the reason this check could land before that report. The gate also does not
yet run the stage-1 checks a second time over the corrected tree; that is the
other half of rule 4 and is not wired.

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
