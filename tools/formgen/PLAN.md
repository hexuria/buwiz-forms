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

Measured 2026-08-07 at r14, worktree `.claude/worktrees/form-correction`,
branch `gol/form-correction`. STATUS.md holds the r14 census and the gate
table; the rows below carry only the per-defect numbers.

**Corpus census — every number carries its denominator.**

| Quantity | Value | Note |
| --- | --- | --- |
| Bundles under `forms/` | 53 | 38 direct + 15 under `forms/extra` |
| Unique form **codes** | 50 | 1701 ships 3 bundles, 1702MX ships 2 |
| Codes on BIR's official list | 44 of BIR's 51 | derived from GOAL.md's 42/48 plus the two landed forms (1604-CF, 2200AN); **not re-verified against bir.gov.ph today** |
| Codes we carry that BIR does not list | 6 | 0620, 1621, 1709, 2000-DST, 2316, 2550-DS — the user asked to keep them |
| BIR codes still missing | 7 | 1600, 1601-E, 1601-F, 1602, 1603, 1704, 2000 |
| Pages | 116 | across 53 bundles |
| Lattice cells | 20,688 (10,002 classified `field`) | r14; 51 cells `field` → `shaded` |
| Emitted inputs | 45,583 | r14; −332. 40,008 comb slot divs, of which **281 carry no input** |
| Comb cells | 4,521 | the pin said 4,540 and was stale at HEAD, not here |
| Findings in `review-findings.json` | 172 | **49 blocker+major open of 116** at r14 (was 58). The 138 immutable baseline entries are untouched; the digest at `gate.py:8713` still matches |

**Gate — full runs r14 (04:22, `8defe23`) and r15 (05:35, `e38672f`), both
clean-tree. 9/12 PASS in both; r15 is authoritative.** STATUS.md holds the full
table and the analysis.

    PASS  self-tests 10 · conversion 53/53 · rules 53/53 · paper 53/53
    PASS  artwork 53/53 · text 53/53 · tracked-files · audit-refresh 53
    PASS  determinism 8ceeab9e506d
    FAIL  assertions    inputs_over_printed_text 20/53 forms (was 40)
                        comb_slots_match_printed 36/53 forms (was 22) — G16
    FAIL  findings      49/116 blocker+major open (was 58)
    UNEV  comb-referee  40/53; at r15 every residual error is G16's shadow

Same three checks red as r13, and no longer for stale reasons. **r14's referee
UNEVALUABLE was a THIRD reviewed emitter pin nobody had counted** —
`HTML_RUNTIME_SCRIPT_SHA256`, read only by the referee, which runs last. It is
re-pinned and **r15 confirms the fix**: all five `form emission binding has
errors` entries are gone. What remains at r15 is `form audit relation contains
errors` on exactly the forms where compartments are now correctly refused, which
fires on `assertion_valid is not True` — **the referee is UNEVALUABLE because
G16 is open, not because of a second defect.**

