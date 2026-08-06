# Plan — three stages to a submittable form

Living document. Update the tables below in the same commit as the change that
moves them. Depth lives elsewhere: [ARCHITECTURE.md](ARCHITECTURE.md) (the
stages and the rules), [GOAL.md](GOAL.md) (objective, coverage, constraints),
[STATUS.md](STATUS.md) (all volatile measured numbers),
[README.md](README.md) (the pipeline itself).

**Merging PR #13 lands the pipeline and the corpus so they can be polished in
place. It does not land defect-free forms.** 26 blocker+major findings are open
and two gate checks fail. That is the starting position of this plan, not a
regression against it.

---

## Where we are

Measured 2026-08-06 at HEAD `0ea1f84`, worktree
`.claude/worktrees/pdf-native-extraction`, branch
`gol/pdf-native-form-extraction` (clean, in sync with `public`).

**Corpus census — every number carries its denominator.**

| Quantity | Value | Note |
| --- | --- | --- |
| Bundles under `forms/` | 53 | 38 direct + 15 under `forms/extra` |
| Unique form **codes** | 50 | 1701 ships 3 bundles, 1702MX ships 2 |
| Codes on BIR's official list | 44 of BIR's 51 | derived from GOAL.md's 42/48 plus the two landed forms (1604-CF, 2200AN); **not re-verified against bir.gov.ph today** |
| Codes we carry that BIR does not list | 6 | 0620, 1621, 1709, 2000-DST, 2316, 2550-DS — the user asked to keep them |
| BIR codes still missing | 7 | 1600, 1601-E, 1601-F, 1602, 1603, 1704, 2000 |
| Pages | 116 | across 53 bundles |
| Lattice cells | 20,797 (10,401 classified `field`) | |
| Emitted inputs | 46,076 | 40,012 comb slots + 6,064 plain |
| Comb cells | 4,523 | |
| Findings in `review-findings.json` | 138 | 26 blocker+major **open** of 84 |

**Gate — last full run r13, 2026-08-06 19:38, at commit `d74771e` (4 commits
behind HEAD). 9/12 PASS.** Re-run of `self-tests` only at true HEAD: PASS,
10 modules.

    PASS  self-tests · conversion 53/53 · rules 53/53 · paper 53/53 · artwork 53/53
    PASS  text 53/53 · tracked-files · audit-refresh 53 · determinism 5103254450db
    FAIL  assertions    inputs_over_printed_text 40/53 forms (258 offenders)
                        comb_slots_match_printed 22/53 forms (186 offenders)
    FAIL  findings      26/84 blocker+major open
    UNEV  comb-referee  STALE VERDICT — see below

The comb-referee verdict in r13 is **stale**. `ddce158` re-pinned the reviewed
HTML hashes to the bytes the referee actually reads and its commit message
claims 51/53 forms measured, 0 layout mismatches, 0 disagreements. That claim
is **NOT MEASURED on disk**: `build/comb-referee.json` is from 2026-08-06 00:22
and still carries the pre-`ddce158` corpus. The next full gate run settles it.

**Live landmine, blocks the next full gate:** `gate.py:75`
`EXPECTED_COMB_SUBJECTS = 4442` contradicts `comb_referee.py:86`
`EXPECTED_COMBS = 4540`. One of the two was re-pinned and the other was not.
This is G01 in the defect table and should be fixed before anyone spends 60
minutes on a gate run.

---

## The three stages

    STAGE 1  GENERATE   pinned PDF -> IR -> lattice -> emit -> HTML
    STAGE 2  CORRECT    declared per-form corrections, applied after generation
    STAGE 3  MAP        fields -> eBIRForms XML payload keys

The dividing line, stated once:

> **Stage 2 is for facts the SOURCE cannot tell us. Stage 1 is for us misreading
> a source that is correct.**

A stage-1 bug moved into stage 2 buys speed now and pays forever: 53 bundles of
hand-maintained corrections re-verified on every regeneration, while the bug
still ships to every new form. Of the user's four correction items, **exactly
one is stage 2** (TIN branch-code width). The other three are traced producer
bugs.

---

## Stage 1 — generate

The working surface. One row = one defect class. Edit the row; do not rewrite
the table. `S` = status: `open` / `diag` (diagnosed, unfixed) / `fixing` /
`done`.

