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

Measured 2026-08-08 at r27, worktree `.claude/worktrees/form-correction`,
branch `gol/form-correction`, corpus tagged `corpus/r27`. STATUS.md holds the
r27 census, the four user-visible screenshot checks and the gate table; the
rows below carry only the per-defect numbers.

**r27 is the first round to move `inputs_over_printed_text` without hiding it.**

| Assertion | r20 | r22 | r23/r25 | r27 | |
| --- | --- | --- | --- | --- | --- |
| `inputs_over_printed_text` | 20 / 149 | 19 / 131 | 20 / 147 | **12 / 33** | −8 forms, −114 offenders |
| `comb_slots_match_printed` | 22 / 188 | 36 / 254 | 22 / 193 | **23 / 203** | **WORSE by 1 form / 10** |
| `inputs_span_no_printed_divider` | 11 / 67 | 5 / 33 | 5 / 33 | **5 / 33** | unmoved, offender-for-offender |
| `money_boxes_have_inputs` | 0 / 0 | 4 / 4 | 0 / 0 | **0 / 0** | PASSES |
| `printed_box_peers_all_fillable` | 0 / 0 | 1 / 1 | 0 / 0 | **0 / 0** | PASSES |

r22 "improved" the first row by shrinking every comb's writing surface to a
3.12pt stub. **r27 did not**, and the proof is in the bytes rather than in the
count: all **7,405** slot rectangles surviving in the ten changed documents were
compared attribute-string to attribute-string against their r26 selves and
**zero moved**; the only rectangles removed are the 22 belonging to the eleven
refuted caption blocks. 2550M item 4's TIN is still 14.16pt in a 15.60pt row and
2316 item 3 is still 14 boxes — both documents byte-identical.

**The row that got worse is G16, and it is this fix's own measurement cost.**
Refusing 94 occupied compartments makes `comb_slots_match_printed` publish
`invalid-emission` on exactly 94 cells, because it pairs the k-th input with the
k-th compartment while `data-slot-index` already carries the compartment's true
number. 76 of the 94 were already offenders; the 18 that were not are 2200S ×16,
1800 `p1c68` and 2550-DS `p1c79`. Filed as **F192**. The assertion must not be
weakened: what it cost to leave those compartments alone was 89 taxpayer typing
surfaces laid on printed ink.

**Two new open findings and one new open class come out of r27**: F190 (a live
input over the printed caption "27 Tax Debit Memo" in the first compartment of a
29-box money comb, on 2200A/2200C/2200P) and F191 (the referee's retained-subject
contract does not know a third retained shape, so 3 forms produce no report).
Neither was patched here — F191 in particular is the adjudicator, and changing it
in the same increment as the producer it adjudicates is the failure this project
has already paid for twice.

**r23 started nothing. It paid r21/r22's three regressed assertion families,
and one of them got worse in the paying.**

| Assertion | r20 | r22 | r23 | |
| --- | --- | --- | --- | --- |
| `comb_slots_match_printed` | 22 / 188 | 36 / 254 | **22 / 193** | forms back to r20 |
| `money_boxes_have_inputs` | 0 / 0 | 4 / 4 | **0 / 0** | PASSES |
| `printed_box_peers_all_fillable` | 0 / 0 | 1 / 1 | **0 / 0** | PASSES |
| `inputs_span_no_printed_divider` | 11 / 67 | 5 / 33 | **5 / 33** | unmoved |
| `inputs_over_printed_text` | 20 / 149 | 19 / 131 | **20 / 147** | **WORSE by 1 form / 16** |

The one that got worse is reported first in STATUS.md and is **G05**, not a new
class: r22 had not fixed those 21 offenders, it had *hidden* them by shrinking
every comb's typing surface to a 3.12pt divider band that is too short to reach
a caption and too short to type in. Restoring the writing box (F186) restores
the debt with it.