**That landmine is defused, and a second one of the same shape was found and
defused with it.** `EXPECTED_COMB_SUBJECTS` and `EXPECTED_COMBS` now agree at
**4,521**, which is what the lattice actually produces — both were reading
4,540, and re-running the HEAD (21e0630) lattice over the unchanged IR shows
they had gone stale in 21e0630 itself, not in this session. `guides.py`'s
`("2550m-2007", 3)` expectation and all 53 `EXPECTED_HTML_STRUCTURE_SHA256`
moved at r14 too. See STATUS.md §"Census pins were stale at HEAD, again".

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
| **G01** | Census pins contradict each other, or contradict the producer; a full gate fails on its own constants after 60 minutes | was 4442 vs 4540; **both now 4,521 = measured** (r14) | `gate.py:80`, `comb_referee.py:86` + per-slug, `guides.py` expectation table | **done** | The second instance was worse than the first: both files agreed on 4,540 and both were wrong, because 21e0630 shipped a lattice change without its census. `comb_referee`'s self-test held the same number as a literal and now derives it from the pin. The class is not closed — one number still lives in two files |
| **G02** | Comb compartments merged into one wide input — the user's "4 year boxes as 1 big box" | 22 of 53 forms / 186 offenders (`comb_slots_match_printed`, r13) | `extract.py:382,1571` stroke→rect ignores `lineCap`; `audit.py` topology chooser | diag | STATUS.md; 2550M `p1c2`; gap = ½ supporting rule's stroke width in **every** case in the histogram |
| G02a | 2550M item 1 YYYY: 4 printed compartments, 1 free-text input | 1 cell | same | diag | 0.36pt = round cap at width 0.72 |
| G02b | 2550-DS item 4 `Year Ended (MM/YYYY)`: 6-cell comb → 1 input | F115 | same | open | ledger |
| G02c | 1701MS items 8, 10C: comb → wide input, overflows | F041 | same | open | ledger |
| G02d | 2316 TIN items 3/12/16: 8 inputs for 14 printed comb cells | F111 (blocker) | same | open | ledger |
| G02e | 2200C item 1 date: MM and YYYY groups have no inputs (6 of 8 cells dead) | F097 (blocker) | same | open | ledger |
| G02f | 1800 item 14 centavos: free-text where every other row is 2 comb slots | F073 | same | open | ledger |
| G02g | 0605 items 5, 7, 9: 22 printed compartments → 8 unbounded inputs, TIN included | F163 | same | open | ledger; official ticks counted inside each input rect |
| G02h | 2551M item 2 `Year Ended` → 1 input; Schedule 1 period+name columns merged per row | F164, F165 | same | open | ledger |
| G02i | 2550Q item 10 address line 3 + 10A ZIP → one input; line 2 of the same block is correct | F166 | same | open | ledger |
| **G03** | Real field has **no** input — the user's "no yellow box here" | 160 empty non-fillable `label` cells ≥40×9pt and ≥600pt², 38 of 53 forms (2026-08-07) — the candidate population; 16 open findings | `lattice.py` cell classification | open | F049, F054, F058, F062 (Fiscal checkbox, 4 forms); F135 (2553 Q3); F106 (2200S ×3, blocker); F109 (2200T ×3, blocker); F112 (2316 items 23/24, blocker); F064, F065 (1707 specify lines); **new 2026-08-07** F150 (2551M 23A Surcharge), F151 (1701-conso Sched C+D Description, blocker), F152/F153 (0619E, 0620 Amended-YES checkbox, blocker) |
| G03a | An empty printed box is classified `label`, so no input is ever emitted. A `field` cell with 0 inputs does **not** occur anywhere in the corpus (measured: 0 of 9,971) — this is the whole mechanism | F150, F151 | `lattice.py` | open | ledger |
| **G04** | Input exists where nothing should be fillable — grey spacers made FILLABLE | **169 inputs sit wholly on official grey decoration, 22 of 46 measured forms** (1pt inset, ≥95% tone 150–240, zero black; 7 forms have no saved raster) (2026-08-07); 11 open findings | `lattice.py` field classification vs tone | open | F066 (1707 grey filler = 330×17pt input); F081 (1801 rows 23A-D, 200pt inputs on grey band); F093, F095 (2200A grey "not applicable"); **new 2026-08-07** F154 (1701 449×34pt input over the sworn declaration), F156–F158 (0619E/0619F/0620 header pads, incl. the tax-type pad beside pre-printed "WE"/"WB"), F159 (2552), F160 (2551Q), F161 (1701MS), F162 (1701-attachment) |
| **G05** | Input overlaps pre-printed text | 40 of 53 forms / 258 offenders (r13); got **worse by 19** on the writing-surface fix | `lattice.py` cell segmentation — the rectangle spans caption **and** comb | diag | STATUS.md triage; F134 (2553 "DD" header) |
| **G06** | Lines painted that do not exist on the official sheet | 2 open findings | extract/guides crop — barcode tail | open | F027 (1700 p1), F030 (1701, all 4 pages) |
| **G07** | Text run mis-positioned or reordered | 3 open findings | emit text placement / run ordering | open | F070 (1707A "Calendar" 4pt high); F102 (2200P header 5pt high); F060 (1702Q guide: superscript reordered, corrupts two sentences) |
| **G08** | Guide reflow orphans ATC codes from their industry | F120 | `guides.py` reflow | open | ledger |
| **G09** | Oversized leading comb slot | 29 groups at ≥1.10× median, 17 at ≥1.25× (corpus, 2026-08-06) | `lattice.comb_bands` | open | re-measured this session |
| **G10** | 137 of 138 findings carry `audit_blind: true` — the audit is structurally blind to the field layer | 137/138 (171 of 172 at 2026-08-07) | `audit.py` assertions | open | F028: live inputs over 1700's statutory tax brackets, on a form scoring rules 100% / text 100% / 0 missing / 0 extra |
| **G11** | **A cell the lattice itself marks `mixed` — meaning it knows pre-printed glyph ink is inside — is emitted with a full set of editable comb slots, so the taxpayer can type on a pre-printed constant.** `emit.py`'s `PrePrintedInk` guard (F028's second guard) applies to plain text cells only and has no effect on comb slots | **the defect's own metric is 0**: editable compartments sitting on a short pre-printed constant go **175 → 0** (r14, 2026-08-07). 281 compartments refused across 26 forms. 156 of the 180 `mixed` cells still carry inputs and should — they are money combs whose printed ink is the decimal decoration (C4) | `emit.py` `comb_slot_verdicts()`, per slot | **done** | F139–F146 all `fixed`. Verdict is per COMPARTMENT: a slot is refused when the source printed exactly one alphanumeric glyph **wholly inside that slot's walls**, or shaded it at the unchanged 0.87 threshold. Per-slot is forced by the corpus — 1600-PT prints the century in the *leading* two boxes and 1702EX the branch code in the *trailing* three, so no rule over the group tells the two apart. `II 011`, `XC 010`, `2 0` and `0 0 0 0 0` are no longer typeable; 2000-DST's money grid keeps all 14 compartments including the printed decimal bullet (C4 intact). Rasters in the session scratchpad `preprinted/` |
| **G12** | A caption and the writable blank beside it are segmented into one `label` cell, so the blank gets **no input at all**. Same root cause as G05, opposite symptom — G05 is the case where the merged cell *does* get an input | 2 confirmed (2026-08-07) | `lattice.py` cell segmentation | open | F148 (1701 p4 item 9 "(specify)", `p4c89` 312.90×14.76pt label, 0 inputs), F149 (1701A p2 item 63) |
| **G13** | **A multi-column guide source is reflowed scanline-by-scanline, interleaving the columns and binding values to the wrong key.** On 2551M this puts the wrong tax rate against an ATC code | 2 forms confirmed (2026-08-07); 2551M header row carries 5 source column labels in 3 cells | `guides.py` reflow — no column detection | open | F167 (2551M, **blocker**: PT 060 emitted with Tax Rate 5%, officially 2%), F168/F169/F170 (0605 tax-type table and the two-column Guidelines). **F127 is marked `fixed` and this table is still wrong** — that fix removed prose flattening only, and `reflow_rate_without_description` cannot see a rate bound to the wrong code |
| **G14** | A BIR-only control field is emitted as a taxpayer input | 1 confirmed (2026-08-07) | `lattice.py` field classification | open | F147 (0605 "BCS No./Item No. (To be filled up by the BIR)" = 253.0×17.5pt free text, no maxlength). The exclusion works on the same sheet for DLN/PSIC/PSOC, so this box was missed, not unhandled |
| **G16** | **`audit.py`'s `comb_slots_match_printed` requires a comb's input indexes to run 0..N−1 with no gap, so it fails on every compartment G11 correctly refuses.** The emission contract changed; the assertion that owns it was not told | **76 offenders, 24 forms**, `slot-input-index-mismatch` + `invalid-emission` (r14). The other 167 of the assertion's 247 are the pre-existing `source-topology-unevaluable` population | `audit.py` `check_comb_slots_match_printed` | open | STATUS.md §"The two assertions". **The assertion must not be weakened.** The fix is to re-derive the constant from the SOURCE PDF's own text operators — where this assertion already reads from, so it stays independent of `emit.py` — and accept a gap exactly where the source printed one. This is the "a schema change is declared everywhere it is asserted, in the same commit" rule being paid late |
| **G17** | **A reviewed emitter pin lives in a place no one has enumerated, and only the referee reads it — so it costs a full 60-minute gate run to discover.** `comb_referee.HTML_RUNTIME_SCRIPT_SHA256` is a third such pin, distinct from `EXPECTED_HTML_STRUCTURE_SHA256` and from the four producer SHAs | 5 forms UNEVALUABLE at r14, report partial 40/53; 2 of the pin's 3 hashes had moved | `comb_referee.py:535`, read at `comb_referee.py:2822` | open | The pin itself is re-pinned and **r15 confirms it** (all five emission-binding errors gone). The class stays open because the underlying defect is the enumeration, not this pin: the "census pins that must move together" list under **How we work** did not contain it, and does not name whatever else is like it. An inventory that a producer change can be checked against in seconds — rather than at the end of an hour — is the actual fix |
| **G15** | **The `?debug=fields` overlay shipped in `forms/` is the OLD self-referential one; the fixed overlay exists only in `emit.py` and has never been regenerated into the corpus.** In the shipped legend blue dashed means "this input is fine"; in the fixed one it means "printed box with NO input" — the inverse | was 0 / 38; **now `printed box with no input` → 53 of 53, `no usable box` → 0** (r14, 2026-08-07) | `emit.py` overlay, unregenerated | **done** | F172 `fixed`. Nothing needed fixing — the corrected overlay already existed in `emit.py` and had simply never been written out. Regenerating the corpus at r14 shipped it. The Stage-1 definition of done is no longer blocked on the overlay |