| ID | Symptom | Count (denominator, date) | Owning function | S | Evidence |
| --- | --- | --- | --- | --- | --- |
| **G01** | Census pins contradict each other; next full gate fails on its own constants | 4442 vs 4540 (2026-08-06) | `gate.py:75`, `comb_referee.py:86` | open | grep at HEAD `0ea1f84` |
| **G02** | Comb compartments merged into one wide input — the user's "4 year boxes as 1 big box" | 22 of 53 forms / 186 offenders (`comb_slots_match_printed`, r13) | `extract.py:382,1571` stroke→rect ignores `lineCap`; `audit.py` topology chooser | diag | STATUS.md; 2550M `p1c2`; gap = ½ supporting rule's stroke width in **every** case in the histogram |
| G02a | 2550M item 1 YYYY: 4 printed compartments, 1 free-text input | 1 cell | same | diag | 0.36pt = round cap at width 0.72 |
| G02b | 2550-DS item 4 `Year Ended (MM/YYYY)`: 6-cell comb → 1 input | F115 | same | open | ledger |
| G02c | 1701MS items 8, 10C: comb → wide input, overflows | F041 | same | open | ledger |
| G02d | 2316 TIN items 3/12/16: 8 inputs for 14 printed comb cells | F111 (blocker) | same | open | ledger |
| G02e | 2200C item 1 date: MM and YYYY groups have no inputs (6 of 8 cells dead) | F097 (blocker) | same | open | ledger |
| G02f | 1800 item 14 centavos: free-text where every other row is 2 comb slots | F073 | same | open | ledger |
| **G03** | Real field has **no** input — the user's "no yellow box here" | 10 open findings | `lattice.py` cell classification | open | F049, F054, F058, F062 (Fiscal checkbox, 4 forms); F135 (2553 Q3); F106 (2200S ×3, blocker); F109 (2200T ×3, blocker); F112 (2316 items 23/24, blocker); F064, F065 (1707 specify lines) |
| **G04** | Input exists where nothing should be fillable — grey spacers made FILLABLE | 4 open findings | `lattice.py` field classification vs tone | open | F066 (1707 grey filler = 330×17pt input); F081 (1801 rows 23A-D, 200pt inputs on grey band); F093, F095 (2200A grey "not applicable" schedule cells) |
| **G05** | Input overlaps pre-printed text | 40 of 53 forms / 258 offenders (r13); got **worse by 19** on the writing-surface fix | `lattice.py` cell segmentation — the rectangle spans caption **and** comb | diag | STATUS.md triage; F134 (2553 "DD" header) |
| **G06** | Lines painted that do not exist on the official sheet | 2 open findings | extract/guides crop — barcode tail | open | F027 (1700 p1), F030 (1701, all 4 pages) |
| **G07** | Text run mis-positioned or reordered | 3 open findings | emit text placement / run ordering | open | F070 (1707A "Calendar" 4pt high); F102 (2200P header 5pt high); F060 (1702Q guide: superscript reordered, corrupts two sentences) |
| **G08** | Guide reflow orphans ATC codes from their industry | F120 | `guides.py` reflow | open | ledger |
| **G09** | Oversized leading comb slot | 29 groups at ≥1.10× median, 17 at ≥1.25× (corpus, 2026-08-06) | `lattice.comb_bands` | open | re-measured this session |
| **G10** | 137 of 138 findings carry `audit_blind: true` — the audit is structurally blind to the field layer | 137/138 | `audit.py` assertions | open | F028: live inputs over 1700's statutory tax brackets, on a form scoring rules 100% / text 100% / 0 missing / 0 extra |

G10 is the one to read twice. It is why Stage 2's central guarantee is not yet
real (see Risk R1).

**Not measured / not yet diagnosed:**
- How many of the 626 both-endpoints-unsupported borders have a round or
  projecting cap — i.e. how far the `lineCap` fix moves G02. **NOT MEASURED.**
- Why neither failing assertion moved when the painted-wall fix widened 131
  cells and created 95. **NOT DIAGNOSED.** A number that does not move when it
  should deserves the same suspicion as one that moves wrongly.
- Whether `ddce158`'s referee claim reproduces. **NOT MEASURED.**

---

## Stage 2 — correct

Not built. Four binding rules, not open for relitigation (ARCHITECTURE.md
§"Rules the user set"; rule 4 is the design constraint they imply):

