# Priority HTML Form Readiness

Last verified: **July 20, 2026**.

The machine-readable authority is
`packages/form-specs/form-migration-status.json`, cross-checked against
`crates/bir-core/src/forms/support_level.rs`. Release evidence lives in
`packages/form-specs/form-release-evidence.json`. This page summarizes those
records and source-pack constraints; it does not promote a form.

Every exact revision below has a Rust render provider, committed fixtures,
semantic React/CSS, a paper specification, deterministic pagination, and
official calibration references. Every form is still `ScaffoldOnly`, every
`visual_parity` capability is false, and no form is `release_ready`.

## Current status

| Exact target | Queue authority | Renderer route | Reviewed source | Primary blockers |
| --- | --- | --- | --- | --- |
| `2551Q:2018` | Proven | `html_only` | PDF + XML | Full-page visual gate and signed native/package evidence |
| `1601C:2018` | Proven | `experimental` | PDF + plain/encrypted XML | Full-page visual parity, native preview/print/PDF export, and packaged-offline evidence |
| `0619E:2018` | Blocked | `experimental` | PDF + plain/encrypted XML | Queue, calibrated parity, native, and packaged-offline evidence |
| `0619F:2018` | Blocked | `experimental` | PDF + plain/encrypted XML | Queue, calibrated parity, native, and packaged-offline evidence |
| `0605:1999` | Blocked | `experimental` | PDF + XML | Queue, calibrated parity, native, and packaged-offline evidence; exact revision is 1999 |
| `1701Q:2018` | Blocked | `experimental` | Locked 2018 PDF + hash-locked 7.9.5.0 HTA-backed exact editable XML contract | Reviewed outbound helpers/credential handling, queue/submission, parity, native, and packaged-offline evidence |
| `2550Q:2024` | Blocked | `experimental` | PDF + guide + XML | Queue, calibrated parity, native, and packaged-offline evidence; official paper is 612 by 1008 points |
| `1701:2018` | Blocked | `experimental` | Main PDF + conditional attachments + XML | Queue, conditional-page parity, native, and packaged-offline evidence |
| `1702RT:2018C` | Blocked | `experimental` | Four-page PDF + XML | Queue and full-page parity; native and packaged-offline evidence |
| `1702MX:2018C` | Blocked | `experimental` | Main/attachment PDFs + XML | Queue and full-page parity; native and packaged-offline evidence |

In the authoritative migration manifest, `1601C:2018` has
`capabilities.queue_submission: true`, while `1701Q:2018` has
`capabilities.xml_round_trip: true` and `capabilities.queue_submission: false`.
Those independent gates must not be collapsed into one filing-readiness claim.

The current complete-page comparisons are all above the strict 1% release
threshold. Structural line-only diagnostics are not a substitute for that
gate. Its page-indexed static-copy checks pass and its non-promoting
two-pixel edge F1 scores are approximately 0.964 and 0.991. `0605:1999`
remains blocked at approximately 7.44% on page 1 and 10.23% on page 2 even
though its structure-only diagnostics are approximately 0.26% and 0.20%.

`2551Q:2018` now gates against a pinned same-rasterizer chromium reference
(comparison `official-complete-page-v2`): the pinned official PDF is
converted to vector SVG with pdftocairo and rasterized by the same
Chromium/Playwright environment that captures the parity screenshots. The
raw Poppler-raster difference and the pinned per-page Poppler-vs-Chromium
noise floor are mandatory non-gating diagnostics reported next to the gate:

| 2551Q:2018 page | Gate vs chromium raster | Poppler-raster diagnostic | Pinned noise floor |
| --- | --- | --- | --- |
| Page 1 | 7.1422% | 6.5242% | 3.6104% |
| Page 2 | 5.4557% | 4.6501% | 3.0690% |

Page 1 was 7.3355% / 6.6530% before the structural fixes recorded below.

The gate being higher than the Poppler diagnostic is an honest finding, not a
regression: historical calibration drifted toward Poppler's rasterization
idiosyncrasies.

A 2026-07-20 five-probe investigation established what the residual is, and
corrected an earlier incorrect attribution to glyph shape and anti-aliasing:

- The measured rasterization/anti-alias floor is **0.6875% (page 1) and
  0.5540% (page 2)** — below the 1% gate, so anti-aliasing is not the blocker.