G10 is the one to read twice. It is why Stage 2's central guarantee is not yet
real (see Risk R1). **G11 was fixed first** and is `done`: it was the only row
where a single producer bug put a live text box on a statutory constant. Its
successor is **G16**, which is that fix's unpaid half — the assertion that owns
the emission contract was not told the contract changed.

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
- **Census pins that must move together:** `gate.py:72-80`
  (`EXPECTED_FORMS`, `EXPECTED_IN_CORPUS_FORMS`, `EXPECTED_EXTRA_FORMS`,
  `EXPECTED_COMB_SUBJECTS`), `comb_referee.py` (`EXPECTED_FORMS`,
  `EXPECTED_COMBS`, `EXPECTED_COMBS_BY_SLUG`, `EXPECTED_HTML_STRUCTURE_SHA256`,
  **`HTML_RUNTIME_SCRIPT_SHA256`**, and the four producer SHAs
  `LATTICE_/AUDIT_/EXTRACT_/VERIFY_PRODUCER_SHA256`), and **`guides.py`'s
  per-page expectation table**. The last two were added at r14 after each cost a
  run: `guides.py`'s at self-test time, `HTML_RUNTIME_SCRIPT_SHA256` at the end
  of a 60-minute gate, because only the referee reads it and the referee runs
  last. **This list has been wrong every time it has been consulted — treat it
  as a starting point, not an inventory. That is G17.**
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

