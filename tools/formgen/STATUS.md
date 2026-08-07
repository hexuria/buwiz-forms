# STATUS — formgen, measured state

**Update rule: any commit that moves a number below updates this file in the
same commit.** This is the only formgen document allowed to hold measured
status numbers (`GOAL.md` owns that rule; `README.md` owns the process).

Measured 2026-08-07 over the 53-form corpus, on branch `gol/form-correction`,
regenerated at the r18 producer bytes. Assertion counts are from a corpus-wide
`audit.py` run over that regeneration; the findings tally is recomputed from
`review-findings.json`.

## Corpus census — r18 (nothing moved, and that was the prediction)

**No census pin moved this increment, and none should have.** r18 changed
`audit.py` (two new assertions), `gate.py` (their names and count contracts) and
`comb_referee.py` (the audit producer pin). None of those three is a generator:
`extract.py`, `lattice.py`, `guides.py` and `emit.py` are byte-identical to r14.
`batch.py` re-converted 53/53 and `git status -- forms/` is empty — every one of
the 53 bundles, plus `forms/index.html` regenerated from the fresh batch report,
came out byte-identical. The prediction was made before the run and is recorded
here because a census that moves when nothing generative changed would be the
defect, not the reassurance.

| Quantity | r18 | r14 | Note |
| --- | --- | --- | --- |
| Bundles / unique codes | 53 / 50 | 53 / 50 | 38 direct + 15 under `forms/extra` |
| Pages | 116 | 116 | |
| Lattice cells | 20,688 (10,002 `field`) | 20,688 | unchanged |
| Comb cells | 4,521 | 4,521 | unchanged; pin still matches |
| Emitted inputs | 45,583 | 45,583 | unchanged |
| Comb slot divs | 40,008 | 40,008 | unchanged |
| Comb slots with no input | 281 | 281 | the compartments the source already filled in |
| Editable slots on a short pre-printed constant | 0 | 0 | G11's own metric |
| `mixed` cells still carrying an input | 156 of 180 | 156 of 180 | correct — money combs, printed ink is the decimal decoration (C4) |
| Assertions demanded by the gate | **10** | 8 | `audit.ASSERTION_KEYS` and `gate.REQUIRED_ASSERTIONS` both |
| Findings | **181** | 172 | **59 blocker+major open of 125** (was 49 of 116) — 9 appended, 1 reopened; see below |

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

## PT 060 still reads 5%. It is officially 2%. NOT FIXED at r18.

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

## Gate — r18 (2026-08-07 13:41, `191b683`, clean tree) — 9 of 12 PASS

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

## Findings ledger (`review-findings.json`, 138 findings)

| Severity | Open | Resolved |
| --- | --- | --- |
| blocker | 5 | 16 |
| major | 21 | 42 |
| minor | 36 | 4 |
| cosmetic | 12 | 2 |

The gate counts blocker+major only: **26 open of 84**, unchanged this
increment.

## Comb referee — still UNEVALUABLE, still user-blocked

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