- Glyph outlines are not the blocker either. Rendering with the real platform
  Arial (the face the official PDF specifies) scores 7.3631%, marginally worse
  than bundled Arimo; Helvetica and Liberation Sans land within 0.11 points.
- Part of the residual is genuine **per-run geometry error**, and the axes are
  coupled, so every single-axis global correction sits at a local optimum:
  correcting size alone, weight alone, or baseline alone each leaves the gate
  unchanged or worse.
- An earlier headline figure of "median text-run width ratio 0.9437, 62% of
  runs narrower than 0.97" was later shown to be a **measurement artifact**:
  PyMuPDF span bounding boxes include leading and trailing space advances that
  a DOM `Range` rectangle excludes. Measured consistently, the median ratio for
  the ATC table is **0.99872** with only 11 of 93 runs below 0.97. Advance
  width is not a broad defect.

### Structural fixes landed 2026-07-20, and what they cost to get right

The criterion localized real defects the single 7.34% number was burying, and
fixing them improved BOTH rasterizers, which is the signature that separates
convergence from metric-chasing:

| 2551Q page 1 | Before | After |
| --- | --- | --- |
| Structural recall | 0.915875 | **0.965591** |
| Largest unmatched cluster | 1130 px | **105 px** |
| Worst scored cell | **0.000000** | 0.4523 |
| Complete page (chromium gate) | 7.3355% | **7.1422%** |
| Poppler diagnostic | 6.6530% | **6.5242%** |

What was actually wrong, and three corrections worth remembering:

- The three full-width rules were a WEIGHT defect, not displacement. Official
  part boundaries are 1.5pt (3 device px); ours drew 1pt. The localizer
  reported "dy=+2, recovers 100%", which is true and misdescribes the cause —
  shifting a too-thin stroke down does land it on ink. Moving those rules would
  have been the wrong fix. The tool now reports stroke thickness alongside
  offset so the two cases are distinguishable.
- Item 17's specification field is an UNDERLINE, not a box. We painted a top and
  left stroke the official does not have. This also cleared the dead cell that
  scored F1 0.000000.
- Thickening `border-right` was reverted after review: our right edge is x=1180
  and the official frame x=1179-1181, so growing it inward painted ~780 px of
  ink the official lacks while never reaching x=1181. `structural-ink-coverage-v1`
  cannot see this — at tolerance radius 1 each side dilates over the other — so
  recall rose while the untoleranced complete-page number got worse. A note sits
  at the site so it is not re-added.
- A thicker border moves what is inside it. The left-frame fix displaced every
  Part II text band by 1-2 px until the grid column was narrowed by the same
  0.5pt; that single compensation is what turned a mixed result into an
  improvement on both rasterizers.

**Item 12A capacity.** The official field carries 15 interior dividers on a
28.4 px pitch across 455 px: 16 character positions. The renderer declared 26 —
a 62% overstatement of what the form accepts, and the defect behind the
"Item 12A value" cell flagged at F1 0.703 with precision 0.621. A full audit of
all 96 comb fields on both pages found no other capacity mismatch, so this was
the only one. Ten wrong dividers moved the complete-page number by about a
hundredth of a percent, which is why capacity needs its own check rather than
relying on any pixel gate.

**Not fixed, deliberately, with reasons recorded at each site:** the Part II
centavos column is missing structure rather than displacement (our separator
sits 2 device px right and 2 px narrow, so a border-only fix paints ink the
official lacks while still missing its edges, and measured worse); page 2's ATC
row-pitch drift shows in the cell component rather than as a structural cluster;
and the y=1201 part boundary retains a ~1 px downward bias.

**Measurement caution recorded from this work.** Threshold-based row counting
misreports sub-pixel geometry: counting rows below tone 150 made a correct
3-device-px stroke look like 4 because our lighter partials crossed the
threshold and the official's did not. Read tone profiles, not thresholded
counts, when judging stroke placement.

### The decisive result: the 1% gate is not reachable for 2551Q

A follow-up prototype drove the Schedule 1 ATC table — 75.1% of page 2's diff
and uniquely safe to test because it renders from a static constant — to its
geometric limit, and then a controlled A/B tested the last remaining
rendering-side hypothesis. Both were adversarially reproduced.