## Blocked — needs the user

**CI does not run on this repository any more (2026-08-07).** PR #14 has ZERO
checks. This is not a configuration fault: `gh api .../actions/permissions`
reports `enabled: true, allowed: all`, all four workflows report `state:
active`, and the formgen workflow's `pull_request: branches: [main]` trigger
matches #14. Closing and reopening the PR fired no run either.

The last run repo-wide was 2026-08-06 14:29 — the #13 merge to main. Nothing
since, on any branch or event. Workflows active + permissions open + no runs is
what an exhausted Actions minutes quota looks like, and the private origin is
already recorded as out of minutes; `public` (hexuria) appears to have reached
the same state.

**Consequence, stated rather than dropped:** the third clause of the Stage 1
done-condition — `gh pr checks` every check green — cannot be evaluated. Per
this plan's own rule, a check that cannot be evaluated is a FAILURE, not a pass.
Stage 1 cannot be declared complete while this holds, even if the gate reaches
12/12.

Everything else is local and unblocked: the gate, the defect classes, the
findings ledger, the visual review.

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

- **2026-08-07 (r14)** — **G11 fixed and G15 closed; 332 inputs removed across
  35 of 53 bundles.** `emit.comb_slot_verdicts` decides per COMPARTMENT, never
  per group: a slot is refused when the source printed exactly one alphanumeric
  glyph wholly inside that slot's own walls, or shaded it at the unchanged 0.87
  threshold. 281 compartments refused across 26 forms, spelling only constants —
  `00000` ×42, `20` ×4, `II011`, `XC010`, `VN010`, `WI165`, `039`, `250000` and
  13 grey separator/caption compartments. 2000-DST's money grid keeps all 14
  compartments of every money comb including the printed decimal bullet, so C4
  is intact; 1600-PT's year comb keeps its two YY boxes while refusing the
  century, which is the case that forces per-slot. The other 51 removals are
  `lattice.covering_shading_band` landing: cells sitting on official grey
  "no entry applies" bands, confirmed against the pinned PDF by rasterising
  2200T page 2's Part V header. **F139–F146 and F172 resolved; 58 → 49
  blocker+major open of 116.** Three pin faults found and fixed on the way, all
  of them stale AT HEAD rather than caused here: `EXPECTED_COMBS` /
  `EXPECTED_COMB_SUBJECTS` 4540 → **4521** (21e0630's shaded-paper fix removed
  19 combs without its census — G01 repeating one commit later, and it would
  have failed r14 after 60 minutes); `guides.py` `("2550m-2007", 3)` 1 → 0; and
  all 53 `EXPECTED_HTML_STRUCTURE_SHA256`, which had been stale since GOAL.md
  §Blocked and had been making the comb referee UNEVALUABLE every run. The
  refresh was reviewed, not rubber-stamped: a tag/attribute diff of all 53
  emitted documents against their HEAD selves shows 332 `<input>` deleted, zero
  elements added, and nothing else moved. **New row G16**: the fix's unpaid half
  — `audit.py`'s `comb_slots_match_printed` demands contiguous input indexes, so
  it now reports 76 new offenders for compartments that are correctly empty.
  The assertion was NOT weakened and must not be. **New row G17**: r14's
  referee UNEVALUABLE turned out to be a THIRD reviewed emitter pin,
  `HTML_RUNTIME_SCRIPT_SHA256`, which only the referee reads and which was
  absent from the pins-move-together list. Two of its three hashes moved and
  exactly the two this fix touches — the field runtime and the debug overlay —
  while the band-data runtime is byte-identical. It is re-pinned; **that re-pin
  carries no verdict and the next full gate settles it.**