The two that went green went green because the **corpus** changed, not because
a check did: `emit.comb_writing_rect` lays every rectangle the emitter draws
out on the writing box, and `audit.py`'s source-occupancy query — unchanged —
stopped being asked about a 3pt strip where the printed constant is not. That
alone took the `invalid-emission` population 64 offenders / 25 forms → 3 / 3.
The one exclusion added, `audit.source_bureau_reservations`, is derived from
the pinned PDF's own text operators, claims **exactly one box corpus-wide**
(0605 `p1c17`, blocker F147's), and publishes its own count.

**Gate — full clean-tree run r23 (2026-08-08 00:49, `912c6ed`). 9/12 PASS, the
same three checks red as r22, and TWO assertions fewer inside the red one.**

    PASS  self-tests 10 · conversion 53/53 · rules 53/53 · paper 53/53
    PASS  artwork 53/53 · text 53/53 · tracked-files · audit-refresh 53
    PASS  determinism ba1bd2d8c47e  (moved, and had to: all 53 form documents
                                     changed. Two generations still compare
                                     byte-for-byte)
    FAIL  assertions    inputs_over_printed_text        20/53  (r22: 19)
                        comb_slots_match_printed        22/53  (r22: 36)
                        inputs_span_no_printed_divider   5/53  (r22: 5)
                        money_boxes_have_inputs          GONE  (r22: 4)
                        printed_box_peers_all_fillable   GONE  (r22: 1)
    FAIL  findings      32/129 blocker+major open (r22: 33/129)
    UNEV  comb-referee  2550M p1c89/p1c90 — character-for-character r22's

**The referee is UNEVALUABLE for exactly r22's reason and r23 did not touch
it.** `p1c89`/`p1c90` are F184's cells; `lattice.py` is byte-identical this
round and the referee's derivation was not edited. It is the one thing between
9/12 and 10/12, it is r22's debt, and closing it is the reviewed
`retired_proven_false` transition F184 already names — which needs independent
evidence and a human, not an integration-time edit to the adjudicator.

**One of the four red assertion rows is green, honestly.**
`printed_box_peers_all_fillable` goes 14 offenders on 14 of 53 forms to 0 on 0
with `audit.py` byte-identical throughout. `inputs_span_no_printed_divider`
falls 79 → 67 offenders on the same 11 forms.
`comb_slots_match_printed` got **worse by 3** (185 → 188) and that is reported
in full in STATUS.md rather than netted off: two genuine position mismatches
were fixed, and five money-comb cells on 2550M now claim a compartment the
sheet does not print, which is the new finding F184.

**Corpus census — every number carries its denominator.**

| Quantity | Value | Note |
| --- | --- | --- |
| Bundles under `forms/` | 53 | 38 direct + 15 under `forms/extra` |
| Unique form **codes** | 50 | 1701 ships 3 bundles, 1702MX ships 2 |
| Codes on BIR's official list | 44 of BIR's 51 | derived from GOAL.md's 42/48 plus the two landed forms (1604-CF, 2200AN); **not re-verified against bir.gov.ph today** |
| Codes we carry that BIR does not list | 6 | 0620, 1621, 1709, 2000-DST, 2316, 2550-DS — the user asked to keep them |
| BIR codes still missing | 7 | 1600, 1601-E, 1601-F, 1602, 1603, 1704, 2000 |
| Pages | 116 | across 53 bundles |
| Lattice cells | **20,704** (10,050 classified `field`) | r20: +16 cells, +48 `field` — the two `lattice.py` fixes and the cap model |
| Emitted inputs | **45,643** | r20: **+60 and nothing deleted**. 40,017 comb slot divs, of which **281 carry no input** (unmoved) |
| Comb ledger subjects | **4,543** | the `EXPECTED_COMBS` denominator (active + `retained_unresolved`). r20: 4,543 subjects, **4,522 active**, **21 retained**, moving on six slugs. `gate.EXPECTED_COMB_SUBJECTS` moves with it 4,521 → 4,543 — it was ALREADY WRONG at HEAD, because r19 moved the referee's twin and not it |
| Form documents changed at r20 | **25 of 53** | plus `forms/index.html`; **0 guide documents**. Tag inventory across them: +60 `<input>`, +29 `<div>`, +3 `<rect>`, zero deletions, visible text token-identical |
| Gate-demanded assertions | **10** | unchanged at r20 |
| Findings in `review-findings.json` | **185** | **42 blocker+major open of 128** at r20 (was 55 of 126): 15 closed on measurement (F049, F054, F058, F062, F106, F135, F150, F152, F153, F173–F177, F180), **F184 filed open** (2550M money combs claim a compartment the paper does not print) and **F185 filed open** (11 comb-spanning inputs the cap model made visible). The 138 immutable baseline entries are untouched; the digest at `gate.py:8752` still matches, re-verified at r20 |

**Gate — full clean-tree run r20 (2026-08-07 18:27, `73c3ce4`). 9/12 PASS, the
same three checks red as r19, and one assertion fewer inside the red one.**
STATUS.md holds the full table and the two self-inflicted faults this run
found.

    PASS  self-tests 10 · conversion 53/53 · rules 53/53 · paper 53/53
    PASS  artwork 53/53 · text 53/53 · tracked-files · audit-refresh 53
    PASS  determinism b5e4f9e1b979  (moved from 7a152bc88161, and had to: 25
                                     form documents changed. Two generations
                                     still compare byte-for-byte)
    FAIL  assertions    inputs_over_printed_text 20/53        (r19: 20, unmoved)
                        comb_slots_match_printed 22/53        (r19: 22, unmoved)
                        inputs_span_no_printed_divider 11/53  (r19: 11, unmoved)
                        printed_box_peers_all_fillable        GONE (r19: 14/53)
    FAIL  findings      42/128 blocker+major open (r19: 55/126)
    UNEV  comb-referee  52/53 forms, 2551Q the only error, identical to r19

Confirmed by a second full clean-tree run at `e7416c8` (19:20) after the two
self-inflicted faults were fixed: same 9 of 12, same three red, `findings` now
FAIL 42/128 rather than UNEVALUABLE, `comb-referee` back to 52/53, and the
determinism digest character-for-character the 18:27 value, so the corpus did
not move between the two runs.

*(r19's block, superseded: determinism `7a152bc88161`; assertions 20/22/11/14;
findings 55/126; comb-referee 52/53.)*

**Three of the four assertion populations did not move by a single form, and
the fourth emptied.** The two pre-existing rows are unmoved form-for-form,
which is what makes "the checkbox class is fixed and nothing else regressed" a
measurement rather than a hope. The one number that did move the wrong way,
`comb_slots_match_printed`'s offender count, is reported in full above and in
STATUS.md, and is now finding F184 / row G19.

**The two new red rows are the point, not a regression.** Every one of their 93
offenders was already in the shipped corpus at r14; what changed is that a check
can see them. Neither pre-existing assertion moved by a single form, and the
determinism digest is character-for-character the r14/r15 value, so the corpus
under measurement is provably unchanged.

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
| **G05** | Input overlaps pre-printed text | was 40 of 53 forms / 258 (r13), 20 / 147 (r23-r25); **12 of 53 forms / 33 offenders (r27)** | `lattice.py` cell segmentation — the rectangle spans caption **and** comb; `emit.slot_constant`; `audit.glyph_boxes` | **fixing** | r27 removed three populations at once and STATUS.md carries the split: 92 money decimal-bullet compartments that were live typing surfaces on printed ink (**F189**), 11 printed caption blocks read as 2-compartment combs — 1606 p2's whole statutory rate table and five excise mastheads (**F188**) — and the false positives, where `glyph_boxes` scored an input against the font's LINE box so every glyph was charged with its face's full descender. **Not a shrunk writing surface**: 7,405 surviving slot rectangles compared byte-for-byte, zero moved. What is left is 33 offenders on 12 forms, F134 among them |
| **G06** | Lines painted that do not exist on the official sheet | 2 open findings | extract/guides crop — barcode tail | open | F027 (1700 p1), F030 (1701, all 4 pages) |
| **G07** | Text run mis-positioned or reordered | 3 open findings | emit text placement / run ordering | open | F070 (1707A "Calendar" 4pt high); F102 (2200P header 5pt high); F060 (1702Q guide: superscript reordered, corrupts two sentences) |
| **G08** | Guide reflow orphans ATC codes from their industry | F120 | `guides.py` reflow | open | ledger |
| **G09** | Oversized leading comb slot | 29 groups at ≥1.10× median, 17 at ≥1.25× (corpus, 2026-08-06) | `lattice.comb_bands` | open | re-measured this session |
| **G10** | 137 of 138 findings carry `audit_blind: true` — the audit is structurally blind to the field layer | was 171 of 172; **the first two field-layer assertions landed at r18** and catch **93 offenders across 22 of 53 forms** — `inputs_span_no_printed_divider` 11 forms / 79 offenders (44,536 inputs walked), `printed_box_peers_all_fillable` 14 forms / 14 offenders (7,223 printed boxes recovered from the source). 9 of the 93 were on populations no open finding covered (F173–F181); the rest independently re-derive **16 existing human findings** from the pinned PDF alone | `audit.py` assertions; `gate.py` allowlists | **fixing** — the first of the two now PASSES (r20: `printed_box_peers_all_fillable` 14 forms → 0, `audit.py` byte-identical) | 0619-E's A10 offender box `[276.05, 134.64, 289.08, 146.16]` is F152's `(276.0, 135.0) 12.5 x 10.5` to the point; 2550M's A9 offender `p1c2 [209.28, 90.72, 270.00, 102.48]` is G02a, hand-diagnosed on 2026-08-06 and invisible to every check until now. Neither assertion reads `b.layout`, `b.plan`, emit.py's markers or the IR — only `ordered_vector_paints` and `drawn_glyph_boxes` — which is why they can see what `money_boxes_have_inputs` and `comb_slots_match_printed` structurally cannot. **Not `done`:** these bound two field-layer questions (a box the source drew but nobody made fillable; an input laid across a divider the source printed). "Does an input match its printed box" and "is a printed constant overtypeable" are still unbound |
| **G11** | **A cell the lattice itself marks `mixed` — meaning it knows pre-printed glyph ink is inside — is emitted with a full set of editable comb slots, so the taxpayer can type on a pre-printed constant.** `emit.py`'s `PrePrintedInk` guard (F028's second guard) applies to plain text cells only and has no effect on comb slots | **the defect's own metric is 0**: editable compartments sitting on a short pre-printed constant go **175 → 0** (r14, 2026-08-07). 281 compartments refused across 26 forms. 156 of the 180 `mixed` cells still carry inputs and should — they are money combs whose printed ink is the decimal decoration (C4) | `emit.py` `comb_slot_verdicts()`, per slot | **done** | F139–F146 all `fixed`. Verdict is per COMPARTMENT: a slot is refused when the source printed exactly one alphanumeric glyph **wholly inside that slot's walls**, or shaded it at the unchanged 0.87 threshold. Per-slot is forced by the corpus — 1600-PT prints the century in the *leading* two boxes and 1702EX the branch code in the *trailing* three, so no rule over the group tells the two apart. `II 011`, `XC 010`, `2 0` and `0 0 0 0 0` are no longer typeable; 2000-DST's money grid keeps all 14 compartments including the printed decimal bullet (C4 intact). Rasters in the session scratchpad `preprinted/` |
| **G12** | A caption and the writable blank beside it are segmented into one `label` cell, so the blank gets **no input at all**. Same root cause as G05, opposite symptom — G05 is the case where the merged cell *does* get an input | 2 confirmed (2026-08-07) | `lattice.py` cell segmentation | open | F148 (1701 p4 item 9 "(specify)", `p4c89` 312.90×14.76pt label, 0 inputs), F149 (1701A p2 item 63) |
| **G13** | **A multi-column guide source is reflowed scanline-by-scanline, interleaving the columns and binding values to the wrong key.** On 2551M this puts the wrong tax rate against an ATC code | **2551M: 0 of 15 ATC codes carried their official rate → 15 of 15** (r19, measured on the written tree by a checker sharing no producer with the emitter). 3 guide bundles changed, 0605 and 2200-AN with it | **`emit.py`** `reflow_page` → `_column_bands` → `_table_markup`, using the new `guides.table_columns` — **NOT `guides.py`'s reflow, which is what this row, STATUS.md and F167 all said, and is why the fix failed to land twice**. `BLOCKER-PLAN.md` C9 named `emit.py` and was right | **done for the rate binding; F170 and F183 remain** | F127, F167, F168, F169 all `fixed` at r19 on the measurement above; **F170 stays open** (0605's ATC region is still cut into two tables, so its 3-line header section cannot reach `MIN_COLUMN_SUPPORT`) and **F183 is newly filed** (2551M's left `Tax Rate` label is set at x 237.60 against its own column edge of 251.52, so the label — not any rate — falls one cell left). The old grid came from `_coverage_gutters`, which calls a 1pt bin a gutter below 12% of peak; on 2551M p2 the real gutter sits at 4–5 runs against a peak of 18, so all four missing boundaries were bins the histogram called occupied. `guides.table_columns` asks where a *cell starts* instead and keeps a column only where two lines agree |
| **G14** | A BIR-only control field is emitted as a taxpayer input | 1 confirmed (2026-08-07) | `lattice.py` field classification | open | F147 (0605 "BCS No./Item No. (To be filled up by the BIR)" = 253.0×17.5pt free text, no maxlength). The exclusion works on the same sheet for DLN/PSIC/PSOC, so this box was missed, not unhandled |
| **G16** | **`audit.py`'s `comb_slots_match_printed` requires a comb's input indexes to run 0..N−1 with no gap, so it fails on every compartment G11 correctly refuses.** The emission contract changed; the assertion that owns it was not told | **r27: 94 more cells, one per compartment the emitter newly refuses** — 76 already offenders, 18 not, taking the assertion 22 forms / 193 → 23 / 203 and filed as **F192**. Was 76 offenders, 24 forms (r14). The other 167 of the assertion's 247 are the pre-existing `source-topology-unevaluable` population | `audit.py` `check_comb_slots_match_printed` | open | STATUS.md §"The two assertions". **The assertion must not be weakened.** The fix is to re-derive the constant from the SOURCE PDF's own text operators — where this assertion already reads from, so it stays independent of `emit.py` — and accept a gap exactly where the source printed one. This is the "a schema change is declared everywhere it is asserted, in the same commit" rule being paid late |
| **G17** | **A reviewed emitter pin lives in a place no one has enumerated, and only the referee reads it — so it costs a full 60-minute gate run to discover.** `comb_referee.HTML_RUNTIME_SCRIPT_SHA256` is a third such pin, distinct from `EXPECTED_HTML_STRUCTURE_SHA256` and from the four producer SHAs | 5 forms UNEVALUABLE at r14, report partial 40/53; 2 of the pin's 3 hashes had moved | `comb_referee.py:535`, read at `comb_referee.py:2822` | open | The pin itself is re-pinned and **r15 confirms it** (all five emission-binding errors gone). The class stays open because the underlying defect is the enumeration, not this pin: the "census pins that must move together" list under **How we work** did not contain it, and does not name whatever else is like it. An inventory that a producer change can be checked against in seconds — rather than at the end of an hour — is the actual fix |
| **G18** | **A human-reviewed referee control no longer holds, and it is the last thing between the referee and a complete corpus report.** `REVIEWED_2551Q_EXPLICIT_COMPARTMENTS` reviewed 2551Q `p2c5` as `measured` with 14 compartments and `p2c80` as 12; the referee now returns `unevaluable — source topology does not occupy a strict majority of the full comb band` for both | **52/53 forms report at r19** (r18: 40/53). 2551Q is the only one that errors, and it takes 105 subjects with it, which is also why `combs_found` is 4,433 against an expected 4,538. p2c5 measures 6.96pt of a 17.70pt band; p2c80 7.44pt of 18.78pt | `comb_referee.validate_2551q_referee_golden` vs whatever moved the majority rule under it | open | **The pin was NOT moved and must not be** — moving a reviewed control to match the producer that stopped satisfying it is the failure this project already paid for at `EXPECTED_COMBS` (r14) and `HTML_RUNTIME_SCRIPT_SHA256` (G17). Not caused by r19: 2551Q's `index.html`, layout and IR are byte-identical and only three *guide* documents changed; r19 merely made 2551Q reach the check. Same shape as G10's assertions — newly visible, not newly broken. **Reaching PASS is further off than this one form**: `forms_ok` is 0 and 4,385 of 4,433 subjects are `source_unevaluable`, so 53/53 would buy a complete report, not a score |
| **G19** | **A comb slot boundary is taken from a divider the page's own `comb_divider_final_visible_ids` excludes, so a money box claims a compartment the paper does not print.** The lattice already computes the right answer and records it beside the wrong one; `legacy-continuity` outranks it | 5 cells on 2550M (r20), plus the 2 on 1707/1707A that pre-date r20 — the `layout-printed-mismatch` half of `comb_slots_match_printed`, 2 → 7 | `lattice.py` comb band, `legacy_dividers` / `frame_dividers` vs `dividers` | open | **F184.** 2550M `p1c89` is the MM box of a Schedule row: the source strokes ticks at x 260.40 and 263.52, then paints a white fill over the whole box (seqno 477) AFTER the 263.52 tick (seqno 419), so one tick survives to the paper. `slot_x` is `[246.96, 260.40, 263.52, 273.84]` — a 3.12pt compartment. The cell's own `comb.resolution` reads `final_visible_candidate_cells: 2`, `[final-visible-count-regression, legacy-continuity-only]`, so it is already `active_unresolved` and already blocks the gate. **Not patched at r20 on purpose**: dropping a legacy topology is the reviewed `retired_proven_false` transition, which needs independent evidence and a human |
| **G20** | **A retained comb subject whose legacy comb RESOLVED is refused by the referee's retained-subject contract, so a whole form produces no report.** The contract encodes "retained because the topology could not be resolved"; a caption-block refutation is a third shape — resolved geometry, refuted semantics | 6 subjects on 3 forms (2200A `p1c94`/`p1c115`, 2200C `p1c84`/`p1c105`, 2200P `p1c93`/`p1c114`); the other 5 refuted subjects carry `unresolved` and are accepted. Referee 46/53 → **50/53** once the census half moved | `comb_referee.py:4133` retained-subject validation | open | **F191.** The census half (`EXPECTED_RETAINED_SUBJECTS_BY_SLUG` 22 → 33 on seven slugs) is a pin this integration owns and moved with its cause. The contract half was deliberately not touched: changing the adjudicator in the same increment as the producer it adjudicates is what cost `EXPECTED_COMBS` (r14) and `HTML_RUNTIME_SCRIPT_SHA256` (G17), and a producer rewriting its own `resolution_status` to satisfy its referee is the same fault mirrored |
| **G21** | **A caption printed beside a comb is swallowed into the comb's FIRST compartment, which then carries a live single-character input laid over the printed words.** The comb's other compartments are correct money boxes, so the caption-block refutation deliberately leaves it alone | 3 cells, 3 forms: 2200A `p1c111`, 2200C `p1c101`, 2200P `p1c110` — slot 0 is 173.66pt wide against a 14.55pt pitch for slots 1-28 | `lattice.py` cell segmentation / run assignment | open | **F190.** `comb_compartment_glyph_counts` for the cell is `[17, 0, 0, … 0]`: SOME compartment is multi-glyph and EVERY compartment is not, which is exactly why `printed_caption_refutes_comb` is stated over every compartment — refusing here would have cost 28 real money boxes on three forms |
| **G15** | **The `?debug=fields` overlay shipped in `forms/` is the OLD self-referential one; the fixed overlay exists only in `emit.py` and has never been regenerated into the corpus.** In the shipped legend blue dashed means "this input is fine"; in the fixed one it means "printed box with NO input" — the inverse | was 0 / 38; **now `printed box with no input` → 53 of 53, `no usable box` → 0** (r14, 2026-08-07) | `emit.py` overlay, unregenerated | **done** | F172 `fixed`. Nothing needed fixing — the corrected overlay already existed in `emit.py` and had simply never been written out. Regenerating the corpus at r14 shipped it. The Stage-1 definition of done is no longer blocked on the overlay |

G10 is the one to read twice. It is why Stage 2's central guarantee is not yet
real (see Risk R1). **G11 was fixed first** and is `done`: it was the only row
where a single producer bug put a live text box on a statutory constant. Its
successor is **G16**, which is that fix's unpaid half — the assertion that owns
the emission contract was not told the contract changed.

**Not measured / not yet diagnosed:**
- ~~How many of the 626 both-endpoints-unsupported borders have a round or
  projecting cap.~~ **MEASURED at r20: 625, not 626, and 98 of them (15.7%)
  carry a round or projecting cap; 527 are butt-capped, where modelling the cap
  changes nothing by construction.** Stroke census behind it: of 569 OPEN
  stroked subpaths, 229 butt / 270 round / 70 projecting; every open subpath in
  the corpus is a single `l` op, and the 133 multi-`l`-op paths are all CLOSED
  rectangles, which must not be capped.
- Why neither failing assertion moved when the painted-wall fix widened 131
  cells and created 95. **NOT DIAGNOSED.** A number that does not move when it
  should deserves the same suspicion as one that moves wrongly.
- Whether `ddce158`'s referee claim reproduces. **NOT MEASURED.**

---

## Stage 2 — correct

> **Reconciled 2026-08-08 (user decision): batch-versioned immutability.**
> Stage-1 batches are immutable once a sighted gate scores them (tag
> `corpus/rN` per verdict); a generator fix produces the NEXT batch, never a
> mutation of the last. Stage 2 builds `forms-corrected/` from a NAMED batch:
> byte-copy per form, then apply that form's correction records — no record
> means byte-identical copy. The applier's manifest names source batch, every
> record, and input/output sha256. The gate runs on BOTH trees; on the
> corrected tree fidelity must fail ONLY at declared divergences, each named.
> Stage 3 binds to `forms-corrected/` only. Full rationale and the
> counter-check that amended the never-regenerate clause:
> ARCHITECTURE.md § Batch-versioned immutability.


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
- **Adding an assertion touches four places, not one** (added r18, and it is
  now self-enforcing): `audit.ASSERTION_KEYS` + `audit.CHECKS`;
  `gate.REQUIRED_ASSERTIONS`; `gate.BASIC_ASSERTION_COUNT_FIELDS` (an exact
  allowlist — an undeclared published count field reads as
  `detail has unsupported fields`); and the `basic_counts` block of
  `gate._synthetic_audit_record`, which every gate self-test fixture is built
  from. `gate.self_test` now asserts that every non-comb name in
  `REQUIRED_ASSERTIONS` has a declared count contract, so omitting the third
  step fails in 3 seconds instead of at the end of an hour. `comb_referee`'s
  `AUDIT_PRODUCER_SHA256` re-pins with it.
- **`gate.py` does not allowlist assertion-detail SHAPES for the basic
  assertions, only names and count-field names.** The `broken`/`held` contract
  (`holds`, `reason`, `offenders`, `offender_count`, `offenders_published`,
  `offenders_omitted`, `offenders_complete`) is validated structurally and needs
  no change for a new assertion that uses it. Only
  `_normalise_outer_comb_assertion` / `_normalise_outer_offender` are
  shape-exact, and they apply to `comb_slots_match_printed` alone.
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

Nothing.

**Retracted 2026-08-07:** this section previously claimed CI was dead across the
repository, probably from exhausted Actions minutes. That was WRONG on both
counts and is corrected here rather than deleted, because the reasoning failed
in an instructive way. The repository is PUBLIC, so GitHub-hosted minutes are
unlimited and quota could never have been the cause. And CI had in fact run on
this branch at 23:05 -- `CI` passed, `formgen` failed. I ran
`gh run list --branch` once, got an empty result, and concluded "no runs
repo-wide" from a single negative observation without checking whether the
repository even had a quota to exhaust. The real failure was ours:
validate_tree's markup scan reading a `<image>` inside a JavaScript comment.

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

## Implementation packages — r37+ (diagnosed 2026-08-10 at `5cd4017`, main agent)

Written so an implementing agent can execute each package mechanically. Every
fact below was **measured on this tree**, not assumed. Baseline: gate r36
**10/13**; `inputs_over_printed_text` 6 forms/15; `comb_slots_match_printed`
12 forms/25; `inputs_span_no_printed_divider` HOLDS 0; findings 18/133 open
blocker+major; comb-referee 33 (the retained floor); determinism
`56248287ed77`; 45,485 inputs; `EXPECTED_COMB_SUBJECTS` 4583.

Execute **P1 → P3 → P2 → P4** (P3 before P2 is deliberate: P3 removes the
underscore cells from P2's population). One package per round. Division of
labour: the implementing agent does implementation + self-tests + scratch-copy
mutations + regeneration + measurement + pins; the operator (main agent) does
the ledger closures, the full gate, commit and push.

Standing rules, restated because they outrank finishing: never widen a
tolerance or weaken an assertion; never special-case on form code or slug; a
check that cannot be evaluated is a FAILURE; `audit.py` is the judge for
P1/P2 and is **locked** there; every census pin moves in the same commit with
the cause named; `batch.py` does NOT refresh `build/audit.json` — run
`audit.py --assertions-only` separately and measure on the tree actually
written; report real numbers including the ones that got worse.

### P1 — knockout-bitten walls (closes F097, blocker) — READY

**Defect.** 2200C p1 item 1 "Date (MM/DD/YYYY)": only DD is typeable. The
frame's rails carry a 1.56pt white bite mid-height; the cell walk leaks
through the hole; MM and YYYY dissolve into blank slivers instead of comb
cells (p1c122 leaks out to x=219.72).

**Measured mechanism** (`build/ir/2200c-2018.ir.json` p1):

- Solid full-height walls (th .48, y 115.22–132.14) at x0 = 59.52, 73.94,
  102.98, 117.38. DD (p1c5) sits between two of them → comb 2, works.
- The rails at x0 = 30.60 and 175.34 are each THREE collinear fragments:
  black 115.22–124.94, **white (gray=1.0) 124.94–126.50**, black
  126.50–132.14. Both black pieces still reach the frame's top and bottom
  rules — the bite is strictly interior to one drawn stroke.
- Short bottom ticks (th .24, y 125.42–132.14) at x0 = 45.12 (MM), 88.58
  (DD), 132.02 / 146.54 / 160.94 (YYYY).

**Corpus census of the signature** — a collinear same-axis knockout STRICTLY
covering a sub-writable gap between two black fragments of one cluster —
is exactly **8 bites in 2 forms**, nothing else in the corpus:

- `2200c-2018` p1, axis v, gap y 124.94–126.50 (1.56pt) at line positions
  x ≈ 30.84, 175.58, 189.98, 334.75, 450.55, 508.54, 537.46 (the row-wide
  white band bites every rail it crosses on the top row).
- `2000-dst-2018` p1, axis v, x ≈ 192.38, gap y 120.62–122.18 (1.56pt);
  same three-fragment signature (v-rule x 192.14–192.62).

**Three negative cases that MUST stay negative** (all measured; encode each
as a fixture):

1. **A perpendicular witness is a junction statement — never bridge.**
   2200A p1 x0=580.66: black y 136.94–146.30 and 146.78–153.02, gap 0.48pt;
   the only white is the PERPENDICULAR h-rule y0=146.30 (x 537.70–594.60).
   The sheet severed the column from the rule above to make it a comb
   divider (p1c24, divider_x [551.86, 566.38, 580.90]); bridging would split
   the 4-slot comb. The same-axis + collinear condition excludes it.
2. **A witness that abuts the gap does not cover it — never bridge.**
   1800-2018 p1, line y≈805.46: black h-segments end exactly where
   full-height columns cross (gaps 0.01–0.24pt at x 194.92, 290.33, 317.69,
   345.07) and the white segments (y 805.54–805.78) also SKIP those ranges,
   ENDING at the gap edge. With ±CLUSTER_TOL_PT slack a 0.24pt gap is
   swallowed by a witness that merely touches it, so coverage must be
   STRICT: `k[al0] <= a1 + 1e-6 and k[al1] >= b0 - 1e-6`. Same for
   1604e-2018 p1 y≈383.6 (a 0.01pt thick/thin butt-joint notch).
3. **A doorway is a real passage — never bridge.** Bound the gap by the
   form's own `min_fillable_line_metrics(ir)["glyph_height_pt"]` (2.930pt on
   the smallest form; both real bites are 1.56pt). Metrics absent → bound
   0.0 → never bridge.

**Change** (`tools/formgen/lattice.py` only):

New pure helper near `build_lattice` (:1881):

    def bridge_knockout_bites(lattice: Lattice,
                              knockouts: Sequence[dict[str, Any]],
                              axis: str, max_gap_pt: float) -> int

- Returns the bridge count (tests and the probe use it); mutates
  `lattice.spans` in place. `if max_gap_pt <= 0.0 or not knockouts: return 0`.
- Along keys `("y0","y1") if axis == "v" else ("x0","x1")`.
- Per line i: `local = [k for k in knockouts if abs(centre(k) -
  lattice.positions[i]) <= CLUSTER_TOL_PT]`; need ≥2 spans and a local
  witness.
- Consecutive spans (…,a1),(b0,…), `gap = b0 - a1`: bridge iff
  `0 < gap < max_gap_pt` and a local witness covers it STRICTLY (epsilon
  1e-6 — see negative case 2). Merge intervals, count.
- Do NOT compare witness thickness: `line_thickness_gray` reports the
  page-wide cluster max (2000-DST's line reports 0.96 while its local
  fragments are 0.48), so a thickness test would misfire.
- Docstring carries the bite / junction / doorway trichotomy with the three
  measured cases above, and why strictness and same-axis are load-bearing.

Caller — `build_page`, immediately after the two `build_lattice` calls
(:6220–6223), before `merge_grid` (:6234):

    knockout_v = [r for r in page["rules"] if r.get("axis") == "v"
                  and tone_role(r.get("gray")) == "knockout"]
    knockout_h = [r for r in page["rules"] if r.get("axis") == "h"
                  and tone_role(r.get("gray")) == "knockout"]
    bite_bound = (0.0 if fillable_metrics is None
                  else float(fillable_metrics["glyph_height_pt"]))
    bridge_knockout_bites(xl, knockout_v, "v", bite_bound)
    bridge_knockout_bites(yl, knockout_h, "h", bite_bound)

Do NOT touch the raw/legacy lattices (:6044–6046): the legacy view keeps the
old reading; the new comb subjects must register as NEW ACTIVE subjects
through the existing ledger flow.

**Fixtures + mutations** (in `self_test`, same style as the shading-seam
block ~:6790): positive bridge (two collinear black v-fragments, 1.5pt gap,
exact white fragment, bound 3.0 → count 1 and `covers()` true across the
joint band); bare-paper gap → 0; the 2200A shape (perpendicular white only)
→ 0; the 1800 shape (white abutting the gap edge) → 0; doorway (gap 5.0,
bound 3.0) → 0; bound 0.0 → 0. Scratch-copy mutations, each tripping exactly
its own check: strict→±CLUSTER_TOL_PT (abut check fires), drop the
same-axis/collinear filter (perpendicular check fires), drop the size bound
(doorway fires), bridge without witness (bare-paper fires).

**Verify in this order — expected numbers are acceptance, deviations are
STOP-and-report:**

1. `python3 tools/formgen/lattice.py --self-test --ir
   build/ir/2551q-2018.ir.json` → PASS.
2. Probe (scratchpad script): monkey-wrap `bridge_knockout_bites` to record
   counts, run the module's own page build over all 53 IRs → **v-bridges 8
   (7 on 2200c-2018 p1, 1 on 2000-dst-2018 p1), h-bridges 0, all other
   forms 0.**
3. `python3 tools/formgen/batch.py --report build/batch-report.json`;
   `git status` — shipped bytes may change ONLY for the two forms (plus
   their provenance and forms/index.html). Any other bundle → STOP.
4. Layout deltas, enumerated per cell in the report: 2200C p1 gains a
   2-slot comb at x≈30.84–59.76 (divider ≈45.24) and a 4-slot comb at
   x≈117.62–175.58 (dividers ≈132.14/146.66/161.06); the whole top row
   re-forms and the page's cell ids renumber. 2000-DST p1: the wall at
   x≈192.38 restores; the cell spanning it splits.
5. Fresh judge: `audit.py --assertions-only` →
   `comb_slots_match_printed` holds or improves from 12/25 and the two new
   combs land in agreement (printed 2/4 = latticed = emitted); any NEW
   offender is diagnosed in the report before commit.
   `inputs_over_printed_text` ≤ 6/15; the two zero families stay 0.
6. Census + pins, one commit: gate `EXPECTED_COMB_SUBJECTS` 4583→4585;
   referee `EXPECTED_COMBS` 4583→4585, `EXPECTED_COMBS_BY_SLUG["2200c-2018"]`
   +2; `EXPECTED_HTML_STRUCTURE_SHA256` recomputed for exactly the two
   slugs; `LATTICE_PRODUCER_SHA256` re-pinned with a dated cause. If the
   re-formed row creates combs beyond +2, list each with its ticks and move
   the census by the true, named delta.
7. Referee corpus run (CLI per gate `_comb_referee_command`): forms_error 0;
   combs_found = new census; both new subjects `measured` (a
   source-unevaluable landing is reported with its reason before shipping);
   emission mismatches stay 33; `subjects_retained_unresolved` stays 33;
   pending_transitions 0. A retained subject changing state → STOP.
8. `guides.py --self-test` — if the per-page tables for 2200C p1 /
   2000-DST p1 move, update those pins with the per-cell cause (precedent:
   commit 9f76779).
9. Input delta vs HEAD, counted the same way on both sides: expect +6 on
   2200C (2+4), small ± on 2000-DST; report exact.
10. Hand back to the operator: F097 closure text (mechanism, bridge
    conditions, shipped slot counts, census moves, the three negative cases
    proven by fixture), full gate (expect 10/13, findings 18→17), commit,
    push.

### P3 — ruled blanks, fixed upstream (closes F148, F149, F200) — before P2

The reverted attempt (commit `5cd4017`, finding F200) proved the emit-side
fix collides with `inputs_over_printed_text` while underscores are TEXT: an
input on the ruled blank necessarily overlaps the run that draws it. Fix it
upstream: `extract.py` reclassifies an underscore group as the RULE it
typographically is. The blank becomes paper under a drawn rule; the lattice's
new h-line at the blank's baseline splits the caption cell from the blank
strip; the strip becomes an ordinary field cell and receives its input
through the NORMAL flow. No assertion needs an exception anywhere.

Prerequisites, in order, each answered in the report before any code:

1. Confirm the assertion reads IR text_runs, not `page.get_texttrace()`:
   `grep -n "inputs_over_printed_text" tools/formgen/audit.py`, read the
   offender construction. If it is texttrace-based the upstream fix cannot
   clear it — STOP; the only honest route is then a reviewed weakening of
   the assertion, which is the user's decision.
2. GROUP-based population census — this is exactly where the reverted
   attempt went wrong: its validation counted whole-run underscores (34
   cells) while its implementation matched groups inside mixed runs (+61
   inputs). The census and the implementation MUST share one definition: a
   group = ≥3 consecutive `_` glyphs within one run, split at any other
   glyph. Publish groups / cells / forms / expected input delta. Known
   members beyond the 34 whole-run cells: 1600WP p1c214 `Page ____ of ____`
   (2 groups), 1700 p2c40, 1707 p1c214, 1801 p2c69, 2200A p3 `XA ____` ×3,
   2200AN p2 `XG___`, 1706 p2c125/p2c127.
3. Find where rule dicts are schema-locked (`grep` rules validation in
   gate.py / validate_tree.py / comb_referee.py) BEFORE adding any
   provenance key; if key-locked, either carry provenance an allowed way or
   justify the schema move explicitly in the report.

Change (`tools/formgen/extract.py`): split text runs at underscore-group
boundaries; publish each group as an h rule at the GLYPH'S OWN INK BAND,
measured from the extraction API's per-glyph boxes (rawdict/texttrace) — if
the band is not derivable for a group, LEAVE IT AS TEXT and count it (fail
closed, never guess); tone from the run's fill; the group's glyphs leave the
run's text. Add extract mutations: a 2-underscore group stays text; an
underivable ink band stays text; a mixed run splits into text+rule+text at
the measured extents.

Verify: text parity stays clean 53/53 (both sides re-extract with the same
extractor — symmetric by construction); rules parity stays clean (emit draws
IR rules; the round trip re-extracts the drawn stroke); the caption cells
split and ids renumber on ~13 forms; every blank strip's classification is
reported — a strip the sliver rule refuses (height < glyph height) gets no
input and is RECORDED, not forced; `inputs_over_printed_text` improves;
comb censuses unchanged; structure pins recomputed for every touched form;
extract self-test probe counts stay pinned. Close F148/F149 on shipped-bytes
evidence, mark F200 fixed ("upstream reclassification — option 2 of the
recorded pair"), and re-verify the full census population end-to-end.

### P2 — part-constant description rows (F151, blocker) — measure first, abort honestly

AFTER P3, so the underscore cells are already out of this population.

Target: 1701-2018-conso p2 Schedule D p2c132/136/140/144 (x 26.16,
w 452.71) and Schedule C p2c97/103/109 (x 54.24, w 283.61) — each kind
`label` holding ONLY a row number (`1 `, `2 `…), ≥97% blank, bordered, with
fillable siblings. The measured trap: 1,875 label cells carry a blank run
≥100pt, most being section headers and caption rows that must NEVER gain an
input; bordered item-number boxes are labels whose ink fills them.

Measurement script first, on the post-P3 tree, for every label cell:
border_count; shading coverage AT THE CELL (`on_shaded_paper` with the
form's glyph height — label cells were never asked); printed-ink x-extent as
a fraction of width; whether the ink is a single leading cluster; the blank
remainder's width × height against the form's own line metrics. Anchors =
the 7 target cells; counter-anchors = full-width unshaded caption rows and
item-number boxes. Encode a rule ONLY if the corpus separates bimodally with
a constant-free bound (precedent: the 4.4× separation behind
printed_partitions, log r33). If the populations overlap → ABORT: publish
the distributions, leave F151 open with the measurement attached.

If separation holds: extend the classification (lattice `classify_cell` or
emit `field_verdict` — pick the layer that keeps audit.py independent; it
stays locked) so a bordered, unshaded cell whose printed ink is a leading
constant with a viable blank remainder is a FIELD; emit already trims the
writing box past leading ink. Every cell that gains an input corpus-wide is
listed in the report; `inputs_over_printed_text` must not regress.

### P4 — placement/artwork family (diagnosis recipes)

- **F027/F030** (stray black bar outside the frame, 1700/1701 p1, every
  page): locate the bar in the IR, then in the SOURCE content stream; check
  the clip state (extract models clips since r20's CLIP_PROBE work). If the
  source draws it clipped away and we paint it → extract/emit clip bug; fix
  and add the shape to the clip fixtures. If the source paints it unclipped
  → faithful rendering, close not-a-defect with the operator evidence.
- **F064/F065** (1707 items 8A/9: drawn comb band / white specify line, no
  input): NOT the P1 mechanism (1707 has zero bites in the census). Probe
  the cells (x ~275–594, y ~336–347 and item 9's line): kind, comb,
  field_verdict reason, ledger state — then choose the fix.
- **F070/F102** (runs set 4–5pt too high: 1707A `Calendar`, 2200P
  ` Total Tax– `): diff IR run y against emitted CSS top for the named runs;
  the delta should implicate one emit placement path; add the run shape to
  emit's self-test with the fix.
- **F060/F120** (guide reflow: superscript reordered out of `(4th)`;
  orphaned ATC codes): both live in `emit.reflow_page` → `_column_bands` →
  `_table_markup` (~emit.py:3305). Reproduce on those two guides; fix
  ordering/row-fill; verify no other guide's bytes move.
- **F134** (2553 input over `DD` header): re-verify by GEOMETRY on the
  current tree (ids renumber). If it is the documented side-bearing
  over-reach (audit.py:541–548), close not-a-defect citing F199; if real
  ink inside a comb rectangle, it joins F199's frozen-geometry list —
  report, do not force.
- **F073** (1800 centavos): the region split landed in r33; the residual
  claim is font size/overflow. Compare the region inputs' fitted face
  against sibling money rows; fix face selection or close with
  measurements.
- **F166** (2550Q address/ZIP): locate by geometry — an earlier probe found
  p1c6 at 4 slots / 4 inputs, so it may already be fixed; verify against
  the official raster crop before closing.

### P5 — needs the user (do not implement)

- **F154** sworn-declaration strip: zero area_fills under it, so no tone
  rule can see it; a note strip segmented as a field (same class as F198).
  Any fix costs shipped inputs elsewhere — user review.
- **F156** `WE` swap claim: needs the official sheet, not the content
  stream.
- The three structural blockers stay deliberately deferred and are listed
  in F199/F196 and GOAL.md: audit runtime attestation (comb-referee can
  never PASS without it), glyph ink extents (12 of the 15
  `inputs_over_printed_text` survivors are documented over-reach; a
  font-outline route via bundled Arimo/Tinos exists), and build_lattice
  fused positions (F196's 6 cells).

---

## Log

- **2026-08-10** — Implementation packages P1–P5 appended above the log,
  written for mechanical execution by implementing agents. P1
  (knockout-bitten walls, F097) fully diagnosed: 8-bite corpus census, the
  strict-coverage and same-axis discriminators, and three measured negative
  cases (2200A junction, 1800 abutting witness, doorway). P3 re-scoped
  upstream after the reverted emit-side attempt (F200). P2 gated on a
  bimodality measurement with an explicit abort. Diagnosed at `5cd4017`,
  gate r36 10/13.

- **2026-08-08** Batch-versioned immutability reconciled with the user and recorded (ARCHITECTURE.md): stage-1 batches freeze per scored gate, stage 2 applies records to a named batch, uncorrected forms byte-copy, gate runs on both trees. One true stage-2 record known: 2550M TIN 3->5.
Newest first. One line each.

- **2026-08-07 (r23)** — **The three regressed assertion families, paid; two
  are green and the third is back to r20's form count.** `emit.py` now lays
  every rectangle it DRAWS for a comb — the slot div, the input inside it, the
  band-template JSON a cloned row is re-laid out from, and the face
  `field_box` fits — out on the WRITING box through one function,
  `comb_writing_rect`, while the divider band survives emission unmodified for
  `comb_referee.classify_band` and the reviewed 2551Q control. 2550M's item-4
  TIN compartments are **14.16pt inside a 15.60pt row again, not 3.12pt**
  (F186 closed on the shipped bytes and on a 3× screenshot). That corpus
  change alone took `comb_slots_match_printed`'s `invalid-emission` population
  **64 offenders on 25 forms → 3 on 3** with `audit.py`'s source-occupancy
  query untouched: it had been asking the source about a 3pt band where the
  printed constant is not. `money_boxes_have_inputs` 4 → **0** and
  `printed_box_peers_all_fillable` 1 → **0**; the only exclusion added,
  `audit.source_bureau_reservations`, reads the sheet's own
  "(To be filled up by the BIR)" from the pinned PDF's text operators — not
  from `emit.BureauReservation`, not from the IR — reports the matching
  phrase's rectangle and never its line (0605 sets two captions on one
  baseline, and a line-wide rectangle would excuse the taxpayer's Return
  Period boxes; a mutation to it fails two new self-test assertions), claims
  **exactly ONE box corpus-wide**, and publishes `boxes_bureau_reserved`
  declared in `gate.BASIC_ASSERTION_COUNT_FIELDS`. **Reported loudly:
  `inputs_over_printed_text` got WORSE — 19 forms/131 → 20/147, +21 new and −5
  cleared** — and every one of the 21 is G05's existing caption-plus-comb
  population, cell for cell, that r22 had hidden rather than fixed. **F187**
  files the residue: 2200-A/C/P's Bureau band still reports one compartment to
  `comb_slots_match_printed`, which asks for ink and cannot see a caption; not
  fixed here because that assertion's shape is contract-bound by the referee.
  Census: **no comb pin moved and none should have** — `lattice.py` is
  byte-identical, 4,583 subjects / 4,561 active / 22 retained, 45,765 inputs
  and 40,213 slot divs all unchanged. All 53 `EXPECTED_HTML_STRUCTURE_SHA256`
  and `AUDIT_PRODUCER_SHA256` moved; `HTML_RUNTIME_SCRIPT_SHA256` was
  re-derived and did not. The 53-document review is the strongest yet: **tag
  inventory delta ZERO for every tag name, 239,562 elements before and after,
  visible text token-for-token identical**, the whole change being slot-div
  style attributes. 33 → **32 blocker+major open of 129**. No check, tolerance
  or assertion was weakened. **Gate r23: 9/12, the same three red as r22, and
  `money_boxes_have_inputs` and `printed_box_peers_all_fillable` are GONE from
  the `assertions` detail — the full clean-tree gate confirming 4 → 0 and
  1 → 0. Determinism `ba1bd2d8c47e`, moved and had to. The comb referee is
  UNEVALUABLE for character-for-character r22's reason (2550M `p1c89`/`p1c90`,
  F184's cells) and r23 neither cleared nor worsened it.**

- **2026-08-07 (r20)** — **`printed_box_peers_all_fillable` PASSES, 14 of 53
  forms → 0, with `audit.py` byte-identical (`8d22a957…`) throughout.** Two
  producer bugs in `lattice.py`: `GroupGeometry.span` filtered a cluster's
  coverage by distance to the cluster's own *mean* centre and so could drop a
  rule that is itself a member (0619-E's Amended-YES wall, 0.35 against a 0.30
  tolerance, merged the box into its caption); and `assign_points` placed a text
  run by its bounding-box centre, which is the run's ADVANCE, so the whitespace
  in `Calendar        Fiscal` counted as printed text inside the checkbox drawn
  in the gap. `glyph_ink_spans` reads the per-character origins the IR already
  carries. **`extract.py` now models PDF 32000-1 §8.4.3.3 line caps** — 340 of
  569 open strokes in this corpus carry a round or projecting cap and were being
  published 0.36pt short at each end, which is how 2550M's four year boxes
  reached the taxpayer as one input — proven by a written-here 200×200 probe
  page with 13 asserted cases and a mutation that restores the old behaviour.
  **15 findings closed on measurement** (F049, F054, F058, F062, F106, F135,
  F150, F152, F153, F173–F177, F180), each against its own coordinates in the
  shipped bytes and by a checker that never consults the r20 audit, so a box
  that cleared because its row *peer* lost an input would still read as
  uncovered. 55/126 → **42/128** blocker+major open. **Reported loudly:**
  `comb_slots_match_printed` got worse by 3 — five money-comb cells on 2550M
  claim a compartment the sheet does not print, filed as **F184** / new row
  **G19**, and deliberately not patched because the fix is the reviewed
  `retired_proven_false` transition. **F185** files the 11 comb-spanning inputs
  the cap model made visible. A third `lattice.py` fix was needed on the way:
  the first regeneration made that assertion worse by 13, not 3, because a
  suppressed subject's `mapped_partition_cell_ids` is a partition and nothing
  enforced it — 2550M's `p1c7` nests inside `p1c6` and both claimed three cells,
  which correctly invalidated the whole form's owner registry.
  `resolve_retained_partition_overlaps` gives a contested cell to the smallest
  claiming area; corpus-wide 3 cells, one page, one form, no mapping emptied.
  **Census: `EXPECTED_COMBS` 4,538 → 4,543, retained 17 → 21 on six slugs, 25 of
  53 `EXPECTED_HTML_STRUCTURE_SHA256`, both producer SHAs — and
  `gate.EXPECTED_COMB_SUBJECTS` 4,521 → 4,543, which was ALREADY WRONG at HEAD**
  because r19 moved its twin and not it. `comb_referee`'s own self-test caught
  the census move exactly as designed (its retained-one fixture slug 2551M went
  to retained-three), so the fixture rotated to 1604-CF and 2551M became the
  retained-many negative control. No check, tolerance or assertion was weakened.
- **2026-08-07 (r18)** — **The audit can see the field layer. G10 moves from
  `open` to `fixing` with its first two assertions.** `inputs_span_no_printed_divider`
  walks 44,536 emitted inputs and asks the pinned PDF whether it drew a
  compartment divider inside one: **79 offenders on 11 of 53 forms**.
  `printed_box_peers_all_fillable` recovers 7,223 printed boxes from the source's
  own paint stream and reports a box with no input whose identical row peer has
  one: **14 offenders on 14 of 53 forms**. Neither reads the layout, the plan,
  emit.py's markers or the IR — the population that was blind was blind precisely
  because the two nearest existing assertions enumerate from the producer that
  made the mistake. **These 93 offenders are newly VISIBLE, not new**; every one
  was in the shipped corpus at r14. 16 of them independently re-derive existing
  human findings at the same coordinates (0619-E's A10 box matches F152's
  reviewer-measured `(276.0, 135.0) 12.5 x 10.5`; 2550M's A9 `p1c2` is G02a), and
  **9 were on populations no open finding covered: F173–F181** — five checkboxes
  that make a required election unstateable (1701 ATC II016 Mixed Income 8%;
  1701MS spouse OSD; 1706 item 11 treaty "No"; 2200M item 12 treaty "No"; 2550Q
  **2nd quarter**) and four comb-spanning input groups including the TINs of
  1600WP and 2553. `gate.py` grew its two allowlists, its fixture and its count
  literal (8 → 10) in the same commit, plus a new self-test invariant so the next
  assertion cannot be added without its count contract.
  `comb_referee.AUDIT_PRODUCER_SHA256` `d31b4d7a` → `8d22a957`. **No census pin
  moved and none should have** — no generator changed, `batch.py` re-converted
  53/53 byte-identical and `forms/index.html` regenerated byte-identical.
  **Reported loudly: PT 060 still reads 5% and is officially 2%.** The guide
  reflow fix did not land (`guides.py` byte-identical to r14; the work was
  reported `fixed: false`), so **F127 is REOPENED** with the retraction in its own
  resolution — its closure measured prose flattening, which really is gone, while
  the code-to-rate association it says was destroyed still is. Blocker+major open
  49/116 → **59/125**, and going up for these two reasons is the ledger working.
  One cosmetic defect found and NOT fixed here because `audit.py` is another
  agent's file: `audit.py:13516` prints `assertions {n}/8` from a literal, so the
  console now reads `10/8`. Console-only, no check reads it, but it is a stale
  census literal of exactly the G01 shape and should derive from
  `len(ASSERTION_KEYS)`. **Gate r18: 9/12, three red, the same three as r17 —
  determinism `8ceeab9e506d`, identical to r14/r15; both pre-existing assertion
  counts unmoved at 20 and 22; the referee's UNEVALUABLE is exactly r17's
  residue on 1604C, 1700, 1701MS, 1702EX, neither cleared nor worsened, and
  still undiagnosed.**
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
