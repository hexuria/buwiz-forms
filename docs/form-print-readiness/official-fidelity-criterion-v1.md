<!--
Design specification produced by an adversarially-verified investigation on
2026-07-20. STATUS: SPECIFIED, NOT IMPLEMENTED, NOT PROMOTABLE.

This document exists because the >=1% complete-page gate was proven unreachable
for 2551Q:2018 for a structural reason (the official PDF does not embed its
fonts, so the pinned reference encodes Poppler's substituted outlines). See
priority-forms-readiness.md for that finding.

Nothing in this document weakens the existing 1% complete-page gate. That gate
remains in force and the complete-page percentage remains a mandatory,
never-hidden number.
-->

# `official-fidelity-v1` — Release Criterion Specification

**Status: SPECIFIED, NOT YET PROMOTABLE.** Section 8 lists four blocking preconditions that are unsatisfied today. Implement the criterion; do not let it gate a release until those are closed.

---

## 0. Honest verdict, stated first

A tolerance-based criterion **does exist and is worth adopting**, but not in the shape any of the four characterization agents proposed, and **not as a fidelity proof**.

Three things the evidence establishes beyond argument:

1. **No page-global scalar is viable.** A removed comb field scores page-global edge F1 `0.999837` — *better* than the `0.998313` cross-rasterizer floor. A form with every field deleted would outscore rasterizer noise. Page-global scope is disqualified at every radius.
2. **The tolerance metric is a displacement detector, not a fidelity measure.** It has 3–4 orders of magnitude of margin on translation, scale, ink-weight and deletion. Its signal on content correctness is negligible and must never be relied upon: every statutory tax rate on 2551Q page 2 replaced with a wrong value scored a max per-region regression of `0.19e-4` and passed every existing assertion. That is not a tunable weakness; it is what the metric measures.

   > **Implementation amendment (2026-07-20, measured).** With the shipped cell table — named regions **plus** the two 64px offset grids added in §1.2 to close the coverage attack — an in-place digit substitution on the ATC rates measures `1.07e-2`, roughly 500× the region-only figure and above `M_CELL`. Smaller cells raise text sensitivity as a side effect of the coverage construction. **This changes nothing about the doctrine.** Incidental detection is not reliable detection: it scales with how much of a cell the changed glyph occupies, so a same-width substitution inside a large cell can still vanish. `packages/form-renderer/visual/tools/fidelity-injection.spec.ts` therefore classifies content defects as `content-assertion`, asserts only the by-construction property (the structural stratum stays bit-identical, measured `0.000e+0`), and records the incidental cell movement without ever letting it substitute for `static-text-exhaustive-v1`.
3. **The criterion that follows is a NON-REGRESSION gate, not a parity gate.** It is pinned to a reviewed baseline. It certifies "no worse than a reviewed state". It can never certify "matches the official form". Every evidence document must say this in those words.

Therefore `official-fidelity-v1` is defined as a **composite** of five bound components plus mandatory diagnostics. The tolerance metric is one component. It is not permitted to stand alone, and the spec below makes standing alone structurally impossible: the audit fails closed if any component is absent.

---

## 1. The metric

### 1.0 Naming

| id | role |
|---|---|
| `official-fidelity-v1` | the composite release criterion (evidence `gate` value) |
| `cell-edge-f1-v1` | tolerance component — displacement/weight detector |
| `structural-ink-coverage-v1` | rule/border/box geometry component |
| `page-ink-budget-v1` | absolute ink and paper-substrate component |
| `official-complete-page-v2` | **retained unchanged**, demoted to bounded diagnostic (§7) |
| `static-text-exhaustive-v1` | content-correctness component (non-pixel) |
| `encoded-artwork-integrity-v1` | barcode payload component (non-pixel + hash) |

All six components must be present and passing. `official-fidelity-v1.passed` is their conjunction. There is no weighting, no averaging, no "worst-of" aggregation across components.

### 1.1 Shared primitives — bit-exactness rules

These must produce **identical integers** in TypeScript and Python. Every rule below is load-bearing; two prior investigations independently hit disagreements here.

**P1 — Decode.** RGBA8, non-interlaced. Both implementations read the same hashed artifact bytes. Assert `expected.width === actual.width && expected.height === actual.height`; throw otherwise. No colour management, no premultiplication.

**P2 — Composite-over-white luminance `L`.** For pixel index `i`, offset `o = 4*i`:

```
a = data[o+3] / 255
r = data[o+0] * a + 255 * (1 - a)
g = data[o+1] * a + 255 * (1 - a)
b = data[o+2] * a + 255 * (1 - a)
L = 0.2126 * r + 0.7152 * g + 0.0722 * b
```

**Storage type: IEEE-754 binary64 on both sides.** Change `grayscale-edge-match.ts` from `Float32Array` to `Float64Array`.

> This is a *correctness requirement*, not an optimization, and I am deliberately overruling the prior recommendation to emulate `Float32Array` in Python via `array('f')`. Both routes were proven to reproduce; binary64 is chosen because emulating a JS typed-array precision detail pins an implementation artifact into the audit forever, and any future refactor of that one line silently desynchronizes the two implementations with no test able to see it. Binary64 is the native type in both languages and the expression is evaluated left-to-right as `((0.2126*r) + (0.7152*g)) + (0.0722*b)` under IEEE rules in both. Expect the existing non-promotional edge numbers to shift in the 7th decimal on this change; that is expected and the diagnostic is being re-baselined regardless.

**P3 — Sobel edge mask `E`.** For `1 <= y <= h-2`, `1 <= x <= w-2` only. Border pixels are never edges (pin this: it matches today's loop bounds).

```
gx = -tl + tr - 2*l + 2*r - bl + br
gy = -tl - 2*t - tr + bl + 2*b + br
E[y*w+x] = 1 if (gx*gx + gy*gy) >= EDGE_THRESHOLD_SQUARED else 0
```

with `EDGE_THRESHOLD_SQUARED = 2304.0` (= 48², exactly representable). **`Math.hypot` is forbidden.** It is a scaled libm-class routine and is not bit-reproducible against Python's `math.hypot`. Prior agents found the two agreed *on this data*; that is luck, not a guarantee. The squared form is provably deterministic. Expression order as written, left-to-right.

**P4 — Dark-ink mask `I`.**

```
I[i] = 1 if data[o] < 160 and data[o+1] < 160 and data[o+2] < 160 else 0
```

Raw channel comparison, **alpha deliberately ignored** (this matches the existing `darkInkMask`). Pure integer. The artifacts are fully opaque so this is immaterial in practice, but the rule is pinned so it cannot drift.

**P5 — Tolerance neighbourhood: Euclidean-disc dilation via explicit offset list.**

```
OFFSETS(r) = [(dx,dy) for dy in -r..r for dx in -r..r if dx*dx + dy*dy <= r*r]
dilate(M, r): D[i]=0; for each p with M[p]==1, for each (dx,dy) in OFFSETS(r),
              if target in bounds: D[target]=1
```

`|OFFSETS(1)| = 5`. Pure integer, order-independent (boolean OR), trivially identical in both languages.

> I am overruling the prior recommendation of a Felzenszwalb exact squared-EDT. Its sole advantage was yielding every radius for one fixed cost; §2 pins the radius at **1**, so that advantage is worth nothing. Against that, the EDT's parabola-intersection step requires either float division or careful rational cross-multiplication, and it is the single most likely place for two independent implementations to diverge. Disc dilation at r=1 is ~0.4 s per mask — *cheaper* than the 1.7 s EDT — and is byte-identical to the `dilate` already in `grayscale-edge-match.ts`, so this primitive is a drop-in with zero semantic change. Simplicity here is a correctness property.

**P5a — Unification note.** `dilateMask` in `official-page-diff.ts` is a **square** (Chebyshev) dilation. `dilate` in `grayscale-edge-match.ts` is a **disc**. `official-fidelity-v1` uses the **disc form everywhere, no exceptions**. This changes `expectedInkMissingPercent` / `unexpectedActualInkPercent` (disc ⊂ square, so both percentages will rise slightly). That is an intentional tightening and a one-time re-baseline. Do not leave two dilation semantics in one criterion.

**P6 — Structural stratum `S`.** From `I`: a maximal run of consecutive `I=1` pixels along a row (horizontal) or column (vertical) of length `>= STRUCTURAL_MIN_RUN` contributes all its pixels to `S`. `S` = union of horizontal and vertical contributions. Pure integer.

**P7 — Connected components.** 8-connected, iterative explicit-stack flood fill, seeds visited in raster order. Only the **largest component size** is reported, so label numbering is irrelevant to the result.

**P8 — Ratio and F1 edge cases.** Pin exactly today's semantics:

```
ratio(n, d) = 1 if d == 0 and n == 0
            = 0 if d == 0 and n != 0
            = n / d otherwise
f1(p, r)    = 0 if p + r == 0 else (2*p*r) / (p + r)
```

### 1.2 `cell-edge-f1-v1`

**Scoring cells.** The union of three sources, deduplicated by `(x,y,w,h)`:

- **N** — the reviewed named regions (`PAGE_ONE_CRITICAL_REGIONS`, `PAGE_TWO_CRITICAL_REGIONS`), unchanged. These remain the geometry anchors.
- **G0** — a non-overlapping 64×64 device-px grid with origin `(0,0)`, clipped to the page.
- **G1** — the same grid with origin `(32,32)`, clipped to the page.

> G0 ∪ G1 exists solely to close the coverage attack. Three red-team defects — a mirrored PDF417, a displaced masthead, and a fabricated `"NOT VALID FOR FILING"` advisory line — scored a *perfect* `1.000000` because they fell outside every one of the 113 named regions. Coverage cannot be a review promise; it must be a construction. The 32-px offset grid guarantees any defect up to 32×32 px lies wholly inside at least one cell, so a defect straddling a G0 boundary cannot be diluted into invisibility.

**Scored cell.** A cell is *scored* iff `expectedEdgePixels(cell) >= MIN_CELL_EDGE_PIXELS`. Unscored cells are enumerated in the report but not gated.

**Computation.** Dilation is computed **page-globally, once**, then counted within each cell:

```
Ed = dilate(E_expected, 1);  Ad = dilate(E_actual, 1)
for each cell C:
  expEdge_C     = |{i in C : E_expected[i] == 1}|
  actEdge_C     = |{i in C : E_actual[i]   == 1}|
  matchedExp_C  = |{i in C : E_expected[i] == 1 and Ad[i] == 1}|
  matchedAct_C  = |{i in C : E_actual[i]   == 1 and Ed[i] == 1}|
  precision_C   = ratio(matchedAct_C, actEdge_C)
  recall_C      = ratio(matchedExp_C, expEdge_C)
  f1_C          = f1(precision_C, recall_C)
```

Global-then-count is mandatory and must be pinned: computing dilation per-cell would fabricate false mismatches at every cell border, and with two overlapping grids that would swamp the signal.

**Gate.** For every scored cell: `f1_C >= pinned_baseline_f1_C - M_CELL`. Precision and recall are recorded per cell but **not** independently gated here — §1.4 binds the missing-ink direction globally, which is where the cross-form data showed recall is the weak side (P−R gaps to +0.1774).

**Coverage assertion.** `coverage = |{i : E_expected[i]==1 and i lies in >= 1 scored cell}| / |{i : E_expected[i]==1}|` must satisfy `coverage >= MIN_EDGE_COVERAGE`. This is the mechanical replacement for "regions must be proven to tile the page".

### 1.3 `structural-ink-coverage-v1`

```
S_exp = structural(I_expected);  S_act = structural(I_actual)
Sd_exp = dilate(S_exp, 1);       Sd_act = dilate(S_act, 1)
structural_recall    = ratio(|S_exp AND Sd_act|, |S_exp|)
structural_precision = ratio(|S_act AND Sd_exp|, |S_act|)
unmatched_expected   = S_exp AND NOT Sd_act            // a mask
largest_unmatched_cluster = largest 8-connected component of unmatched_expected
```

Gated on all three. This stratum is **font-independent by construction** — every text-only defect I have evidence for leaves it bit-identical to baseline, which is correct behaviour and must be stated loudly rather than mistaken for insensitivity.

**Relationship to the existing `compareOfficialStructure`** (ink 100 / run 20 / radius 4): they are different populations and are **not comparable**. Keep the legacy fields in the report for continuity; never average, substitute, or present one as the other. Mark the legacy fields `"superseded_by": "structural-ink-coverage-v1"` in the schema.

### 1.4 `page-ink-budget-v1`

```
expected_ink   = |I_expected|
actual_ink     = |I_actual|
ink_missing    = |I_expected AND NOT dilate(I_actual, 2)|
ink_unexpected = |I_actual   AND NOT dilate(I_expected, 2)|
paper_pixels   = |{i : data_actual[o]==255 and [o+1]==255 and [o+2]==255}|
```

Four bound quantities: `ink_missing / expected_ink`, `ink_unexpected / actual_ink`, `actual_ink` as a ratio to its pinned baseline, and `paper_pixels`. The first two already exist as `expectedInkMissingPercent` / `unexpectedActualInkPercent` and are **currently computed but unbound** — that is a live gap, independent of this proposal.

---

## 2. Thresholds

Every constant below carries its status. **No constant was chosen to make a form pass**; §8 records that 2551Q fails several of these today under absolute reading, and §6 records that the criterion is a non-regression gate precisely because absolute thresholds are not defensible from this evidence.

| constant | value | status | justification |
|---|---|---|---|
| `TOLERANCE_RADIUS_PX` | **1** | **measured, pin now** | see below |
| `M_CELL` | **0.0010** | **measured locally, BLOCKED on §8.1** | see below |
| `MIN_CELL_EDGE_PIXELS` | 200 | measured | below this, single-pixel changes swing F1; the p1 ink floor of `0.8333` was driven by a tiny cents-cell region |
| `MIN_EDGE_COVERAGE` | 0.98 | derived from construction | with G0∪G1 the achieved value should be ≈1.0; 0.98 is slack for page margins, not a target |
| `EDGE_THRESHOLD` | 48 | **PROVISIONAL — sweep required** | inherited, never swept |
| `INK_THRESHOLD` | 160 | **PROVISIONAL — sweep required** | inherited; a demonstrated cliff at 159→162 flips structural recall `0.824 → 0.136` |
| `STRUCTURAL_MIN_RUN` | 24 | **PROVISIONAL — sweep required** | ≈4 mm at 144 DPI, longer than any glyph stroke at 8–9 pt; never swept |
| `STRUCTURAL_RECALL_DROP` | 0.002 | measured | below |
| `STRUCTURAL_PRECISION_DROP` | 0.002 | measured | below |
| `MAX_UNMATCHED_CLUSTER` | `max(baseline, 200)` | measured | below |
| `COMPLETE_PAGE_HEADROOM_PP` | 0.25 | measured | below |
| `INK_RATIO_BAND` | ±0.03 | measured | below |
| `MIN_PAPER_PIXEL_RATIO` | 0.98 × baseline | measured | below |

### 2.1 `TOLERANCE_RADIUS_PX = 1` — decisive, and not close

| defect | r=1 | r=2 | r=3 |
|---|---|---|---|
| whole-page 1 px misregistration (page-global F1) | 0.979945 | **1.000000** | **1.000000** |
| one field shifted 2 px (worst-region F1, floor ≈0.945/0.948/0.950) | 0.896417 | 0.955233 — **missed** | 1.000000 — **missed** |
| one field shifted 4 px | 0.783495 | 0.819226 | 0.876854 |
| one field removed | 0.901714 | 0.933999 | 0.941047 |

r=2 scores a whole-page 1 px misregistration as **literally perfect**, and scores a fake-bold render as an *improvement*. The inherited radius of 2 must be abandoned. r=3 and r=4 additionally collapse headroom (0.0036 and 0.0020 of range remaining at the best-scoring page across 24 pages), leaving a regression nowhere to move the number.

### 2.2 `M_CELL = 0.0010` (10e-4)

**Measured floor: exactly zero.** Three in-process clean re-renders → self-diff 0 px. Two fully independent Playwright browser processes, 80 regions, both pages → max |Δ region F1| = `0.0000e-4`. Renders are byte-identical across full runs (24/24 PNGs, `cmp`-clean).

Margin: with a zero floor the ratio is unbounded; the operative question is what 10e-4 catches that 50e-4 does not.

| defect | Δ region F1 | caught at 10e-4 | caught at 50e-4 |
|---|---|---|---|
| translate 1 dp, one label | 44.7e-4 | ✅ 4.5× | ❌ |
| column shift 2 dp | 25.2e-4 | ✅ 2.5× | ❌ |
| stray 12 dp square (≈2.1 mm) | 43.6e-4 | ✅ 4.4× | ❌ |
| fabricated `"NOT VALID FOR FILING"` line | 23.4e-4 | ✅ 2.3× | ❌ |
| border +2 dp | 526.3e-4 | ✅ | ✅ |
| masthead displaced 4 dp | 10.0e-4 | ⚠️ exactly on the band | ❌ |
| translate 1 dp, whole region | 268.0e-4 | ✅ | ✅ |
| font-size +0.5% | 1002.6e-4 | ✅ | ✅ |
| page scale +0.05% | 584.5e-4 | ✅ | ✅ |
| missing checkbox | 9708.0e-4 | ✅ | ✅ |
| whole-page 1 px misregistration | 1663.0e-4 | ✅ | ✅ |
| serif font substitution | 1338.5e-4 | ✅ | ✅ |
| fake bold (+21.9% ink) | 176.5e-4 | ✅ | ✅ |
| faded print `#777777` | 523.2e-4 | ✅ | ✅ |

At 50e-4 the attack surface roughly doubles. 10e-4 is the correct band **and it is blocked** — see §8.1.

### 2.3 Structural thresholds

Page-2 baseline: recall `0.987697`, precision `0.991480`, largest unmatched cluster `134` px.

| defect | recall | largest cluster | margin |
|---|---|---|---|
| ruled line deleted | 0.96733 | 1908 px | 14× on cluster |
| whole table shifted 2 dp | 0.96835 | 683 px | 5× on cluster |
| global 1 px shift | 0.97842 (prec. 0.97495) | — | clear |
| every text-only defect | **identical in every digit** | **identical** | correct, by design |

`0.002` on recall/precision sits ~10× below the smallest real signal (`0.0194`) and above a zero measured floor. `max(baseline, 200)` on the cluster sits 3.4× above the page-2 baseline of 134 and 3.4× below the smallest real signal of 683.

### 2.4 Complete-page headroom `+0.25 pp`

This binds **increases only**, because the number is non-monotone (§7). It catches: page scale +0.2% (+3.739 pp), missing field label (+0.781 pp), label clipped (+0.561 pp), 1 dp region translation (+0.562 pp), column shift 2 dp (+0.674 pp), missing barcode (+0.477 pp), mirrored PDF417 (+0.406 pp), font-size +5% (+0.269 pp). It does **not** catch, and is not relied upon for: missing checkbox (+0.021 pp), missing ATC entry (−0.100 pp), missing wordmark (−0.032 pp), wrong tax rates (+0.014 pp).

### 2.5 Ink budget thresholds

`INK_RATIO_BAND = ±0.03` on `actual_ink / baseline_actual_ink` catches fake bold (+21.9%), doubled borders (+31.4%), and faded print (which moves ink below the 160 threshold). The measured local floor is 0 (byte-identical renders), so ±3% is ~generous slack for a future Chromium AA change, not a fitted value.

`MIN_PAPER_PIXEL_RATIO = 0.98 × baseline` on the count of exactly-`(255,255,255)` pixels. This is the direct, exact answer to the tint attack: a page tinted to luminance 232 drives `paper_pixels` to ≈0 while producing a max cell regression of only `10.9e-4`. Do not attempt to catch tinting with a percentage gate — the complete-page number has a cliff there, jumping `7.3304% → 55.5732%` on a 4/255 change.

---

## 3. Complete set of companion assertions

The tolerance component closes exactly one attack family (sub-tolerance drift, §Attack 5). Everything else is closed here. **The criterion is not defensible without all of these.**

### 3.1 Retained unchanged from `form-parity.spec.ts`

| assertion | closes |
|---|---|
| `expectCriticalRegionGeometry` @ 2 device px | sub-radius displacement of named elements — **load-bearing, now carries more weight, not less** |
| `verifyPageIndexedStaticText` | baseline content presence |
| `"2551Q PDF417 artwork keeps the reviewed source geometry"` | masthead/seal/barcode bounding-box placement |
| exact page count | page loss/insertion |
| overflow / clipping (`geometry readiness sees overflow hidden descendants`) | clipped labels |
| long-value + adaptive character-guide tests | fixture-driven layout |
| `"2551Q page-one typography keeps the reviewed bundled-font calibration"` | font substitution at source |

None of these may be weakened or removed. `official-fidelity-v1` is **additive**.

### 3.2 NEW — `static-text-exhaustive-v1`

Closes **Attack 1** (all statutory tax rates wrong, `0.19e-4`, zero violations) and **Attack 2** (swapped column headings and a fabricated perjury clause, `0.00e-4`, zero violations).

`containsExactStaticText` is a per-string containment test against `formPage.innerText()`. It is **order-blind, position-blind, and addition-blind** — this is a live defect in the current suite today, not a new risk. Three changes:

**(a) Add `order: number` and mandatory `selector` to `OfficialStaticTextEntry`** for every heading, column heading, statutory citation, and table cell.

**(b) Add ATC rate coverage.** `Form2551Q.tsx:522` renders `row.rate` into the third `<td>` and **no manifest entry anywhere contains a Tax Rate string**. The `data-atc-code` attribute already exists at `Form2551Q.tsx:512`. Add, for every ATC row:

```ts
{ id: "p2-pt010-rate", page: 2, order: <n>, kind: "table-entry",
  selector: ".official-atc-table tr[data-atc-code='PT010'] td:nth-child(3)",
  text: "3%" }
```

**(c) Add `verifyStaticTextExhaustive(pageText, entriesForPage, allowedResidual)`** — an *ordered, consuming* match:

```
cursor = 0
for entry in entries sorted by order:
    idx = normalizedPageText.indexOf(entry.text, cursor)
    if idx < 0 -> violation { kind: "missing-or-reordered", id }
    residual += normalizedPageText.slice(cursor, idx)
    cursor = idx + entry.text.length
residual += normalizedPageText.slice(cursor)
residual must contain only whitespace and strings in `allowedResidual`
```

`allowedResidual` is a pinned list of fixture-supplied values for the parity fixture. A reordering breaks the ordered match; an insertion lands in `residual`. Both attacks fail closed.

**(d) Add a manifest-completeness assertion.** Every `[data-atc-code]` row and every `.payment-headings > b` in the DOM must have a manifest entry. This prevents the manifest from silently falling behind the renderer.

### 3.3 NEW — `encoded-artwork-integrity-v1`

Closes **Attack 3** (`transform: scaleX(-1)` on the PDF417 — payload destroyed, bounding box unchanged, so both geometry assertions pass; `0.00e-4` on page 1, and detected on page 2 only by the accident that the page-2 barcode happens to fall inside the `Schedule 1 masthead` region).

Two bindings, both required:

1. **DOM-level payload assertion** — the encoder input string for each symbol must equal a reviewed pinned value. Robust across Chromium versions.
2. **Raster crop hash** — crop the symbol's pinned bounding box from the actual page raster, SHA-256, compare to a reviewed pin per `(form, revision, fixture)`. Catches any rendering-side corruption the DOM cannot see, including mirroring.

Recommend adding a PDF417 **decode** assertion as a follow-up; it is strictly better than the hash but must not block this spec.

### 3.4 NEW — permanent defect-injection regression suite

**A criterion that cannot detect a deliberately injected regression is worthless, and this is the only mechanism that keeps that property true over time.**

Ship all six red-team attack families plus the graded translation/scale/erasure series as a tracked test. Each case asserts that `official-fidelity-v1` **fails**, naming which component fires. New criterion changes must keep every case failing.

Two injections are known **VOID** (they produced `self_changed_pixels = 0`, i.e. did not render) and must be excluded, not recorded as misses: `hairline_borders` on both pages, and `label_text_swap` on page 1. Border `+1` device px is **unrenderable** at DSF 1.5 — three independent injection methods produced exactly 0 changed pixels. It is not a defect class that can exist here and must not be listed as a miss.

---

## 4. Evidence schema and audit recomputation

### 4.1 Additions to the visual evidence report

Top level:

```json
{
  "schema_version": 2,
  "gate": "official-fidelity-v1",
  "producer": "playwright-form-fidelity-v1",
  "producer_path": "...", "producer_sha256": "...",
  "criterion": {
    "id": "official-fidelity-v1",
    "components": ["cell-edge-f1-v1","structural-ink-coverage-v1",
                   "page-ink-budget-v1","official-complete-page-v2",
                   "static-text-exhaustive-v1","encoded-artwork-integrity-v1"],
    "is_non_regression_gate": true,
    "proves_parity": false,
    "constants": { "tolerance_radius_px": 1, "edge_threshold": 48,
                   "ink_threshold": 160, "structural_min_run": 24,
                   "m_cell": 0.0010, "min_cell_edge_pixels": 200,
                   "min_edge_coverage": 0.98, "grid_size_px": 64,
                   "grid_offsets": [[0,0],[32,32]],
                   "luminance_storage": "binary64",
                   "dilation": "euclidean_disc" },
    "baseline_source": "crates/bir-print/src/html_forms/form_2551q.rs",
    "baseline_pin_sha256": "..."
  },
  "chromium_build": "145.0.7632.6",
  "playwright_version": "...",
  "environment_drift_evidence": "...path or null..."
}
```

Per page, **in addition to every existing field** (`full_page_changed_pixels`, `full_page_changed_percent`, `poppler_raster_changed_*`, `reference_noise_floor_*`, region report, structural legacy fields — all retained):

```json
{
  "cell_edge_f1": {
    "comparison": "cell-edge-f1-v1",
    "cell_table_sha256": "...",
    "scored_cell_count": 0, "unscored_cell_count": 0,
    "edge_coverage": 0.0, "min_edge_coverage": 0.98,
    "worst_cell": { "id": "...", "f1": 0.0, "baseline_f1": 0.0, "regression": 0.0 },
    "max_regression": 0.0, "m_cell": 0.0010,
    "cells": [ { "id": "...", "kind": "named|grid0|grid1",
                 "x":0,"y":0,"width":0,"height":0,
                 "expected_edge_pixels":0,"actual_edge_pixels":0,
                 "matched_expected_edge_pixels":0,"matched_actual_edge_pixels":0,
                 "precision":0.0,"recall":0.0,"f1":0.0,
                 "baseline_f1":0.0,"regression":0.0,"scored":true,"passed":true } ],
    "passed": false
  },
  "structural_ink_coverage": {
    "comparison": "structural-ink-coverage-v1",
    "structural_expected_pixels":0,"structural_actual_pixels":0,
    "structural_fraction_of_expected_ink":0.0,
    "recall":0.0,"precision":0.0,
    "baseline_recall":0.0,"baseline_precision":0.0,
    "largest_unmatched_cluster_px":0,"baseline_largest_unmatched_cluster_px":0,
    "unmatched_diff":"...png","unmatched_diff_sha256":"...",
    "passed": false
  },
  "page_ink_budget": {
    "comparison": "page-ink-budget-v1",
    "expected_ink_pixels":0,"actual_ink_pixels":0,
    "baseline_actual_ink_pixels":0,"ink_ratio":0.0,
    "ink_missing_pixels":0,"ink_missing_percent":0.0,
    "ink_unexpected_pixels":0,"ink_unexpected_percent":0.0,
    "paper_pixels":0,"baseline_paper_pixels":0,
    "passed": false
  },
  "complete_page_role": "reported_diagnostic_with_regression_ceiling",
  "complete_page_baseline_percent": 0.0,
  "complete_page_ceiling_percent": 0.0
}
```

`structural_fraction_of_expected_ink` is **mandatory and non-optional** — it is the anti-gaming instrument for `STRUCTURAL_MIN_RUN`. Someone could raise the run length until only perfectly-matched rules survive; publishing the structural population's size and its fraction of total ink (measured: p1 47.2% / 114,389 px; p2 53.2% / 110,378 px) makes that shrinkage visible on the face of the evidence.

### 4.2 What `scripts/audit_html_form_migration.py` must independently recompute

In pure Python (stdlib only — `zlib`, `struct`, `math`; no numpy, no scipy, no JS), from the **hashed artifacts**, never from the reported numbers:

1. **Every existing check, unchanged** — `pixelmatch_mask` complete-page recomputation, the Poppler diagnostic, the pinned noise floor, all artifact hashes, all dimensions.
2. **Luminance → Sobel → `E_expected`, `E_actual`** per P2/P3. Assert `expected_edge_pixels` and `actual_edge_pixels` **exactly** (integers).
3. **Disc dilation r=1** per P5, then per-cell accumulation per §1.2. Assert all four integer counts per cell **exactly**; assert `precision`, `recall`, `f1` with `math.isclose(rel_tol=1e-12)`.
4. **Cell table reconstruction.** Rebuild N∪G0∪G1 from the pinned named regions and the pinned grid constants; assert the reconstructed table's SHA-256 equals `cell_table_sha256`. This prevents cell definitions being swapped for friendlier ones.
5. **`edge_coverage`** recomputed and `>= MIN_EDGE_COVERAGE`.
6. **Structural stratum** per P6/P7 — all counts exactly, `largest_unmatched_cluster_px` exactly, and the unmatched-diff PNG re-derived and hash-compared.
7. **Ink budget** — every count exactly, including `paper_pixels`.
8. **Baselines read from the Rust pins**, not from the report. Same pattern as the existing reference-hash pinning. A report claiming a baseline the Rust pin does not carry is an error.
9. **Mandatory presence.** Error if `full_page_changed_percent` is absent or if any of the six components is missing. `official-fidelity-v1` cannot be reported without the complete-page number.
10. **Consistency.** Error if `criterion.proves_parity` is anything but `false`, or `is_non_regression_gate` anything but `true`.

### 4.3 Fail-closed properties preserved

- `TRUSTED_VISUAL_EVIDENCE_PRODUCERS` stays an **empty frozenset**. `playwright-form-fidelity-v1` is registered only after the user reviews the producer.
- Every new artifact (`unmatched_diff`, barcode crop) is hashed, re-read, and re-verified via `_audit_hashed_file`.
- Clean-source binding via `--require-clean-source` is unchanged and required for promotion runs.
- `parsePromotionVisualThreshold` and `RELEASE_VISUAL_MAX_CHANGED_PERCENT = 1` **remain in the tree** (see §7.2).
- Untracked artifacts (`tmp/`, `test-results/`) still cannot promote anything.

### 4.4 Measured runtime (not estimated)

Per page, 1224×1872, this machine, CPython 3.13, no numpy:

| step | cost | status |
|---|---|---|
| `read_png_rgba` ×2 | 4.5 s | already paid |
| `pixelmatch_mask` (gate) | 4.7 s | already paid |
| Poppler diagnostic pixelmatch | 4.7 s | already paid |
| luminance ×2 | 1.0 s | new |
| Sobel ×2 | 3.2 s | new |
| disc dilate r=1 ×2 | 0.8 s | new |
| ink masks + dilate r=2 ×2 | 1.6 s | new |
| structural extraction ×2 | 0.8 s | new |
| connected components | 0.3 s | new |
| cell accumulation (≈1,200 cells, 2 grids) | 0.5 s | new |

**New cost ≈ 8.2 s/page.** 2551Q's two pages: ~18 s → ~35 s. All ten forms (24 pages) once chromium references exist: ~3.7 min → ~7 min. Comfortably inside budget, and the same order as what the audit already pays for pixelmatch.

---

## 5. Known, accepted limitations — record these verbatim in the evidence document

1. **This is a non-regression gate, pinned to a reviewed baseline.** It passes the current render by construction. It certifies "no worse than a reviewed state", never "matches the official form".
2. **The reference does not encode the document's true typography.** `pdffonts` shows the official PDF embeds none of its primary faces (Arial, Arial-Bold, Arial-Italic, Times New Roman all `emb=no`). Poppler substitutes, so the pinned reference encodes *Poppler's substituted outlines*. Glyph outline shape is ~57% of the residual. The absolute fidelity numbers — complete-page **7.3355%** (p1) / **5.4557%** (p2), page-global edge F1 **0.9653** / **0.9919** — are the fidelity claim, and they are what they are.
3. **Cross-environment drift is unmeasured** (§8.1). All floors are single-machine, single-Chromium (145.0.7632.6), single-Playwright.
4. **Three constants are unswept**: `EDGE_THRESHOLD=48`, `INK_THRESHOLD=160`, `STRUCTURAL_MIN_RUN=24`. Each has a demonstrated cliff nearby — ink 159→162 flips structural recall `0.824 → 0.136`.
5. **Two page-1 cells are already pathological at baseline and are dead detectors**: `"Item 12A value"` (F1 ≈0.703–0.758) and `"Item 17 inline specification field"` (100% ink-miss, 98% ink-unexpected — possibly an artifact of the fixture-blanking rules, **not verified**). Investigate before baselining; do not silently pin.
6. **Page 2 has only 4 named regions vs 76 on page 1.** The grid closes the coverage hole but page-2 named-region sensitivity remains structurally weaker.
7. **Border `+1` device px is unrenderable** at DSF 1.5 (Chromium snaps it). Not a detectable-or-missed data point.
8. **The evidence base is one form, one fixture, one platform.** Cross-form scores span F1@r2 `0.7197`–`0.9517` with no natural break; that continuum tracks text density (Pearson r = −0.5414), not correctness.
9. **Sensitivity is form-dependent.** The same 600×36 erasure costs `0.01144` F1 on 2551Q p1 but only `0.00576` on 1601C p2. The pages that most need policing are policed least.
10. **Defect injection was via DOM/CSS and raster surgery**, not renderer-source mutation. It measures *metric* sensitivity, not end-to-end pipeline sensitivity.
11. **`tmp/diagnose-2551q-print-raster.mjs` hardcodes a page-2 → sheet-3 mapping that is wrong.** Any round-trip measurement built on it is silently invalid. Fix or annotate.

---

## 6. What `official-fidelity-v1` does NOT prove

State this block, unedited, wherever the criterion is documented.

- **It does not prove visual parity with the official form.** Full parity remains the complete-page percentage and nothing else, and that number is 7.3355% / 5.4557%.
- **It does not prove the printed text is correct.** Wrong statutory tax rates on every ATC row scored `0.19e-4` region-scoped — indistinguishable from zero — and passed every pixel component. The shipped grid cells raise that to `1.07e-2` for an in-place digit change (see the §0 amendment), but that is incidental and unreliable, and is explicitly not relied upon. Content correctness rests entirely on `static-text-exhaustive-v1`.
- **It does not prove fixture values are correct.** `prepareOfficialBlankComparison` makes `.comb-value` spans transparent by design; changing a digit in a monetary value produces a self-diff of **exactly 0 px**. The visual criterion is *structurally incapable* of checking any fixture value. Nobody may read visual parity as value correctness.
- **It does not prove the barcode is machine-readable.** Only `encoded-artwork-integrity-v1` addresses that, and only by payload string + raster hash.
- **It does not prove anything about a page area no cell covers.** Coverage is now constructed rather than promised, but it is bounded at `>= 0.98`, not 1.0.
- **It does not prove correctness below the tolerance radius** except through `expectCriticalRegionGeometry`'s 2-device-px bound. That assertion is the load-bearing complement that makes the tolerance metric honest.
- **It does not prove cross-platform or cross-Chromium stability.** That is a separate, unmeasured evidence link.
- **It is not native print/export evidence, packaged-offline evidence, or rollback-drill evidence.** Those links in the promotion chain are untouched.

---

## 7. Migration of the complete-page gate

### 7.1 New role — precisely stated

`official-complete-page-v2` is **retained, computed, recorded, and never hidden**. Its role changes from **decision variable** to **mandatory reported diagnostic with a one-sided non-regression ceiling** (`baseline + 0.25 pp`).

The justification is not that it is unreachable. It is that **the number moves the wrong way on real defects**, established independently by three investigations:

| defect | Δ complete-page |
|---|---|
| missing ATC entry (p2, 1,741 px deleted) | **−0.1002 pp** (improves) |
| missing government wordmark (p1, 2,356 px deleted) | **−0.0317 pp** (improves) |
| 600×36 erasure on 2551Q p1 | −0.0464 pp (improves) |
| tax-rate value deleted (p2) | 5.4557% → **5.4523%** (improves) |
| wrong printed digit 8% → 3% | −0.000 pp |

Mechanism, verified not speculated: where our glyphs are slightly offset from the reference's, **both** our ink and the reference ink count as changed pixels; deleting our ink halves that contribution. **At the margin, the current 1% gate rewards deleting content from a tax form.** It is also non-monotone in translation (x−3px `7.856%` > x−5px `7.797%`) and column shift, and has a 48-percentage-point cliff on a 4/255 paper-tint change.

This is a live defect in the incumbent gate, worth surfacing on its own merits, independent of whether this proposal is adopted.

### 7.2 Mechanics

- Keep `release-visual-threshold.ts` and `RELEASE_VISUAL_MAX_CHANGED_PERCENT = 1` in the tree, still enforced for any form claiming the *old* `visual_parity` gate. Nothing is weakened; forms move to the new gate explicitly.
- The evidence `gate` field distinguishes `"visual_parity"` (old) from `"official-fidelity-v1"` (new). The audit validates each against its own rules.
- **CLAUDE.md must be updated in the same reviewed commit.** It currently states "Never weaken the numeric 1% gate, no matter how close a form gets." Adopting `official-fidelity-v1` is a deliberate, user-authorized change to that rule. Leaving the two in contradiction is worse than either position. The replacement wording should state that the 1% gate is retained as a reported diagnostic and one-sided ceiling, and that the complete-page percentage remains the parity claim.

### 7.3 What the other nine forms need

`official-fidelity-v1` is **not applicable** to `0605:1999`, `0619E:2018`, `0619F:2018`, `1601C:2018`, `1701:2018`, `1701Q:2018`, `1702MX:2018C`, `1702RT:2018C`, `2550Q:2024` today. Each needs, in order:

1. **A chromium reference** via `scripts/prepare_chromium_reference.mjs`, with the per-page noise floor recorded and hashes pinned in Rust. Nine of ten forms have Poppler references only.
2. **A reviewed static-text manifest** with `order` and selectors, plus the manifest-completeness assertion. This is the single largest piece of work and it is unavoidable — it is the only mechanism covering content correctness.
3. **A reviewed named-region set** for `expectCriticalRegionGeometry`. The grid is automatic; the named regions are not.
4. **A reviewed baseline pin** — per-cell F1, structural recall/precision/cluster, ink counts, paper pixels, complete-page percent.
5. **The defect-injection suite instantiated for that form**, because sensitivity is form-dependent (0.01144 vs 0.00576 for the same defect).

**Do not set an absolute cross-form threshold from the existing cross-form table.** The nine forms' F1@r2 values (`0.71972` to `0.95171`) form a smooth continuum with no internal gap larger than ~0.015. The only discontinuity is between that population and 2551Q's `0.96372`, and that gap measures calibration effort, not correctness. A threshold placed there would be derived from n=1 — the exact failure mode this project keeps hitting.

**One genuinely useful cross-form result to carry forward:** the Poppler-vs-Chromium reference offset costs **3.07–3.61 pp** on the complete-page number but only **0.00054–0.00169 F1** on the edge metric. The rasterizer noise floor that made the pixel gate unreachable is almost entirely absent from the tolerance metric. That makes the nine forms' Poppler-only numbers legitimately comparable, understated by ~0.001–0.002 F1 — though this quantification comes from one form and is plausible, not proven, for forms with a different font-substitution profile.

---

## 8. Blocking preconditions — none satisfied today

`official-fidelity-v1` may be implemented and run in reporting mode now. It **may not gate a promotion** until all four are closed.

### 8.1 Cross-environment drift must be measured — this gates `M_CELL`

Same-machine repeatability is **exactly zero** (3 in-process renders, 2 independent browser processes, 80 regions, both pages, max |Δ| `0.0000e-4`). Cross-machine and cross-Chromium-version drift is **unmeasured**, and a Chromium bump that shifts glyph rasterization would move every cell at once.

Measure: re-render on a second machine, and after a deliberate Playwright/Chromium upgrade. Derive `M_CELL` from observed drift.

**If drift exceeds ~10e-4, do not widen the band.** Widening to 50e-4 doubles the attack surface (§2.2). Instead pin the Chromium build into the evidence chain and treat a Chromium bump as an explicit re-baseline event requiring review. Record `chromium_build` and `playwright_version` in the evidence report from day one so this is enforceable.

### 8.2 Real page-1 structural defects must be fixed before baselining

Running the structural stratum clean on page 1 immediately localized these, each **exactly 2 device px off**, all currently buried inside the undifferentiated 7.3355%:

- 3 clusters of **1,130 px** — full-width horizontal rules at y=691, y=721, y=1201
- **739 px** — box outline x=47..751, y=1025..1059
- **213 px** — vertical rule x=756..757, y=1674..1802
- **162 px ×2** — checkbox-sized elements at x=1094..1123, y=893..927 and y=1025..1059

Baselining now would bake a 1,130-pixel misplacement into the accepted floor. Fix first, then baseline.

### 8.3 The three unswept constants must be swept and pinned

`EDGE_THRESHOLD=48`, `INK_THRESHOLD=160`, `STRUCTURAL_MIN_RUN=24`. Each has a demonstrated cliff. Sweep each, record the response curve in the evidence file, and pin with the sweep as justification.

### 8.4 The companion assertions in §3.2–§3.4 must land first

Until `static-text-exhaustive-v1` and `encoded-artwork-integrity-v1` exist, a render with every statutory tax rate wrong and a mirrored barcode passes the full criterion. Adopting the pixel components alone would **launder** that render behind an improved-looking headline number. That is the precise failure this project keeps repeating, and the ordering is not negotiable.

---

## 9. Implementation checklist

**`packages/form-renderer/visual/`**
- `grayscale-edge-match.ts` — `Float32Array` → `Float64Array`; `Math.hypot` → squared comparison against `2304.0`; export the primitives; retire `LAYERED_EDGE_EVIDENCE_POLICY` in favour of the criterion block, or set `promotionEligible: false` on the *component* while the composite carries eligibility.
- new `official-fidelity.ts` — P1–P8 primitives, `cell-edge-f1-v1`, `structural-ink-coverage-v1`, `page-ink-budget-v1`, deterministic cell-table builder + SHA.
- new `fidelity-cells.ts` — N ∪ G0 ∪ G1 construction, `MIN_CELL_EDGE_PIXELS`, coverage.
- `official-2551q-static-text.ts` — add `order`, add ATC rate entries, add `verifyStaticTextExhaustive`, add manifest-completeness.
- new `encoded-artwork.ts` — payload assertions + barcode crop hash.
- `form-parity.spec.ts` — wire all six components; keep every existing assertion; emit schema v2.
- new `fidelity-injection.spec.ts` — the permanent defect suite.

**`crates/bir-print/src/html_forms/form_2551q.rs`** — per-page baseline pins (cell F1 table, structural triple, ink counts, paper pixels, complete-page percent) + `baseline_pin_sha256`, byte-locked by `cargo test -p bir-print`.

**`scripts/audit_html_form_migration.py`** — §4.2 items 1–10; registries stay empty frozensets.

**Docs** — CLAUDE.md (§7.2), `docs/form-print-readiness/priority-forms-readiness.md`, and the two `.codex/skills/` playbooks.