- **2026-08-07** — Nine-reviewer sweep of all 53 forms against the official PDFs
  consolidated. **34 findings appended, F139–F172**, all `open`; the 138
  immutable entries and the `cause_codes` block were not touched and the pinned
  digest still matches, so the ledger grew in place and no side file was needed.
  Five new defect classes: **G11** (a lattice-`mixed` cell — pre-printed ink
  inside — still gets a full set of editable comb slots: 180/180 such cells,
  175 slots on a short pre-printed constant across 24 forms, including the
  statutory ATC codes `II 011` and `XC 010`), **G12** (caption swallows the
  writable blank → no input), **G13** (multi-column guide reflow interleaves
  columns; **2551M's guide binds a 5% rate to PT 060, officially 2%, on a
  finding already marked `fixed`**), **G14** (a BIR-only box is fillable),
  **G15** (the shipped `?debug=fields` overlay is the old self-referential one
  in all 38 bundles). G03 and G04 got their first measured denominators: 160
  empty non-fillable `label` cells across 38 of 53 forms, and 169 inputs sitting
  wholly on official grey decoration across 22 of 46 measured forms.
  Three instrument errors found and corrected in the consolidation's own tools,
  recorded here because each would have shipped a wrong number: assuming a
  612pt page width inflated "inputs on printed ink" 8× on landscape bundles
  (156 → 19); 12 of the surviving 19 were comb tick-marks, not text (19 → 7);
  and one evidence image was misread as "2552's Amended-Return YES checkbox has
  no input" when `p1c10` and `p1c11` both carry inputs — the old overlay simply
  does not outline every input. Reviewer reports flagging the relocated tax
  tables as "missing" from 1700/1701/1701A/1701Q/1701MS were checked and
  **rejected**: that is F028's fix working, the tables are in `guide.html` with
  0 inputs and no orphan frame remains; only the dangling "refer to tax table
  below" cross-reference survives (F171, minor).
- **2026-08-06** — Plan created at HEAD `0ea1f84`. Three stages recorded in
  ARCHITECTURE.md. Baseline measured: 53 bundles / 50 codes / 116 pages;
  gate r13 9/12; 26/84 blocker+major open. G01 (census pin contradiction
  4442 vs 4540) found and **not yet fixed**. `ddce158`'s referee claim is
  unreproduced on disk.
