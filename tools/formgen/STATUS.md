# STATUS — formgen, measured state

**Update rule: any commit that moves a number below updates this file in the
same commit.** This is the only formgen document allowed to hold measured
status numbers (`GOAL.md` owns that rule; `README.md` owns the process).

Measured 2026-08-07 over the 53-form corpus, on branch `gol/form-correction`,
regenerated at the r23 producer bytes. Assertion counts are from a corpus-wide
`audit.py` run over that regeneration; the findings tally is recomputed from
`review-findings.json`.

## r23 — the three regressed assertion families, and what it cost to fix two of them

**r23 starts nothing. It is r21/r22's three regressions, paid.** Two of the
three are gone; the third is gone as a *form count* and its residue is filed.
One family got worse and that is reported first, not last.

| Assertion | r20 | r22 | **r23** | |
| --- | --- | --- | --- | --- |
| `comb_slots_match_printed` | 22 / 188 | 36 / 254 | **22 / 193** | form count back to r20 |
| `money_boxes_have_inputs` | 0 / 0 | 4 / 4 | **0 / 0** | **PASSES again** |
| `printed_box_peers_all_fillable` | 0 / 0 | 1 / 1 | **0 / 0** | **PASSES again** |
| `inputs_span_no_printed_divider` | 11 / 67 | 5 / 33 | **5 / 33** | unmoved, offender-for-offender |
| `inputs_over_printed_text` | 20 / 149 | 19 / 131 | **20 / 147** | **WORSE by 1 form / 16 offenders** |

### Reported loudly: `inputs_over_printed_text` got worse, and the fix did it

**+21 new offenders, −5 cleared, 19 forms → 20.** Every one of the 21 is the
population STATUS.md has carried since the writing-surface increment: a comb
cell whose lattice rectangle spans **caption and comb**, so a writing box that
fills the cell reaches the caption printed in its upper half. The offenders
name themselves — `"   Zip Code"` (1600WP `p1c24`), `"Telephone No."` and
`"Zip Code"` (1604CF `p1c16`/`p1c21`), 2551M `p1c74`/`79`/`86`, 2553
`p1c79`/`84`/`91`, 2316 `p1c37`–`40`, 2550M `p1c89`–`91` — the same cells, cell
for cell, that the earlier increment listed under this exact heading.

**r22 had not fixed them. r22 had hidden them**, by shrinking every comb's
typing surface to the 3.12pt divider band, which is too short to reach any
caption and too short to type in. A number that improved because the field
stopped being usable is not an improvement, and restoring the field restores
the debt. The fix belongs in `lattice.py`'s cell segmentation (row **G05**),
not in this assertion and not in the emitter.

### What fixed the other two: emit.py lays out on the WRITING box (F186)

`emit.comb_writing_rect` is now the **one** reader of a comb's vertical extent
for everything the emitter draws — the slot div, the input inside it, the
band-template JSON a cloned row is re-laid out from, and the face `field_box`
fits — and it returns `writing_y0`/`writing_height_pt`. The **divider band**
(`y0`/`y1`/`height_pt`) survives emission unmodified, because that is the
contract `comb_referee.classify_band` seeds source topology from and the one
the reviewed 2551Q control was signed against. `emit.py`'s new
`comb_writing_rectangle_assertions` drives both halves at once, and a mutation
that restates the writing box into `y0`/`y1` fails it.

In the shipped bytes: 2550M `p1c9` (item-4 TIN, a 15.60pt row) reads
`top:0.72pt;height:14.16pt` on all three slots where r22 shipped **3.12pt**;
2316 slot 0 reads `top:0.45pt;height:13.92pt` against r22's
`top:8.71pt;height:6.05pt`.

**Why that moved an assertion nobody edited.** `audit.py`'s
`comb_slots_match_printed` asks the SOURCE whether it printed a constant in
each **emitted** slot rectangle — that is how G16 was closed, and it is right.
With the rectangle collapsed onto a 3pt band, the query landed where the
constant is not, so 64 correctly-refused compartments across 25 forms read as
`editable comb slot has no live input element and the source prints no
constant or shading in that compartment`. Restoring the rectangle takes that
population **64 → 3**. The assertion was not touched.

### The one exclusion added, and its blast radius is ONE box

0605's `p1c17` — `BCS No./Item No. (To be filled up by the BIR)` — is blocker
**F147**, fixed at r22: the box is emitted with no input and
`data-preprinted="bureau"`. Two assertions then reported it, and both were
right on their old model of the paper and wrong about this box:
`money_boxes_have_inputs` called it an `enclosed empty box, no input`, and
`printed_box_peers_all_fillable` reported it against four fillable row peers.

`audit.source_bureau_reservations` now reads the reservation **from the pinned
PDF's own text operators** (`drawn_glyph_boxes`) and from nothing else — not
from `emit.BureauReservation`, not from the IR, not from the layout. The two
answer the same question about the paper through different producers, which is
what still lets this one catch an emitter that reserves a box the sheet does
not.

It is not a relaxation and the numbers say so:

- **Corpus-wide it claims exactly ONE box**, 0605 `p1c17`, and every caller
  publishes the count as `boxes_bureau_reserved`, declared in
  `gate.BASIC_ASSERTION_COUNT_FIELDS` so an undeclared count would fail the
  gate rather than pass quietly.
- The rectangle reported is the **matching phrase's own glyphs, never its
  line**: 0605 sets `Return Period (MM/DD/YYYY)` and `BCS No./Item No. (To be
  filled up by the BIR)` on ONE baseline, and a line-wide rectangle would hand
  the taxpayer's Return Period boxes the Bureau's excuse. A mutation to the
  line-wide rectangle fails two new `audit.py` self-test assertions.
- The phrases are matched **without spaces**, because `drawn_glyph_boxes` drops
  whitespace glyphs; the list quotes the paper including 0605's own missing
  "by", and `(To be filled up by the taxpayer)` does not match.
- Prose is refused: `The machine validation shall reflect the date of payment`
  reserves nothing, while `Machine Validation/Revenue Official Receipt Details`
  does — line-start, not substring.

2200-A/C/P's Bureau band needed **no exclusion at all**: with the writing
rectangle restored its compartments are no longer ink-free, and
`money_boxes_have_inputs` cleared them on the source's own answer.

### The residue, filed rather than absorbed

Three offenders remain inside `comb_slots_match_printed` — 2200A `p1c115`,
2200C `p1c105`, 2200P `p1c114`, the Bureau band's left compartment — because
that assertion asks the source for **ink** and a reservation is a caption. It
is **F187** (minor). None of the three changes its form's verdict: those forms
fail on 27, 25 and 26 offenders of their own. It is deliberately not fixed
here, because that assertion's published shape is contract-bound by
`comb_referee._normalise_outer_comb_assertion`, and moving the referee's
subject and the referee in one increment is what GOAL.md's user decision 1
forbids.

### The four user-visible checks, looked at rather than counted (r23)

Screenshots at 3× device scale over the **shipped `forms/` tree** through a
local static server, in the session scratchpad under `r23/`.

| Check | Verdict | Evidence |
| --- | --- | --- |
| 2550M's comb slot height is the writing box, not 3.12pt | **YES** | `F186-2550m-item4-tin-writing-box.png`. Browser-measured slot 18.88 CSS px = **14.16pt** in a 15.60pt row; the typed digits sit centred in the printed box, not on its floor |
| 2316 item 3 TIN still shows 14 boxes | **YES — 14, unchanged** | `F111-2316-item3-tin-row.png`. `p1c17`+`p1c12`+`p1c14`+`p1c16` = 3+3+3+5 = **14 compartments, 14 inputs**, a `9` typed into every one. F111's fix at r22 is intact and r23 did not disturb it |
| 0605 BCS No./Item No. is NOT fillable | **YES — still refused** | `F147-0605-bcs-bir-only.png`. `p1c17` carries `data-preprinted="bureau"` and **0 inputs**; the caption "(To be filled up by the BIR)" is printed above an empty box with nothing to type into |
| A real money box on each money_boxes-failing form IS fillable | **YES, all four** | `money-2200a.png`, `money-2200c.png`, `money-2200p.png` — the `Tax Payment/Deposit` comb, **14 compartments each, every one typed into**, decimal separator intact. `money-0605.png` — 0605 item 21 `Total Amount Payable` holds `1,234,567.89`. 0605's money boxes are plain fields, not combs, so the currency shot there is a text field by construction |

### Corpus census — r23

Nothing but geometry moved. **No comb census pin moved and none should have:**
`lattice.py` is byte-identical to r22, so the ledger is the same ledger.