1. **A correction never hides a divergence.** The fidelity check still compares
   against the official PDF and still **FAILS** on a corrected field, reporting
   `diverges by declared override <id>, authorised by <authority>`.
2. **Fix the generator; override only the residue** — a short reviewable list,
   never a parallel corpus.
3. **Every correction declares its EXPECTED EFFECT** and a verifier re-derives
   it from the corrected output. A correction that cannot state its effect in
   advance cannot land.
4. **The verifier must not share a producer with the correction.** Re-derive
   from `pdftocairo -svg` or the re-extracted print-to-PDF IR — never from the
   `build/layout/*.json` the correction just mutated. This is the
   `?debug=fields` failure (233/233 OK on a visibly wrong page) and the
   `save()`/`verify()` failure (`3bf32c8`) restated as design.

**Correction record — minimum fields:** `id`, `form`, `subject` (cell/field
identity), `what` (the change), `reason`, `authority` (regulation or release
note, citable), `expected_effect` (machine-checkable), `verified_by` (the
independent producer that re-derives it).

### The register — one entry

| ID | Form(s) | Change | Authority | Expected effect | Status |
| --- | --- | --- | --- | --- | --- |
| C01 | all TIN combs | branch code 3 digits → 5: `000-000-000-000` → `000-000-000-00000` | in-repo: `frm2550m:txtBranchCode` carries `max_length: 5` sourced from `official-hta-runtime#control:L409` | the TIN comb's trailing group emits 5 slots, not 3; total TIN slots 12 → 14 | not built |

Why C01 is genuinely stage 2: the 2007 PDF is correct **and** out of date. No
rule derives "BIR widened this in 2018" from 2007 artwork. Its filing-safety
rationale is independent of the artwork — the HTA runtime the real eBIRForms
client ships declares the width.

Nothing else belongs here yet. Anything proposed for this table must first be
shown *not* to be a stage-1 row above.

---

## Stage 3 — map

**Blocked. Do not start.** Per the user: stage 3 begins after field geometry
settles.

The naming problem is already solved on BIR's side: `rules/forms/*/fields.json`
carries 43 forms and 9,592 field names harvested from the official HTA runtime,
with `serialized_key` values like `frm2550m:txtBranchCode`.

The blocker is ours. Field identity is a **quantised bounding box** —
`lattice.geometry_subject_key` (lattice.py:2699) produces `p<page>@<bbox>` — so
every geometry fix renumbers ids. Measured drift already: 42 of 146 cited cell
ids in the findings ledger no longer exist in the shipped HTML, 9 of them on
OPEN findings, plus 2 dead slugs. The lattice reclassified cells twice on
2026-08-06 alone.

The join is also far from bijective (measured 2026-08-06):

| Gap | Measure |
| --- | --- |
| Bundles with no `fields.json` at all | 13 of 53 |
| Joinable codes with revision skew | 8 |
| Official fields with `serialized_key: null` | 1,234 of 9,592 |
| 0605: names we emit vs official fields | 71 vs 235 |

**Preconditions before stage 3 opens (all must hold):**
1. Field identity is stable across a geometry change — i.e. not derived from
   the bbox alone.
2. Stage-1 rows G02, G03, G04 are `done` (a field that does not exist cannot be
   mapped; a field that should not exist must not be).
3. The findings ledger's cited ids all resolve in the shipped HTML.

---

## How we work

Process rules earned the hard way. Each one cost a day or a 60-minute gate run.

- **Regenerate and commit generated files before a gate run.** A stale generated
  file now fails in 5 seconds (`2bd1c2d`) instead of 50 — but it still fails.
- **One agent per file.** Two agents on `emit.py` once cost a day.
- **A schema change is declared everywhere it is asserted, in the same commit** —
  `gate.py` `BATCH_RECORD_KEYS`, the gate's self-test fixtures, the census pins.
  G01 exists because this was not done.
- **Census pins that must move together:** `gate.py:73-75`
  (`EXPECTED_IN_CORPUS_FORMS`, `EXPECTED_EXTRA_FORMS`, `EXPECTED_COMB_SUBJECTS`),
  `comb_referee.py:85-86` (`EXPECTED_FORMS`, `EXPECTED_COMBS`),
  `EXPECTED_COMBS_BY_SLUG`, `EXPECTED_HTML_STRUCTURE_SHA256`, and the four
  producer SHAs (`LATTICE_/AUDIT_/EXTRACT_/VERIFY_PRODUCER_SHA256`).
