# STATUS — formgen, measured state

**Update rule: any commit that moves a number below updates this file in the
same commit.** This is the only formgen document allowed to hold measured
status numbers (`GOAL.md` owns that rule; `README.md` owns the process).

Measured 2026-08-07 over the 53-form corpus, on branch `gol/form-correction`,
regenerated at the r14 producer bytes. Assertion counts are from a corpus-wide
`audit.py` run over that regeneration; the findings tally is recomputed from
`review-findings.json`.

## Corpus census — r14

| Quantity | r14 | previous | Note |
| --- | --- | --- | --- |
| Bundles / unique codes | 53 / 50 | 53 / 50 | 38 direct + 15 under `forms/extra` |
| Pages | 116 | 116 | |
| Lattice cells | 20,688 (10,002 `field`) | 20,797 (10,401) | 51 cells `field` → `shaded` |
| Comb cells | **4,521** | 4,540 (pinned) / 4,521 (actual) | the pin was stale AT HEAD; see below |
| Emitted inputs | **45,583** | 45,915 | −332 |
| Comb slot divs | 40,008 | 40,008 | unchanged — slots stay, inputs go |
| Comb slots with no input | **281** | 0 | the compartments the source already filled in |
| Editable slots on a short pre-printed constant | **0** | 175 | G11's own metric |
| `mixed` cells still carrying an input | 156 of 180 | 180 of 180 | correct — money combs, printed ink is the decimal decoration (C4) |
| Findings | 172 | 172 | **49 blocker+major open of 116** (was 58 of 116) |

## Gate — r14

Run r14, 2026-08-07 04:22, clean tree at commit `8defe23`. **9 of 12 PASS.**

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
| comb-referee | **UNEVALUABLE** | UNEVALUABLE | 40/53; `HTML_RUNTIME_SCRIPT_SHA256` was a THIRD stale pin — see below |

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
r14 with that reasoning recorded at the constant; **the re-pin is NOT covered by
an r14 verdict** and needs the next full gate run to settle. Cost of finding it
this way: one 60-minute run.

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