| Quantity | r23 | r22 | Note |
| --- | --- | --- | --- |
| Bundles / unique codes | 53 / 50 | 53 / 50 | unchanged |
| Pages | 116 | 116 | |
| Comb ledger subjects (`EXPECTED_COMBS`) | **4,583** | 4,583 | unchanged — no lattice change |
| Active comb cells / retained | **4,561 / 22** | 4,561 / 22 | re-derived from the fresh `build/layout` |
| Emitted inputs | **45,765** | 45,765 | unchanged |
| Comb slot divs | **40,213** | 40,213 | unchanged |
| Comb slots with no input | **287** | 287 | the compartments the source already filled in, plus the Bureau's |
| Form documents changed | **53 of 53** | — | plus `forms/index.html`; every one an attribute-value change |
| Findings | **187** | 186 | **32 blocker+major open of 129** (r22: 33) — F186 closed on the shipped bytes, F187 filed open (minor) |

**Pins moved, each with its cause recorded at the constant:** all 53
`EXPECTED_HTML_STRUCTURE_SHA256` (the emitted documents' geometry) and
`AUDIT_PRODUCER_SHA256` `8d22a957` → `cf7ed2bd` (the Bureau reservation).
`HTML_RUNTIME_SCRIPT_SHA256` was re-derived and did **not** move — all three
pinned runtime scripts are byte-identical, which is the standing evidence that
a layout change did not reach the page runtime.
`LATTICE_/EXTRACT_/VERIFY_PRODUCER_SHA256` are unchanged and still match.

**The 53 re-pinned documents were reviewed, not rubber-stamped**, and the
review is unusually strong: the tag inventory delta is **ZERO for every tag
name** — 239,562 elements before and 239,562 after, nothing added and nothing
deleted — and **visible text is token-for-token identical in every one of the
53**. The whole change is `style` attribute values on `<div class="s">`.

All 11 self-tests pass (10 modules plus `validate_tree`), including a new
`audit.py` mutation-proven pair for the reservation rectangle and a new
`emit.py` block that drives the writing-box/divider-band split in both
directions.

## r20 — a printed box a taxpayer must tick now has somewhere to tick

**`printed_box_peers_all_fillable` goes 14 offenders on 14 of 53 forms to ZERO
on zero, and `audit.py` is byte-identical while it happens** (sha256
`8d22a957…`, the r18 pin, unmoved). The assertion was not read, referenced,
narrowed or re-pinned by either producer fix. It is the first of the four red
assertion rows to go green since it was written.

The other three did not go green, and one moved the wrong way. All four, r19 →
r20, forms of 53 and offenders:

| Assertion | r19 | r20 | |
| --- | --- | --- | --- |
| `printed_box_peers_all_fillable` | 14 / 14 | **0 / 0** | **PASSES** |
| `inputs_span_no_printed_divider` | 11 / 79 | 11 / **67** | 24 offenders cleared, 11 appeared; form count unmoved |
| `inputs_over_printed_text` | 20 / 149 | 20 / 149 | unmoved, offender-for-offender |
| `comb_slots_match_printed` | 22 / 185 | 22 / **188** | **+3, reported loudly below** |

### The two producer bugs behind the checkbox class (`lattice.py`)

**1 — a lattice line did not count its own defining rule as coverage.**
`cluster_collinear` chains rules by pairwise *adjacency*, so a cluster can be
wider than `CLUSTER_TOL_PT` (0.3) and its position is the *mean* of its
members' centres. `GroupGeometry.span` then filtered `all_ink` by distance to
that mean and could therefore drop a rule that is itself a member. On 0619-E
the "Amended Return? Yes" checkbox's left wall (centre 275.64) is one of ten
fragments in the cluster at 275.99 — 0.35 > 0.30 — so the column claimed no ink
over the box's own 12pt and the box merged leftward into the caption.
`line_thickness_gray` already exempts a cluster's own rules for weight and
tone and says so in its docstring; `span` now does the same.

**2 — a text run's WHITESPACE was counted as printed text inside a printed
box.** `assign_points` placed a run by its bounding-box centre, and a bounding
box is the run's *advance*, not its ink. `Calendar        Fiscal ` spans
66.5–148.92; its centre 107.7 lands inside the checkbox drawn at 106.08–119.52
**in the gap between the two words**, so the box held "text", was not
`is_empty`, and `classify_cell` returned `label`. The other eight are the same
sentence: `Yes      No` (1706, 2200M), ` 2nd      3rd` (2553), `?        `
(2200S), `        23B` (2551M). `glyph_ink_spans` now reads the per-character
origins and advances every run in the corpus already carries and returns the
extents of the NON-BLANK characters; a run whose home cell holds any of its ink
does not move, so the 1,575 runs whose centre merely falls between two letters
are untouched.

### The comb class (`extract.py`): PDF 32000-1 §8.4.3.3 was not modelled

A round (`J 1`) or projecting (`J 2`) cap inks **half a stroke width past the
declared endpoint** of an open subpath. The IR published those strokes at their
declared endpoints, so a comb tick stopped 0.36pt short of the rail it lands on
and `lattice.split_verticals` filed it as a box border — the compartment
disappeared. 340 of this corpus's 569 open strokes carry such a cap.

`cap_extension_pt` and `open_stroke_ends` model it, applied to the two ends of
a **reconstructed subpath** only: never to `re`/`qu`, never to a polyline that
returns to its own start, never to an interior join — capping per op would have
grown 133 rectangles-drawn-as-four-`l`-ops by half a stroke on all four sides.
No fixture in either corpus draws a round cap, so a written-here probe page
(`CAP_PROBE_STREAM`, 200×200, 13 asserted cases) proves both directions, with a
mutation that restores exactly the old behaviour.

**What it bought, on the paper:** 2550M item 1 `For the Month of (MM/YYYY)` —
the user's original "four year boxes rendered as one big box" — is now four
compartments with one input each, screenshotted with `2 0 2 7` typed into them
(`scratchpad/blockers/F180-2550m-item1-year-comb.png`). All eight of F180's
named inputs left the offender list.

### Reported loudly: `comb_slots_match_printed` got worse by 3

Not hidden and not explained away. The move is four separate things:

    -1  emission-source-position-mismatch          (2550M, genuinely fixed)
    -1  emission/layout-source-outer-position      (2550M, genuinely fixed)
    -1  source-topology-unevaluable                (181 -> 180)
    +5  layout-printed-mismatch + emission-printed-mismatch   (2550M, NEW)

The +5 is one mechanism and it is now finding **F184**. 2550M's Schedule money
boxes get one more compartment than the sheet prints, because a slot boundary
is taken from a divider the page's own `comb_divider_final_visible_ids`
excludes: the source strokes two ticks in the MM box (x 260.40 and x 263.52),
then paints a **white fill over the whole box** (seqno 477) after the 263.52
tick (seqno 419) and before the other, so only one tick survives to the paper.
A 30× raster of the pinned PDF shows one tick
(`scratchpad/blockers/2550m-p1c89-ticks.png`). The layout already records the
right answer beside the wrong one — `final_visible_candidate_cells: 2`,
`reason_codes: [final-visible-count-regression, legacy-continuity-only]` — so
the subject is `active_unresolved` and already blocks the gate. **Deliberately
not patched here:** dropping a legacy comb topology is the reviewed
`retired_proven_false` transition, which needs independent evidence and a human,
not an integration-time edit to another agent's file.

### A registry invalidation found on the way, and fixed (`lattice.py`)

The first r20 regeneration made `comb_slots_match_printed` worse by **13**, not
3. Cause: a suppressed subject's `mapped_partition_cell_ids` is a *partition*,
and nothing enforced it. Once 2550M's `p1c7` (66.00, 118.80, 99.84, 134.40)
lost its rectangular owner too, it and the row band `p1c6` (28.80, 117.12,
582.72, 136.32) that contains it both claimed `p1c116`, `p1c122`, `p1c123` —
and `audit.validate_comb_owner_registry` correctly invalidated the **whole
form**, taking all 17 of its comb subjects to `source-topology-unevaluable`.
`resolve_retained_partition_overlaps` gives a contested cell to the smallest
claiming area. Corpus-wide: 3 cells contested, on one page of one form, no
mapping emptied, and the registry-invalid offender count is back to 0.

Attributed by bisection over the two producers, both directions:
`new extract + old lattice` reproduces it exactly; `old extract + new lattice`
does not. So the trigger is the cap model and the defect is the ledger's.

## PT 060 reads 2%. It is officially 2%. FIXED at r19.

**This is the third time this claim has been made and the first time it is
measured on the tree that was written.** r17 closed it wrongly, r18 retracted
that closure and reported `fixed: false`, and r19 lands it.

`forms/2551m-2002/guide.html`, first `gl-table`, the PT 060 row, verbatim from
the shipped bytes:

```html
<tr><td>PT 060</td><td>Franchises on electric utilities, gas and water utility</td><td>2%</td><td></td><td>performing quasi-banking functions</td><td>5%</td></tr>
```

The table is 19 x 6 where it was 19 x 4. PT 060 carries **2%** in its own Tax
Rate column, and the `5%` that used to sit against it is back in the RIGHT
half's rate column where the source printed it — it belongs to PT 111.

**Scored by a checker that shares no producer with `emit.py` or `guides.py`**
(`scratchpad/r19_rate_check.py`, written for this closure; it reads the shipped
HTML and compares against the 15 rates read independently out of
`~/Downloads/forms/2551M/2551m.pdf` sha256 `f678be68…` page 2 with
`pdftotext -layout`):

| | ATC codes carrying exactly their official rate |
| --- | --- |
| shipped bytes at r18 (`HEAD:forms/2551m-2002/guide.html`) | **0 of 15** — the table had four columns, so no code→rate association existed at all |
| shipped bytes at r19 | **15 of 15** |

Token census across the same two files: **1,283 tokens before, 1,283 after; 20
percent-tokens before, 20 after.** Nothing was added and nothing was dropped —
only re-associated.

### The owner named in G13, STATUS.md and F167 was wrong, and that is why it failed twice

All three named **`guides.py`'s reflow**. `BLOCKER-PLAN.md` C9 named `emit.py`
and was right. The defect is `emit.py:reflow_page` → `_column_bands` →
`_table_markup`. So r18's proof that "`guides.py` is byte-identical" proved
nothing about this defect, and the misattribution is the reason the fix did not
land twice.

**Mechanism, measured.** 2551M page 2's ATC schedule has exactly one horizontal
rule in its whole 170pt band — the table foot — so `lattice.py` can offer only a
single 568 x 185pt `label` cell and the ruled-grid path has nothing to rebuild
from. The column grid then came from `_coverage_gutters`, which calls a 1pt bin
a gutter below 12% of peak coverage. On this page the real gutter between the
left description and the left rate sits at 4–5 runs against a peak of 18 — it is
not empty, because the descriptions run to x 252.96 and two page-wide titles
cross the sheet. All four missing boundaries were bins the histogram called
occupied.

**The fix asks the unambiguous question instead: where does a *cell* start.**
`guides.table_columns` clusters the x at which lines begin cells and keeps a
column only where at least two lines agree. `emit.py` now takes the table grid
from it. `flow` — the dissolved reading columns the prose path uses — still
comes from `_column_bands`, so no prose region moves and no `_is_prose` verdict
changes.

**Three bundles changed and only three**: `2551m-2002`, `0605-1999`,
`extra/2200an-2018` (`git status -- forms/`). 0605's tax-type table and its
two-column Guidelines are now real columns (F168, F169 closed on the same
measurement), and 2200-AN's Schedule 1A now binds `XG021 | Up to P600,000 | 4%`
across three cells where it used to merge all three into one.

**A declared blast radius of two was wrong, and the reason is worth keeping.**
The separate reflow track measured "exactly two bundles change" over a rebuild
that called `emit.main` without `--guide-source`. That flag is what converts a
standalone guide PDF into reflowed text, and it is the only path 2200-AN's
tables come from — so that rebuild never exercised the case that moved. A
blast-radius measurement has to use `batch.py`'s own argv.

## The reflow was silently dropping text, and nothing had ever noticed (F182)

Found by landing the change above, and it is the more serious of the two
defects because it was losing content rather than misplacing it.

`_table_markup` gave each cell a colspan equal to the number of grid columns
its widest run overlaps, then walked the row with `index += span`. When a run
crossed into a column that a **later run on the same line started in**, the walk
stepped straight past that column's index and the cell was never emitted — its
runs left the document, with the row still well formed.

- **How it surfaced:** `emit.py --self-test`, "a converted guide PDF carries
  every run of its own extraction" — `310 runs, 21 missing` — the moment the
  r19 grid made columns narrow enough to expose it. That check has been in the
  file all along; the old grid was simply too coarse to trip it.
- **What was shipping:** `forms/extra/2200an-2018/guide.html` was missing
  **`(To Part III, Item 16)`** — the pointer telling a filer where Schedule 1C's
  total goes — plus two `t` glyphs. Dense-character diff across the fix: **three
  insertions, zero deletions, 13,228 → 13,248**.
- **The fix** clamps a colspan so it cannot reach a column that owns a cell of
  its own. Content is never dropped; only the span narrows.
- **Isolated:** rebuilding 2200-AN with and without the clamp gives identical
  table shapes (79 and 74 rows either way); the clamped build is 836 bytes
  larger. So the shape change is the grid, and the 836 bytes are the text that
  was being lost.
- A new unit assertion in `emit.py`'s self-test now drives the exact shape,
  independent of any corpus form: *a run crossing into an occupied column does
  not swallow its cell*.

## Gate — full clean-tree run r20 (2026-08-07 18:27, `73c3ce4`)

    PASS  self-tests 10 · conversion 53/53 · rules 53/53 · paper 53/53
    PASS  artwork 53/53 · text 53/53 · tracked-files · audit-refresh 53
    PASS  determinism  byte-identical (b5e4f9e1b979, moved from 7a152bc88161 —
                       25 form documents changed and had to. Two generations
                       still compare byte-for-byte)
    FAIL  assertions   inputs_over_printed_text 20/53        (r19: 20, unmoved)
                       comb_slots_match_printed 22/53        (r19: 22, unmoved)
                       inputs_span_no_printed_divider 11/53  (r19: 11, unmoved)
                       printed_box_peers_all_fillable — GONE from this list
                                                        (r19: 14/53)
    FAIL  findings     42/128 blocker+major open  (r19: 55/126)
    UNEV  comb-referee 52/53 forms, 2551Q the only error — identical to r19

**9 of 12, the same three checks red as r19, and one assertion fewer inside
the red one.** `printed_box_peers_all_fillable` no longer appears in the
`assertions` detail at all: that is the full clean-tree gate confirming the
14 → 0 measurement.

**Re-run to confirmation at `e7416c8` (2026-08-07 19:20), after both faults
below were fixed.** Same 9 of 12, same three red, and each red one now reading
its honest value:

    FAIL  assertions   3 of 10 (the same three)
    FAIL  findings     42/128 blocker+major unresolved   (was UNEVALUABLE)
    UNEV  comb-referee report is partial: 52/53 forms    (was 27/53)
    PASS  determinism  byte-identical (b5e4f9e1b979) — the SAME digest as the
                       18:27 run, so the corpus under measurement did not move
                       between them

**Two faults in this run were mine, both self-inflicted, both fixed, and both
worth recording because each cost a 60-minute run:**

1. **`findings` came back UNEVALUABLE, not FAIL** — "finding 184 schema is
   unsupported". `FINDING_KEYS` is an exact set and my two new entries omitted
   `resolution`; their `cause` also has to be one of the declared
   `cause_codes`, which cannot be extended because `cause_codes` is inside the
   immutable-baseline digest. Both now carry `resolution: ""` and `cause: C5`,
   and `gate.py --only findings` reports the honest **FAIL 42/128**.
2. **`comb-referee` fell to 27/53** — "emitted HTML bytes changed from the
   reviewed pin" on 25 forms. `EXPECTED_HTML_STRUCTURE_SHA256` hashes
   **`build/html/<slug>.html`, the emitted document**, and I refreshed it from
   `forms/<slug>/index.html`, the bundled one. Corrected against the right
   artifact and verified by re-running the referee: **52 of 53 forms, one
   error, and it is 2551Q's reviewed control (`p2c5 != 14`) — byte-for-byte
   r19's position.** 2551Q's own documents are unchanged at r20, and the
   reviewed pin was NOT moved. Exactly the same 25 slugs differ at the emitted
   and the bundled level, so the tag/attribute review below covers the right
   population; only the artifact being hashed was wrong.

## The four user-visible checks, looked at rather than counted (r20)

Screenshots in the session scratchpad under `blockers/`, taken with Playwright
against the shipped `forms/` tree over a local static server, 3× device scale.

| Check | Verdict | Evidence |
| --- | --- | --- |
| 0619-E item 3 "Amended Return?" YES is tickable (F152) | **YES** | `F152-0619e-item3-amended-yes.png` — an X typed into the YES box. Input `p1c22-i` in cell `[275.94, 134.49, 289.08, 145.98]`, which is the assertion's offender box `[276.05, 134.64, 289.08, 146.16]` |
| 2550Q item 3 second-quarter box is tickable (F177) | **YES** | `F177-2550q-item3-quarter-2nd.png` — X in the 2nd box, all four quarters present. Input `p1c11-i` at `[470.4, 110.1, 484.1, 122.3]` |
| 2316 item 3 TIN shows 14 character boxes (F111) | **NO — and unchanged** | `F111-2316-item3-tin.png`. The row is one 37.92pt free-text input + combs of 3, 3 and 5 = **12** boxes where the sheet prints 3-3-3-5 = 14. The first group is the uncombed one. Byte-compared against `HEAD:forms/2316-2021/index.html`: the slot census of that row is **identical** — same four containers, same 3/3/5, same widths. F111 stays open; r20 neither fixed nor worsened it. (The finding's "8" is itself stale: HEAD renders 12, not 8.) |
| 0605 "BCS No./Item No. (To be filled up by the BIR)" is not a taxpayer input (F147) | **NO — and unchanged** | `F147-0605-bcs-bir-only.png`. `p1c17`, 254.51 × 18.96pt at (321.61, 185.88), one free-text input, holds the X. Identical at HEAD, same cell, same rect, one input. **The check as posed is false at HEAD too** — the box has always been fillable, which is what F147 (blocker, open) says. Not worse; not better |

## Corpus census — r20

Re-derived from the regenerated `build/layout` and the shipped `forms/` tree.
**Six census pins moved and one of them was already wrong at HEAD** — r19 took
`comb_referee.EXPECTED_COMBS` to 4,538 and left `gate.EXPECTED_COMB_SUBJECTS`
at 4,521, so `validate_comb_referee_report` was comparing 4,538 against 4,521
and could only ever have failed. That is G01 repeating in the same pair of
files one revision later. Both now say 4,543.

| Quantity | r20 | r19 | Note |
| --- | --- | --- | --- |
| Bundles / unique codes | 53 / 50 | 53 / 50 | unchanged |
| Pages | 116 | 116 | |
| Lattice cells | **20,704** (10,050 `field`) | 20,688 (10,002) | +16 cells, +48 `field` |
| Comb ledger subjects | **4,543** | 4,538 | six slugs move: 0605 21→19, 1600WP 16→17, 1604CF 12→15, 2550M 23→21, 2551M 15→18, 2553 16→18 |
| Retained (suppressed) subjects | **21** | 17 | same six slugs |
| Active comb cells | 4,522 | 4,521 | derived, never a literal |
| Emitted inputs | **45,643** | 45,583 | +60, and **nothing deleted** |
| Comb slot divs | **40,017** | 40,008 | +9 |
| Comb slots with no input | 281 | 281 | unchanged — the compartments the source already filled in |
| Form documents changed | **25 of 53** | — | plus `forms/index.html`; **0 guide documents** |
| Assertions demanded by the gate | 10 | 10 | unchanged |
| Findings | **185** | 183 | **42 blocker+major open of 128** (was 55 of 126) — 15 closed on measurement, F184 and F185 filed open |

**The 25 changed documents were reviewed, not rubber-stamped**, before their
`EXPECTED_HTML_STRUCTURE_SHA256` pins were refreshed (the other 28 are
byte-identical and were not touched). Tag inventory moves in one direction —
**+60 `<input>`, +29 `<div>`, +3 `<rect>`, zero elements deleted** — and
visible text is **token-for-token identical in every document**; the only
text-length changes, 2550M +3,767 and 1604CF −1, are entirely inside the
embedded band-data `<script>`. The three new rects were checked against the
sheet rather than counted: all three are 2550M page 2 at x 574.92, the last
three segments of the right-hand column rule, whose mirror at x 452.28 was
already painted at HEAD. Every other rule moved by exactly half its stroke
width at a capped end and by nothing at a butt-capped one.

## Corpus census — r19

**Exactly one census pin moved, it is a guide-document count, and it moved for
a declared reason.** r19 changed `emit.py` (the table grid and the colspan
clamp) and `guides.py` (the new `table_columns` producer and its self-test).
Neither touches the lattice, the IR, the layout or the form document, so every
*form*-side census is unchanged and was predicted to be before the run.
`batch.py` re-converted 53/53; `git status -- forms/` names three guide
documents and nothing else, and `forms/index.html` regenerated byte-identical.

The comb census pin changed shape too, but not because anything generated moved
— see "the ledger denominator" below: `EXPECTED_COMBS` is the *subject*
denominator (4,538) and the *active comb cell* count (4,521) is now derived from
it rather than confused with it. Re-derived from the fresh `build/layout` for
all 53 forms: 4,538 subjects, 4,521 active, 17 retained, and every per-slug
retained pin matches.

| Quantity | r19 | r18 | Note |
| --- | --- | --- | --- |
| Bundles / unique codes | 53 / 50 | 53 / 50 | 38 direct + 15 under `forms/extra` |
| Pages | 116 | 116 | |
| Lattice cells | 20,688 (10,002 `field`) | 20,688 | unchanged — r19 is guide-side only |
| Comb ledger subjects | **4,538** | 4,521 (pin was wrong) | the `EXPECTED_COMBS` denominator: active + `retained_unresolved` |
| Active comb cells | 4,521 | 4,521 | unchanged; now `EXPECTED_ACTIVE_COMBS`, derived, never a literal |
| Retained (suppressed) subjects | 17 | 17 (uncounted) | now pinned per slug in `EXPECTED_RETAINED_SUBJECTS_BY_SLUG` |
| Emitted inputs | 45,583 | 45,583 | unchanged |
| Comb slot divs | 40,008 | 40,008 | unchanged |
| Comb slots with no input | 281 | 281 | the compartments the source already filled in |
| Editable slots on a short pre-printed constant | 0 | 0 | G11's own metric |
| `mixed` cells still carrying an input | 156 of 180 | 156 of 180 | correct — money combs, printed ink is the decimal decoration (C4) |
| Assertions demanded by the gate | 10 | 10 | unchanged at r19 |
| Guide documents changed | **3 of 36** | — | `2551m-2002`, `0605-1999`, `extra/2200an-2018` |
| Findings | **183** | 181 | **55 blocker+major open of 126** (was 59 of 125) — F127/F167/F168/F169 closed on measurement, F182 filed and fixed, F183 filed open |

## The comb referee: four defects in the referee itself, none of them a producer regression (r19)

The referee has scored `UNEVALUABLE` on every run it has ever made. r19 lands
four fixes to the **referee's own** derivation. None of them weakens a check;
three of them make the referee ask for *more* than it did.

**1 — One tolerance was pinned to five relations that do not share it.**
`validate_audit_position_evidence` demanded `HTML_GEOMETRY_EPSILON_PT`
(0.0002) from all five published position relations. Two of them
(`emission_layout_position`, `emission_layout_outer_position`) compare two of
our own four-decimal serialisations and really are exact to 0.0002. The other
three carry `source` in their names and cross into raw source geometry;
`audit.py` binds exactly those three to `POSITION_TOL_PT` (0.25) and documents
why at its declaration, and `comb_referee.py` already carried the same 0.25
under the same name for its own Poppler work. So **every offender the audit has
ever published failed to parse**, was dropped from `dimensions_by_cell`, and the
re-derived partition collapsed to zero — producing the tolerance error plus two
downstream errors. Each relation is still pinned to exactly one fixed constant;
swapping them in either direction is still rejected. `git log -S` shows the code
unchanged since the landing commit `abb0c1e`; the referee's own self-test
fixtures published 0.0002 on all five fields, a record no producer emits, which
is why it never fired.

**2 — The ledger denominator was moved by a count of a different thing.**
r14 measured comb *cells* (4,521) and subtracted the difference from
`EXPECTED_COMBS`, which is the *subject* denominator — compared against
`len(published_subjects)` and `len(cells)`, both of which enumerate
`retained_unresolved` subjects too. A comb that stops being a writing surface
does not leave the ledger; that is the ledger's whole purpose. **Bisected:**
running `21e0630^`'s `lattice.py` over the unchanged `build/ir` for all 53 forms
yields a ledger identical to HEAD's, form for form — 4,538 subjects, 4,521
active, 17 retained. `21e0630`'s shaded-paper fix moved neither census. Exactly
two subjects were genuinely stale (1700-2018, 143 → 141), and they were already
stale before it. The two quantities are now pinned separately and the active
count is *derived*, so they can never be added or subtracted from each other
again.

**3 — Mixed paper was refused outright.** `bind_artifacts` demanded
`paper.uniform is True`, which failed 1604-CF — whose page 3 really is landscape
in the pinned source (`pdfinfo`: 612x1008, 612x1008, **1008x612**, 612x1008).
A form the referee cannot evaluate scores the same as a broken one. The paper
contract is now bound per page against an exhaustive, canonically ordered
`distinct_sizes` inventory, with `uniform` required to be the true derived
claim — **strictly more than the old check asked of the 52 uniform forms**: a
false `distinct_sizes` used to pass and now does not.

**4 — Named `@page` rules read as grammar violations.** A mixed-size document
emits one `@page page-N` per page plus a `.page-N{page:page-N}` binding; the old
contract demanded a single page size outright, so 1604-CF's four correct named
rules read as thirteen violations — and `slot_records` folds
`invalid_bindings` into every cell's `valid`, so all ten of its combs were
published as emission disagreements they are not. A uniform document must now
carry **no** named rules, and a mixed one exactly one per emitted page bound to
that page's own geometry and its own selector.

## The field layer stopped being invisible — G10's first two assertions (r18)

This is the increment's whole point, so its numbers come first.

**Why:** 171 of 172 ledger findings carried `audit_blind: true`. The 51-form
visual sweep found 138 defects and **137 sat on pages this gate scored rules
100% / text 100% / 0 missing / 0 extra**. The two existing assertions that come
closest each take their candidate population from the producer that made the
mistake, so the mistake removes its own members from the population:
`money_boxes_have_inputs` enumerates from `b.layout_cells` and accepts only
`kind == "field"` (a `field` cell with zero inputs occurs **0 times in 9,971** —
that is the mechanism, not a clean bill of health), and
`comb_slots_match_printed` opens with `if b.layout is None` and inventories the
layout's comb subjects. Neither of the two new assertions reads `b.layout`,
`b.plan`, `build/layout/*.json`, emit.py's markers or the IR. Their whole
expectation comes from the pinned PDF's own composited paint stream
(`ordered_vector_paints`) and its own text operators (`drawn_glyph_boxes`),
scored against `input_boxes(cell)` from the emitted DOM.

| New assertion | Forms failing | Offenders | Denominator |
| --- | --- | --- | --- |
| `inputs_span_no_printed_divider` | **11 of 53** | **79** | 44,536 emitted inputs walked |
| `printed_box_peers_all_fillable` | **14 of 53** | **14** | 7,223 printed boxes recovered from the source |

**These are newly-VISIBLE defects, not new defects and not a regression.** Every
one of the 93 offenders was already in the shipped corpus at r14, r15 and r17;
what changed is that a check can now see them. An assertion that catches real
defects on day one is the point of writing it.

**The strongest evidence that they measure the right thing is that they land on
findings a human found by eye, at the same coordinates.** `printed_box_peers_all_fillable`
reports 0619-E's offender at box `[276.05, 134.64, 289.08, 146.16]`; F152, filed
by a reviewer on 2026-08-07, records "the printed box is at (276.0, 135.0)
12.5 x 10.5 pt". 0620 matches F153 the same way. Both are blockers, both were
`audit_blind: true`, and neither is blind any more. `inputs_span_no_printed_divider`
reports 2550M `p1c2` at `[209.28, 90.72, 270.00, 102.48]` spanning three printed
dividers — the case STATUS.md has carried since 2026-08-06 as G02a, diagnosed by
hand against `lineCap`, and until now invisible to every gate check.

**Nine of the 93 offenders were on populations no open finding covered, and the
ledger now carries them: F173–F181** (5 dead checkboxes, 4 comb-spanning input
groups). The rest map onto open findings already filed — F152, F153, F106, F135,
F150, F049/F054/F058/F062, F041, F073, F111, F115, F163, F164, F165, F166 — so
the two assertions independently re-derive 16 existing human findings from the
source PDF alone.

The five dead checkboxes are worth naming because each one makes a legally
required election unstateable:

| Finding | Form | The box that cannot be ticked | Peers on the same printed row that can |
| --- | --- | --- | --- |
| F173 | 1701 | ATC **II016 Mixed Income – 8% IT Rate** | II011, II015, II017 |
| F174 | 1701MS | item 17 spouse **Optional Standard Deduction** | the taxpayer's identical OSD box |
| F175 | 1706 | item 11 International Tax Treaty **No** | item 11 Yes, item 10 Yes/No |
| F176 | 2200M | item 12 Special Law / Treaty **No** | item 12 Yes |
| F177 | 2550Q | item 3 quarter **2nd** | Calendar, Fiscal, 1st, 3rd, 4th |

Each was confirmed against the pinned source's own text operators before it was
filed: the label immediately right of the offending box was re-derived from the
PDF, so "the dead one is the 2nd quarter" is a measurement and not a guess.

### What the two assertions deliberately refuse to say

Both are narrow on purpose, and the narrowness is the reason to trust them.

- A9 counts a divider only when it is dark (tone ≤ 0.5), thin (≤ 1.6pt),
  materially taller than wide, **still visible after the page composites**, more
  than 0.5pt inside BOTH of the input's own edges, and sharing ≥ 1pt of the
  input's height. The visibility clause is not decoration: 2550M draws a comb
  tick and then paints a white 44 × 13pt rectangle over it, and dropping the
  clause inflates the count from 79 to 111 with 32 dividers that are not on the
  printed page at all.
- A9 reports the **input**, not the divider. 2550Q `p1c41` is one 437pt input
  over 30 printed compartments; publishing 30 rows would bury the one defect.
- A10 stays silent on a row where **nothing** is fillable. Such a row may
  legitimately be Bureau-only, and guessing there is exactly what would make the
  assertion untrustworthy. It speaks only when the sheet itself has already said
  these boxes are the same kind of thing, by giving at least one of them an
  input.

### gate.py had to change too, and it was declared in one commit

`gate.REQUIRED_ASSERTIONS` and `gate.BASIC_ASSERTION_COUNT_FIELDS` are exact
allowlists: an assertion name the gate does not know makes the record
`unsupported basic assertion`, and a published count field it does not know makes
it `detail has unsupported fields`. Both grew by two, the synthetic fixture
`_synthetic_audit_record` declares both new keys' counts, and the self-test's
literal `8` became `10` **plus a new invariant** — every non-comb name in
`REQUIRED_ASSERTIONS` must have a declared count contract — so the next agent who
adds an assertion without declaring its contract fails a 3-second self-test
instead of a 60-minute gate. That is G17's lesson paid forward rather than
restated.

`comb_referee.AUDIT_PRODUCER_SHA256` re-pinned `d31b4d7a` → `8d22a957`, with the
reasoning recorded at the constant: the new assertions add derivation the referee
does not adjudicate and touch no existing assertion's code path.

## PT 060 still reads 5%. It is officially 2%. NOT FIXED at r18. (SUPERSEDED by r19 — kept as the record of two failed landings)

**Report this loudly rather than quietly: the guide reflow fix did not land, so
2551M's ATC table still binds the wrong tax rate to the wrong ATC code.** The
work was done and measured in a separate track and was explicitly reported as
`fixed: false`; `guides.py` is byte-identical to r14 at r18 (`git log
-- tools/formgen/guides.py` ends at `1e4da29`, a census pin), and the shipped
bundle proves it.

Measured at r18, two ways that do not share a producer:

| Source | PT 060 |
| --- | --- |
| `forms/2551m-2002/guide.html`, first `gl-table` | description cell `"Franchises on electric utilities, gas and water utility 2% performing quasi-banking functions"`, **Tax Rate column `5%`** |
| Pinned PDF `2551m.pdf` sha256 `f678be68…` (matches `provenance.json`), page 2, PyMuPDF text operators | row y 202.0–210.0 is ONE scanline carrying TWO source rows: `PT 060 … water utility` with its rate **`2%` at x 251.5** in the LEFT rate column, and `PT 112 2) On interest, commissions and discounts paid from their loan … 5%` at **x 549.1** in the RIGHT rate column |

The reflow has no column detection, so it binds the right half's 5% onto the
left half's code. **A reader picking an ATC from this table can file a franchise
tax at 5% where the statute says 2%.**

**F127 is therefore REOPENED**, with the retraction recorded in its own
`resolution` field. The 2026-08-06 closure measured the symptom it named — prose
flattening, which really is gone — and declared the finding fixed while the
association the finding says was destroyed is still destroyed. The assertion that
closed it, `reflow_rate_without_description`, is structurally unable to see a rate
bound to the wrong code: it asks whether a row has a rate and no description, and
this row has both. F167 (blocker) carries the same defect as its own row and
names `guides.py`'s reflow as the owner. The blocker+major count moved 49 → 59
partly for this reason, and a count that goes up because a wrong `fixed` was
retracted is the ledger working.

## Gate — r19 (2026-08-07 15:41, `d3e7a72`, clean tree) — 9 of 12 PASS

**The authoritative run.** Same verdict count as r18 and the same three checks
red. **No regression on any check, and the referee moved a long way.**

| Check | r19 | r18 | Detail |
| --- | --- | --- | --- |
| self-tests | PASS | PASS | 10 modules (11 run by hand, `validate_tree` included) |
| conversion | PASS | PASS | 53/53 unique tracked forms |
| rules | PASS | PASS | clean on 53/53 |
| paper | PASS | PASS | exact on 53/53 |
| artwork | PASS | PASS | clean on 53/53 |
| text | PASS | PASS | clean on 53/53 |
| assertions | **FAIL** | FAIL | `inputs_over_printed_text` **20** (r18: 20); `comb_slots_match_printed` **22** (22); `inputs_span_no_printed_divider` **11** (11); `printed_box_peers_all_fillable` **14** (14). **Not one of the four moved by a single form** — r19 is guide-side and the assertions are form-side, which is the prediction and the confirmation |
| findings | **FAIL** | FAIL | **55/126** blocker+major unresolved (r18: 59/125). Worst: 1701 5, 1701MS 3, 1707 3, 2553 3 |
| tracked-files | PASS | PASS | no tracked deletion |
| audit-refresh | PASS | PASS | fresh audit atomically published for 53 forms |
| determinism | PASS | PASS | byte-identical, **`7a152bc88161`** (r18/r15/r14: `8ceeab9e506d`). The digest MOVED and had to: three guide documents legitimately changed. Two generations still compare byte-for-byte |
| comb-referee | **UNEVALUABLE** | UNEVALUABLE | **52/53 forms, up from 40/53** — the four referee fixes above cleared twelve. One form still does not arrive: **2551Q**, and it is not a form defect. See below |

**The three red checks are the three that were red at r13, r14, r17 and r18.**
The two counts r19 could plausibly have disturbed — the four assertion
populations — did not move by a single form, which is what a guide-side change
should do and is the check that it did nothing else.

### The one form the referee still cannot report, and why the pin stays put

`2551q-2018` raises `RefereeError: 2551Q reviewed control changed: p2c5 != 14`.
`REVIEWED_2551Q_EXPLICIT_COMPARTMENTS` is a **human-reviewed** control: p2c5 was
reviewed as `measured` with 14 compartments. The referee now returns

```
status      unevaluable
reason      source topology does not occupy a strict majority of the full comb band
contract    y 108.26–125.96, span 17.70pt
measured    6.96pt of that span; 10.74pt unmeasured
```

p2c80 (reviewed at 12) is refused the same way, 7.44pt measured of 18.78pt.

**The pin was not moved and must not be.** Moving a reviewed control to match
the producer that stopped satisfying it is the exact failure mode this project
has already paid for twice (`EXPECTED_COMBS` at r14, `HTML_RUNTIME_SCRIPT_SHA256`
at G17). It is filed as **G18**.

**It is not r19's doing.** r19 changed `emit.py` and `guides.py` on the guide
path only; 2551Q's `index.html`, its layout and its IR are byte-identical
(`git status -- forms/` names three guide documents and nothing else), and the
referee's verdict here is derived from the pinned PDF's Poppler geometry and the
layout. What r19 changed is that 2551Q now *reaches* this check — the same
"newly visible, not newly broken" shape as r18's two assertions.

**Reaching PASS is further off than one form.** With 2551Q reporting, the
referee would carry 53/53 and `combs_found` would be the full 4,538 — but
`forms_ok` is **0** and 4,385 of 4,433 subjects are `source_unevaluable`, so the
status would still be UNEVALUABLE. 52/53 buys a complete-corpus *report*, not a
score.

## Gate — r18 (2026-08-07 13:41, `191b683`, clean tree) — 9 of 12 PASS (superseded by r19 above)

**The authoritative run.** Same verdict shape as r17: three red checks, the same
three, for the same reasons on two of them and for one deliberately new reason.
**No regression.**

| Check | r18 | r17 | Detail |
| --- | --- | --- | --- |
| self-tests | PASS | PASS | 10 modules (11 run by hand, `validate_tree` included) |
| conversion | PASS | PASS | 53/53 unique tracked forms |
| rules | PASS | PASS | clean on 53/53 |
| paper | PASS | PASS | exact on 53/53 |
| artwork | PASS | PASS | clean on 53/53 |
| text | PASS | PASS | clean on 53/53 |
| assertions | **FAIL** | FAIL | `inputs_over_printed_text` **20** (r17: 20); `comb_slots_match_printed` **22** (r17: 22); **`inputs_span_no_printed_divider` 11 — NEW**; **`printed_box_peers_all_fillable` 14 — NEW** |
| findings | **FAIL** | FAIL | **59/125** blocker+major unresolved (r17: 49/116). Worst: 1701 5, 0605 4, 2551M 4, 1701MS 3 |
| tracked-files | PASS | PASS | no tracked deletion |
| audit-refresh | PASS | PASS | fresh audit atomically published for 53 forms |
| determinism | PASS | PASS | byte-identical, **`8ceeab9e506d`** — the SAME digest as r14/r15, which is the independent confirmation that no generator moved |
| comb-referee | **UNEVALUABLE** | UNEVALUABLE | 40/53, **exactly the r17 residue and no more**: `source frame/unframed partition is false` + `form audit relation contains errors` on 1604C, 1700, 1701MS, 1702EX |

**Neither pre-existing assertion count moved by a single form.** That is the
result to read twice: adding two assertions to `audit.py` did not perturb the
eight already there, and the determinism digest is character-for-character the
r14/r15 value, so the corpus under measurement is provably the same corpus.
The two new red rows are the two new assertions doing their job on day one.

**The referee's UNEVALUABLE is unchanged and still undiagnosed.** r17 named
1604C, 1700, 1701MS and 1702EX; r18 names the same four and nothing else. This
increment neither cleared it nor worsened it, and it was not expected to —
nothing here touches the referee's derivation. It stays open as G16's shadow
plus whatever the `source frame/unframed partition` complaint turns out to be.

## Gate — r14 (superseded by r18 above)

Runs r14 (04:22, `8defe23`) and **r15 (05:35, `e38672f`)**, both complete clean-tree
runs. **9 of 12 PASS** in both. r15 is the authoritative one.

| Check | r14 | r13 | Detail |
| --- | --- | --- | --- |
| self-tests | PASS | PASS | 10 modules |
| conversion | PASS | PASS | 53/53 unique tracked forms |
| rules | PASS | PASS | clean on 53/53 |
| paper | PASS | PASS | exact on 53/53 |
| artwork | PASS | PASS | clean on 53/53 |
| text | PASS | PASS | clean on 53/53 |
| assertions | **FAIL** | **FAIL** | `inputs_over_printed_text` 20 forms (was 40); `comb_slots_match_printed` 36 forms (was 22) — see below |
| findings | **FAIL** | **FAIL** | 49/116 blocker+major open (was 58/116) |
| tracked-files | PASS | PASS | no tracked deletion |
| audit-refresh | PASS | PASS | fresh audit atomically published for 53 forms |
| determinism | PASS | PASS | byte-identical (`8ceeab9e506d`) |
| comb-referee | **UNEVALUABLE** | UNEVALUABLE | 40/53. r14's cause (a third stale pin) is fixed; r15's residue is G16's shadow — see below |

The verdict shape is unchanged from r13: the same three checks are red, for
reasons that moved in the intended direction on two of them. r14 is the first
full gate run on this branch.

### The referee's UNEVALUABLE, and the pin nobody had counted

r14 reported `form emission binding has errors` on 0619E, 0619F, 0620, 1600-PT
and 1600-VT, with the payload reason **"HTML runtime scripts disagree with the
reviewed emitter"**. That is `comb_referee.HTML_RUNTIME_SCRIPT_SHA256`, a
**third** reviewed emitter pin — separate from `EXPECTED_HTML_STRUCTURE_SHA256`
and from the producer SHAs, read **only by the referee, which runs last**. Two
of its three hashes moved, and exactly the two the G11 fix claims to touch: the
field runtime (`positionOf` replacing attribute-indexed comb navigation, which
would otherwise stop advancing at the first printed compartment) and the field
debug overlay (F172). The band-data runtime is byte-identical, which is the
standing evidence that none of this reached page scaffolding. Re-pinned after
r14 with that reasoning recorded at the constant, and **r15 settled it**: all
five `form emission binding has errors` entries are gone and no
runtime-script complaint remains. Cost of finding it this way: one 60-minute
run, and one more to clear it. A referee-only re-run cannot substitute — it
reports `audit application envelope is stale`, because changing `comb_referee.py`
invalidates the envelope the previous run bound.

### What is left of the referee's UNEVALUABLE at r15

The report is still partial at 40/53, and every remaining complaint is
`form audit relation contains errors` (1600-PT, 1600-VT, 1604C, 1604E, 1621) or
`form audit source frame/unframed partition is false` (1604C, 1700). Those fire
on `audit_evidence.assertion_valid is not True`, and every named form is one
where compartments are now correctly refused. **The referee is UNEVALUABLE
because `comb_slots_match_printed` fails, so this is G16's shadow, not a second
defect.** Closing G16 is expected to close it; nothing else should be attempted
here first.

## The two assertions, and one of them got worse

Measured over the r14 corpus with the full `audit.py`:

| Assertion | r14 | r13 | Move |
| --- | --- | --- | --- |
| `inputs_over_printed_text` | **20 forms / 149 offenders** | 40 / 258 | **−20 forms, −109 offenders** |
| `comb_slots_match_printed` | **36 forms / 247 offenders** | 22 / 186 | **+14 forms, +61 offenders** |

The second move is a regression in the number and is **not** a regression in
the emitted forms. Splitting the 247 by the state the audit itself reports:

| `emission_state` | offenders | what it is |
| --- | --- | --- |
| `physical-slots` + `source-topology-unevaluable` | 167 | pre-existing; the audit could not evaluate the source's own comb topology. 2000-DST 30, 2200A 25, 2200P 25, 2200C 24 — none of these bundles lost an input at r14 |
| `slot-input-index-mismatch` + `invalid-emission` | **76** | **new, and caused by the G11 fix**: the audit requires a comb's input indexes to run 0..N−1 with no gap, and a refused compartment is exactly such a gap |

`audit.py` was **not** changed to accommodate this, and must not be. The
assertion is asserting an emission contract that the G11 fix deliberately and
correctly broke — a compartment the source already filled in must not carry an
input — and the fix for the number is to teach `audit.py` the new contract by
re-deriving the constant from the SOURCE PDF's own text operators, which is
where that assertion already reads from. That work is **not done**; it is
recorded as **G16** in PLAN.md. Until it is, 76 of the 247 offenders are the
check disagreeing with a change it has not been told about, and 167 are the
pre-existing population.

## Census pins were stale at HEAD, again (fixed at r14)

`comb_referee.EXPECTED_COMBS` and `gate.EXPECTED_COMB_SUBJECTS` both read 4540.
Re-running the **HEAD (21e0630)** lattice over the unchanged IR produces
**4,521**, so the pins went stale in 21e0630 itself — its shaded-paper fix
stopped 19 cells across 13 forms from being writing surface, and therefore from
being combs, without the census moving with it. This is the G01 landmine
repeating one commit later and it would have failed r14 on its own constants
after 60 minutes. Both pins, the 13 per-slug values, and `guides.py`'s
`("2550m-2007", 3)` field-cells-below expectation (1 → 0) moved at r14, each
with a comment naming its cause. `comb_referee`'s own self-test had the same
number as a **literal**; it now derives it from the pin, so that copy cannot
drift again.

## Painted walls now bound cells (this increment)

The user's complaint — a fillable box that does not fill its printed box, "the
yellow box isn't the full width", "no yellow box here" — is a boundary that the
cell grid never saw. `extract.py` files a filled rectangle as a rule only up to
`MAX_RULE_THICKNESS_PT` (1.5) and calls anything heavier an **area fill**;
`lattice.build_page` built `x_lattice` from `page["rules"]` alone, so a table
side painted as a 1.92pt rectangle never became a column. `2550M` page 2 paints
its sides at x 20.16–22.08 and 590.04–591.96 exactly that way.

The asymmetry that named the fix: `comb_boundary_candidates` had **always**
ingested structural area fills, but only for the comb path. `wall_boundaries`
(lattice.py, next to it, same fill-to-candidate shape) now feeds them to the
cell grid too, filtered by `MIN_WALL_ASPECT = 5.0`. `MAX_RULE_THICKNESS_PT` is
untouched: a wall never becomes a rule, never enters `split_verticals`, never
enters the decorative tests. **Verticals only** this increment — a horizontal
wall moves row boundaries and the growable bands measured from them.

The discriminator is measured, not guessed. Over the corpus the 997 vertical
structural fills form two populations that do not overlap on any of three
measurements: 944 **in-field dividers** (2000-OT's TIN group separators, 1707's
2.16pt marks) at aspect 2.28–4.56, and 53 **walls** at aspect 5.50–514.27.
Aspect decides because it is the scale-free measurement.

### 2550M page 2, Schedule 1 — measured against the printed grid

| | before | after | printed |
| --- | --- | --- | --- |
| page-2 `x_lattice` | 13 lines, 77.04 → 523.20 | **15 lines, 21.12 → 591.00** | walls at 21.12 / 591.00 |
| Schedule 1 col 1 (`p2c0/4/8`) | x 77.04–248.16 (171.12pt) | **x 21.12–248.16 (227.04pt)** | 22.08–248.16 = 226.08pt |
| Schedule 1 col 4 (`p2c3/7/11`) | x 448.32–523.20 (74.88pt) | **x 448.32–591.00 (142.68pt)** | 448.32–590.04 = 141.72pt |
| Schedules 6 & 8 right strip 523.20–591.00 | no cell, no input | **`p2c33/41/49`, `p2c58/66/74`** | printed column |
| inputs emitted on page 2 | 101 | **128** | — |

Emitted width exceeds printed width by 0.96pt on each side because a cell snaps
to the wall's **centre**, exactly as it snaps to a rule's centre; `emit.field_box`
then insets by the border thickness. Rasters of the before/after are in the
session scratchpad — this was checked by eye, not only by number.

### Corpus effect

Six forms changed geometry (`1604cf-2008` 111 cells, `2550m-2007` 58,
`1600wp-2010` 28, `2551m-2002` 19, `2316-2021` 6, `0605-1999` 4); seven bundles
changed bytes. 131 field cells **widened** to a painted wall, 95 field cells were
**newly created** on surface that previously had no cell at all, and 11,730pt of
writing-surface width was reclaimed.

A wall-specific census — field cells with ≥10pt of writing surface between the
cell edge and the painted wall that bounds their rows, with no lattice line in
between — moves **199 cells → 90** (7 forms), and the total lost strip width
halves, 9,938pt → 5,469pt. This instrument is *not* the 230-cell/22-form census
from the brief: that one counted all input-vs-printed-box mismatch causes,
of which thick walls were the largest population. The residual 90 sits mostly on
`1604cf-2008` (38) and `2550m-2007` (30) and is the horizontal half plus causes
this increment did not address.

43 previously-`field` cells became `label`. Every one is a narrow left-margin
strip (e.g. `1604cf-2008` p1c33, x 57.84–72.72, empty, 0 text runs) that merged
leftward to its painted wall and absorbed the printed row label already sitting
at x 30.24. Those cells were emitting an input over the right half of a label
box; not emitting one there is the correct outcome, not a lost field.

## Comb dividers lost to stroke caps (this increment — diagnosis only, no code change)

The user's complaint is that a fillable box does not occupy the printed box.
The measured instance: 2550M item 1's YYYY group prints **four** compartments,
but `p1c2` (x 208.56–270.72) emits **one** free-text input. It still does —
`kind: "field"`, no comb — because this increment changed no code.

**The tolerance fix proposed for it was refused, and refusing it was correct.**
The proposal was to make `lattice.supported_at` (lattice.py:183-187) apply
`CLUSTER_TOL_PT` on the y axis as it already does on x. Measured over the
corpus, a symmetric 0.30 flips 138 borders to combs **and 45 combs to borders**,
and does not fix 2550M anyway: its gap is 0.36. The gap histogram has a dead
zone — 0.01/0.09/0.10/0.12/0.18/0.25, then nothing until 0.34/0.35/0.36 — so no
threshold both reaches 0.36 and avoids comb→border flips (even 0.01 flips 37).
The legitimacy test in the brief is also unmet: the x test compares a point
against a horizontal's **length** endpoints, the y test against its
**thickness** band — different classes of measurement. lattice's own precedent
for y-support slack is `supported_near`, which uses `JOIN_EPSILON_PT` (0.05).

**The real cause is in extract.py, and it needs no tolerance at all.** Verified
against the pinned source (`bir2550m.pdf`, sha256 `9fb4101a…`, matching
`provenance.json`): the three ticks are stroked with `lineCap = (1,1,1)` —
**round** — at width 0.72. A round cap paints half the line width past each
path endpoint, so the ticks' real painted extent is y 99.24–102.84, and h4's
ink top edge is **exactly** 102.84. The strokes touch; the 0.36 "gap" is an
artefact of reading the path instead of the ink.

extract.py never consults the cap: there is no `lineCap`/`J` reference in the
file. Its stroke-to-rect conversion applies `half = width / 2.0` to the
**thickness** axis only — visible in the IR, where v177 spans x 223.08–223.80
(0.72 wide, half-width each side of 223.44) while its y stays the bare path
99.60–102.48. The two sites are extract.py:382 (`re` ops) and extract.py:1571
(`l` ops — the one that draws these ticks), where `near`/`far` get `half` and
`start`/`end` get raw `min`/`max`.

The fix is to extend a stroke's **length** by `width/2` when `lineCap` is 1
(round) or 2 (projecting square), and not at all for 0 (butt). That is
honouring the official geometry rather than relaxing a check — strictly more
faithful, form-code-agnostic, and it makes the 2550M contact exact instead of
approximate. It is a producer change to extract.py, so it re-pins
`AUDIT_DEPENDENCY_SHA256` and invalidates every downstream digest.

Not yet measured: how many of the 626 both-endpoints-unsupported borders have a
round or projecting cap, and therefore how far this moves
`comb_slots_match_printed`. That is the next increment's first measurement.

## The comb writing surface (previous increment)

`lattice.comb_bands` published the divider **tick** band as the comb's own
vertical extent. The tick is a guide mark under the writing box, not the box:
on 2550M's item-4 TIN row the cell walls span the full 15.60pt of the row
while the digit separators are 3.12pt stubs along its bottom edge.
`comb_writing_surface` now reports the owning cell's printed walls inset by the
cell's own border thicknesses, the same inset `emit.field_box` gives a plain
text field. The tick band stays published as `divider_band_y0`/`y1`/
`height_pt`.

| Measured over `build/layout` | before | after |
| --- | --- | --- |
| comb cells with a writing box under half their own cell | 4,474 of 4,522 | **0** |
| comb cells with a writing box outside their own cell | 225 | **0** |
| 2550M `p1c9` (item-4 TIN) slot height, in a 15.60pt cell | 3.12pt | **14.16pt** |
| 2550M `p1c9` fitted face | 2.81pt | **8.25pt** (the sheet's modal body size) |

The change is vertical only, and the regenerated bytes prove it: 0 slot counts,
0 pitches and 0 slot X positions moved anywhere in the corpus, and no comb count
moved on any bundle, so `EXPECTED_COMBS_BY_SLUG` needed no re-pin.

## Failing assertions (corpus-wide `--assertions-only`)

| Assertion | r6 forms / offenders | r7 forms / offenders | Movement |
| --- | --- | --- | --- |
| `inputs_over_printed_text` | 40 / 239 | 40 / **258** | **worse by 19 offenders**, same 40 forms |
| `comb_slots_match_printed` | 22 / 186 | 22 / 186 | unchanged, identical form set |
| `money_boxes_have_inputs` | 0 / 0 | 0 / 0 | holds |
| `rules_below_guide_cut` | 0 / 0 | 0 / 0 | holds |
| `run_colour_matches_ir` | 0 / 0 | 0 / 0 | holds |
| `reflow_rate_without_description` | 0 / 0 | 0 / 0 | holds |
| `image_transform_applied` | 0 / 0 | 0 / 0 | holds |
| `no_invented_codepoints` | 0 / 0 | 0 / 0 | holds |

**`inputs_over_printed_text` got worse, and it is the writing-surface fix that
did it.** 17 comb cells newly overlap a printed run and 1 stopped; every one of
the 24 new offender records is a comb cell, and the overlapped runs are the
captions those cells carry in their upper half (`2316-2021` p1c27-30 "Date of
Birth"/"(MM/DD/YYYY)", `2551m-2002` p1c76/81/88 "28C"/"29B"/"30C",
`2553-1999` p1c71/76/83, `2550m-2007` p1c91-93 "Debit Memo",
`1604cf-2008` p1c16/21 "Telephone No."/"Zip Code", `1600wp-2010` p1c24,
`1701ms-2024` p1c183).

The cause is real and is named rather than absorbed: in those cells the lattice
rectangle spans caption **and** comb (e.g. `2551m-2002` p1c76 is 18.65pt tall
with its ticks in the bottom 2.88pt), so "the whole cell inset by its borders"
reaches text that the 3.12pt band never touched. Both readings are wrong for
these cells; the new one is wrong in the direction where a taxpayer can
actually type. No gate verdict changed on it — the check failed on 40 forms
before and after — but the offender count is a debt, and the fix belongs in
lattice.py's cell segmentation, not in relaxing the assertion.

## Findings ledger (`review-findings.json`, 183 findings) — r19

| Severity | Open | Resolved | Total |
| --- | --- | --- | --- |
| blocker | 15 | 22 | 37 |
| major | 40 | 49 | 89 |
| minor | 39 | 4 | 43 |
| cosmetic | 12 | 2 | 14 |

The gate counts blocker+major only: **55 open of 126** at r19 (r18: 59 of 125).

Moved this increment, each on a measurement recorded in the finding's own
`resolution`:

| Finding | Severity | r18 | r19 | On what |
| --- | --- | --- | --- | --- |
| F127 | major | open (reopened) | **fixed** | 2551M: 0 of 15 ATC codes carried their official rate → 15 of 15 |
| F167 | blocker | open | **fixed** | same measurement; the 19x4 table is 19x6 and PT 060 reads 2% |
| F168 | major | open | **fixed** | 0605 tax-type table: `QP \| QUALIFYING FEES-PAGCOR \| VT \| VALUE-ADDED TAX \| \| WG \| WITHHOLDING TAX - VAT AND OTHER`, checked against the official page 2 |
| F169 | major | open | **fixed** | 0605 Guidelines: the TIN branch-code rule is two cells per row, one per source column, instead of one zipped sentence |
| F170 | minor | open | **open** | re-measured, not carried forward: the ATC region is still two tables and the 3-line header section cannot reach `MIN_COLUMN_SUPPORT` |
| F182 | major | — | **filed and fixed** | the reflow was dropping text; 2200-AN shipped without `(To Part III, Item 16)` |
| F183 | minor | — | **filed open** | 2551M's left `Tax Rate` label sits at x 237.60 against its column edge of 251.52 |

## Comb referee — the state below is r13's and is SUPERSEDED by the r19 section near the top

The `EXPECTED_HTML_STRUCTURE_SHA256` pins were refreshed at r14, which is what
first let any form reach `audit_evidence` — and that is what made the r19
tolerance defect visible. Read "The comb referee: four defects in the referee
itself" above for the current picture; the paragraphs below are kept as the
record of what was believed at r13.

`EXPECTED_HTML_STRUCTURE_SHA256`'s 53 reviewed pins remain stale and were **not**
touched: re-pinning them is a user-review action (see `GOAL.md` `## Blocked`).
The producer pin that *is* an agent's to maintain was refreshed this increment:
`LATTICE_PRODUCER_SHA256` `9aeedba0` → `cc32ca68` for `wall_boundaries`. The
other three are unchanged and still match (`audit` `7c902be9`,
`extract` `5f75f191`, `verify` `8dbeb222`).

### Open integration question for the referee (not acted on)

`comb_referee.classify_band` reads `comb["y0"]/["y1"]` as the **source divider
band** — it seeds the open-compartment search and the attached-external-band
retry from them. `emitted_geometry_contract` reads the same two keys as the
**emitted writing box**, which is what emit.py lays out. One field now answers
two different questions. The emission side stays correct automatically; the
source side should read `divider_band_y0`/`divider_band_y1`, which lattice.py
publishes for exactly this purpose.

This was deliberately **not** changed here: the referee is the adjudicator, and
editing its derivation in the same increment as the producer change it
adjudicates is the pattern `GOAL.md`'s user decision 1 forbids. It is inert
today because the check is already UNEVALUABLE on the blocked HTML pins, but it
must be settled before the referee can score again.

## CI

Unchanged since the last measurement: the formgen job went green for the first
time on 2026-08-05 (run 31040386488). The commits in this increment have not
yet been through a CI cycle.

## Open issues, diagnosed

| Issue | State | Root cause / residual | Owner |
| --- | --- | --- | --- |
| `inputs_over_printed_text`: 40 forms / 258 offenders | worse by 19 this increment | 17 new offenders are comb cells whose lattice rectangle spans caption + comb, so the full-height writing surface reaches the caption. Residual populations A/B1/C1/C2 per the 2026-08 triage are unchanged. | `lattice.py` cell segmentation |
| `comb_slots_match_printed`: 22 forms / 186 offenders | unchanged | `printed_compartments` reads only the cell rectangle and no member of the lattice `comb` object, which is why a vertical-only change cannot move it. Residual: the still-refused U-frame-crop / corridor-absorb topologies plus genuine geometry defects. | `audit.py` topology chooser |
| Comb dividers filed as borders (2550M `p1c2` = 1 input where 4 print) | diagnosed, not fixed | extract.py ignores `lineCap`, so a round-capped tick's ink (which reaches its baseline exactly) is recorded 0.36pt short and `supported_at` correctly finds no support. Widening the tolerance was measured, refuted and refused. | `extract.py` stroke-to-rect (382, 1571) |
| comb-referee UNEVALUABLE | user-blocked | 53 reviewed HTML structure pins stale; plus the `classify_band` source/emission key collision above. | user review, then `comb_referee.py` |
| findings: 26 blocker+major open | unchanged | comb capacity (referee track), inputs-over-text populations, guide-cut orphan policy, text mis-position, individual re-verifications | per-cause owners |

## Gate r13 (2026-08-06, HEAD d74771e) — 9/12 PASS

    PASS  self-tests · conversion 53/53 · rules 53/53 · paper 53/53
    PASS  artwork 53/53 · text 53/53 · tracked-files
    PASS  audit-refresh (53 forms) · determinism byte-identical 5103254450db
    FAIL  assertions   inputs_over_printed_text 40 forms; comb_slots_match_printed 22 forms
    FAIL  findings     26/84 blocker+major
    UNEV  comb-referee 0/53 — the 51 reviewed HTML pins are stale (USER-BLOCKED)

Same three failures as r8, with the painted-wall boundaries landed. No
regression from a change that widened 131 cells and created 95.

**The result that needs explaining:** neither assertion moved. 95 new field
cells and 43 field->label conversions produced zero change in
inputs_over_printed_text (40 forms) or comb_slots_match_printed (22 forms).
A number that does not move when it should is worth as much suspicion as one
that moves wrongly — either the assertions do not measure what the fix changed,
or the fix's cells are landing outside their scope. Not yet diagnosed.