- **A check that cannot be evaluated is a FAILURE**, never a pass. UNEVALUABLE
  is a red verdict.
- **Determinism cannot certify a correction applier** — it runs the writer twice
  and both halves drift together (`3bf32c8`).
- **Never edit a check to make it pass.** Never weaken a tolerance
  (position 0.25pt, thickness 0.05pt, advance 0.10pt, size 0.01pt).
- **A finding resolves in the ledger, with evidence, in the same commit as its
  fix.**
- **Any commit that moves a number updates STATUS.md in the same commit.**

---

## Definition of done — as commands, not adjectives

**Stage 1**

```sh
python3 tools/formgen/gate.py                      # exits 0 — all 12 checks, no UNEVALUABLE
python3 -c "import json;d=json.load(open('tools/formgen/review-findings.json'));\
print(sum(1 for f in d['findings'] if f['status']=='open' and f['severity'] in ('blocker','major')))"
                                                   # prints 0
gh pr checks 13                                    # every check green
```
Plus: the user reviews the rendered forms through a **fixed** `?debug=fields`
overlay — one that measures against a producer other than the one that emitted
the boxes.

**Stage 2**

```sh
python3 tools/formgen/gate.py                      # still exits 0 WITH corrections applied
# and the fidelity report names every override:
grep -c 'diverges by declared override' build/audit.json   # equals the correction count
```
A correction whose declared `expected_effect` is not independently re-derived
is a failed correction, not a pending one.

**Stage 3**

```sh
# every emitted input name joins to an official serialized_key, or is listed as
# deliberately unmapped with a reason:
python3 tools/formgen/<mapper>.py --check          # 0 unjoined, 0 unexplained
```

---

## Risk register

Condensed to what changes behaviour.

| ID | Risk | Consequence | Mitigation |
| --- | --- | --- | --- |
| **R1** | **Stage 2's guarantee is close to vacuous today.** The check that is supposed to fail on an override is blind to the field layer: 137/138 findings are `audit_blind: true`; blocker F028 (live inputs over 1700's statutory tax brackets) sat on a form scoring rules 100% / text 100% / 0 missing / 0 extra. | "A correction never hides a divergence" certifies nothing. | Each override must **name the specific check that fails on it and prove it fails**. Close G10 before Stage 2 ships. |
| **R2** | Field identity is a quantised bbox; every geometry fix renumbers ids. | Ledger and mapping both drift silently. 42/146 cited ids already dead. | Freeze identity before Stage 3. Treat a renumbering as a schema change. |
| **R3** | A checker sharing an assumption, code path or source of truth with its subject — 11 instances found so far. | The largest instance would be a self-verifying correction system sitting between the generator and everything downstream. | Rule 4 above: independent producer, always. |
| **R4** | Census pins drift apart (G01, live now). | 60-minute gate run fails on its own constants. | The pins-move-together list under "How we work". |
| **R5** | The comb-referee's 53 reviewed HTML hashes invalidate on **every** legitimate producer change. | Either maximum conservatism or unworkable friction. | Open design question in GOAL.md §Blocked: hash the tag/attribute skeleton, not every byte. **Undecided.** |
| **R6** | Stage-1 fixes that only move a number, not the defect — the painted-wall fix widened 131 cells and moved neither assertion. | Effort spent with no verified effect. | Every fix declares its expected effect too, not only corrections. |
| **R7** | 8+ open findings are TIN-class severity (unenterable Fiscal / Amended / quarter checkboxes, unenterable money boxes). | A form that cannot be filled is as unsubmittable as one filled wrongly. | G03 is not a "minor" row; it is the same class of harm as C01. |

---

## Log

Newest first. One line each.

- **2026-08-06** — Plan created at HEAD `0ea1f84`. Three stages recorded in
  ARCHITECTURE.md. Baseline measured: 53 bundles / 50 codes / 116 pages;
  gate r13 9/12; 26/84 blocker+major open. G01 (census pin contradiction
  4442 vs 4540) found and **not yet fixed**. `ddce158`'s referee claim is
  unreproduced on disk.