- **Joint per-run reconciliation works, and is insufficient.** Correcting size,
  advance width, x-position and baseline *together* is genuinely superadditive
  and converges toward the official document (the Poppler diagnostic, ink
  recall and edge F1 all improve together). Best verified, semantics intact:
  page 2 5.4557% → **4.7276%**. Even pinning all 1,921 ATC glyphs to their
  exact PDF pen origins only reaches 4.1787%, and that variant **breaks the
  page-indexed static-text assertion** (28 violations) because per-glyph spans
  destroy inter-word spacing.
- **A perfect ATC table still fails page 2.** The non-ATC remainder alone is
  31,188 px = **1.3611%**, already over the gate before the ATC table
  contributes a single pixel. At the region's rasterization floor page 2 is
  1.8375%.
- **Rasterization method is not a lever.** Rendering our own output through the
  reference's exact pipeline (`page.pdf()` → `pdftocairo -svg` → same Chromium
  raster, verified to emit zero `<text>` and zero `font-family`) made parity
  marginally *worse*: page 1 7.3355% → 7.4828%, page 2 5.4557% → 5.5495%.
  Skia hinting and grid-fitting therefore account for **none** of the residual.
  The control reproduced the round-trip floor exactly, and re-deriving the
  reference reproduced it at **zero changed pixels**.
- **What is left is glyph outline shape.** For the ATC region the decomposition
  is 10,916 px (11.6%) rasterizer round-trip floor, ~29,260 px (31%) placement,
  and ~53,644 px (**57%**) outline shape, which no rendering-side change fixes.

The root cause is structural and is a property of the source document, not of
this renderer. `pdffonts` reports the primary faces — Arial, Arial,Bold,
Arial,Italic and Times New Roman — as **`emb=no`**: the official PDF does not
embed them. Poppler substitutes faces when rasterizing, so the pinned reference
encodes **Poppler's substituted outlines**, not the Arial the form was authored
in. Matching the reference pixel-for-pixel would therefore require adopting
Poppler's substitution, which is fidelity to a rendering artifact rather than to
the official document. Consistent with this, rendering with the real platform
Arial scores marginally *worse* than bundled Arimo.

**Conclusion of record:** the ≤1% complete-page gate is not achievable for
`2551Q:2018` by any rendering-side change, and closing the remaining difference
would mean overfitting to the reference pipeline's font substitution. The gate
has NOT been weakened in response to this finding, and must not be. How to
resolve the conflict — a different fidelity criterion, an embedded-font source,
or accepting that these forms do not reach `release_ready` under the current
criterion — is an open decision requiring explicit sign-off.

The reference itself was independently audited and is sound: bit-exact
reproduction of all pinned hashes, no `<text>` elements or `font-family`
attributes in the SVGs (so no local-font dependency), zero displacement, and
2.8–3.6× closer to a 576 DPI supersampled arbiter than the Poppler raster.

One structural caveat must be recorded: the chromium reference's own deviation
from that supersampled arbiter is **1.1149% on page 1**, which exceeds the 1%
gate. On page 1 a perfect render is therefore not distinguishable from roughly
1%, leaving an effective geometry budget of about 0.31 points. The 1% gate is
not weakened by this observation; whether page 1's reference basis should be
re-derived at higher precision is an open, explicitly flagged decision.

### Text-excluded structural decomposition (2551Q, 2026-07-20)

A controlled decomposition removed text from BOTH sides by construction —
glyph `<use>` placements stripped from the pinned reference SVG (artwork
`<use>` references preserved), glyph fills made transparent in our render with
layout and non-text ink proven unchanged — and ran the gate's own comparison
on what remains: boxes, fields, ruled lines, containers, gray fills, comb
guides, checkbox outlines, and artwork. Independently reproduced to the digit.

| 2551Q:2018 | Page 1 | Page 2 |
| --- | --- | --- |
| Complete page (gate) | 7.3355% | 5.4557% |
| Text-excluded (structure only) | **3.0556%** | **2.3837%** |
| Text-only complement | 4.4386% | 3.0785% |

Text is therefore ~56–59% of the total error and structure ~41–44%. Both
sides of the text-free comparison share one rasterizer, so no cross-rasterizer
noise floor applies: the structural residual is genuine displacement, not
missing structure (total structural dark ink matches within ~0.1%). It is
dominated by ~1 px vertical registration drift on horizontal rules and
gray-band edges (page 1) and ATC-table row-pitch drift (66% of page 2's
structural diff), plus comb-guide tick differences — geometry that, unlike
the font-substitution residual, is under the renderer's control.

A text-excluded percentage is blind to every character-level defect (wrong
values, digits, labels, check marks, fonts, clipping) and excludes 43–49% of
the page's dark ink. It is a diagnostic, never parity, per the standing rule.

The historical table below predates the chromium gate reference: its 2551Q row
is measured against the Poppler raster and is superseded by the tables above.
Under the current chromium-referenced gate the narrow structural line
diagnostic records 0.4237% / 0.5865% (visual-evidence.json), not the Poppler
figures below.

| Exact target | Complete-page difference by page | Structural difference by page |
| --- | --- | --- |
| `2551Q:2018` | 6.6529977376%, 4.6500544662% | 0.07122507123%, 0.00929591922% |
| `0605:1999` | 7.439441%, 10.226733% | 0.257798%, 0.200626% |
| `0619E:2018` | 8.591945% | 0.175107% |
| `0619F:2018` | 8.377329% | 0.331749% |
| `1601C:2018` | 11.866917%, 12.098137% | 0.389076%, 0.202808% |
| `1701Q:2018` | 13.479781%, 9.589548% | 0.328150%, 0.095185% |
| `1701:2018` | 12.599593%, 15.788224%, 12.253942%, 15.302349% | 0.416833%, 0.446597%, 0.243440%, 0.551471% |
| `1702RT:2018C` | 12.146711%, 8.016661%, 7.996585%, 11.390949% | 0.520877%, 0.148516%, 0.169116%, 0.875999% |
| `1702MX:2018C` | 13.217793%, 19.179227%, 11.552995%, 9.224345% | 0.385322%, 0.819307%, 0.131147%, 0.259195% |
| `2550Q:2024` | 8.303993%, 9.847932% | 0.407240%, 0.344548% |

These results retain the reviewed source corrections for 0619E checkbox
interiors, 0619F Part II row heights, 1701Q and 1701 typography, the 1702RT
Schedule IIIA rounding note, and the 1702MX Item 5 official plain
code/description boxes. Geometry, overflow, capacity, reviewed-copy, and
critical-region checks may pass while these raw comparisons remain blocked;
those narrower checks do not establish visual parity.

A July 18 cross-rasterizer diagnostic first measured this: rendering the
official 2551Q page itself through Poppler and through Chromium from
Poppler's vector SVG differed by approximately 3.61% under the same pixel
comparison. That floor is now measured per page by
`scripts/prepare_chromium_reference.mjs`, pinned in the reference manifest
(`chromium_raster.noise_floor_changed_pixels`), and independently recomputed
by the migration audit whenever visual evidence is presented. The floor does
not relax the 1% gate; it is provenance that keeps the cross-rasterizer noise
visible instead of buried inside a single number, and the gate itself now
excludes that noise by comparing within one rasterizer.

## Status meanings

- `ScaffoldOnly` means some in-app implementation exists but the exact revision
  has not satisfied the production capability gate.
- `experimental` permits development calibration and testing only.
- `html_only` means the exact revision uses the semantic HTML document path and
  has no alternate renderer. It is not a release claim.
- Queue authority is controlled independently by Rust-owned filing evidence.
- `release_ready` requires queue, visual, native output, and packaged-offline
  capabilities plus reviewed evidence.

The calibration viewer intentionally lists committed fixtures for both
HTML-routed and scaffold-only forms. Its labels expose those statuses rather
than implying that every visible form can be filed or shipped.

## Promotion gates

Before changing queue authority, verify the typed model, exact XML round trip,
formula and validation evidence, persistence, carry-over, amended-return
behavior, queue adapter, and desktop editor.

Before changing `visual_parity`, verify full-page comparisons against the
pinned exact-revision official PDF at the manifest DPI, exact page geometry,
critical regions, long values, maximum schedules, and zero clipping.

Before setting `release_ready`, record successful preview, system print, direct
PDF export, and packaged-offline operation on macOS, Windows, and Linux using
the same immutable semantic document.

Run the audit after every capability or route change:

```sh
npm run audit:forms:migration
```
