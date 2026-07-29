# formgen — PDF-native form conversion

A deterministic pipeline that converts one pinned BIR form PDF into an HTML
renderer by **reading the PDF's content stream**, not by looking at pictures of
it.

## Why this exists

The previous approach rasterised the official PDF to PNG and pixel-diffed it
against a Chromium screenshot of our HTML. That measurement made glyph outline
shape roughly 57% of the residual, and because the source PDFs do not embed
their primary faces, the "official" raster actually encoded *Poppler's
substituted glyphs* rather than BIR's typography. The gate was therefore
unreachable by proof, and 273 commits went into trying anyway.

The premise was wrong, not the effort. A PDF is a vector document. Everything
the previous approach was trying to *infer from pixels* is written down in the
file:

| Question | Raster answer | Content-stream answer |
| --- | --- | --- |
| Where is this border? | cluster dark pixels, guess | `re` operator, exact pt |
| How thick is it? | count pixels, off-by-one | exact `height` of the filled rect |
| Is it a rule or grey decoration? | ink presence — indistinguishable | literal fill value `0.0` vs `0.8509` |
| Which of two overlapping rects wins? | whatever the pixels ended up as | its position in the content stream |
| What font is this? | unknowable, it was substituted | `/BaseFont /Arial,Bold` |
| What size? | measure and guess | `Tf` operand |
| Custom letter spacing? | invisible | glyph origins vs `/Widths` |

So: stop rasterising. Extract, generate, and verify entirely in vector space.

## The pipeline

```
pinned PDF ──extract.py──► form IR (geometry + typography, exact pt)
                              │
                              ├──lattice.py──► box model: cells, regions, growable bands, combs
                              ├──fonts.py────► font plan: CSS face per run + advance-metric proof
                              │
                              └──emit.py─────► HTML + CSS (absolute pt layout, @page = MediaBox)
                                                  │
                                     Chromium print-to-PDF
                                                  │
                                              extract.py  ← the same extractor
                                                  │
                                             verify.py ──► IR-vs-IR numeric diff
```

The last step is the important one. **We compare extraction to extraction, not
raster to raster.** Our HTML is printed to PDF by Chromium, that PDF is parsed
by the *same* extractor, and the two IRs are diffed numerically:

- borders → matched by position and thickness, tolerance 0.25pt / 0.05pt
- text → matched by content, then compared on family, weight, style, size,
  origin, baseline and advance width
- images → matched by SHA-256, then by placement
- paper → exact equality, any difference fails immediately

Glyph outlines never enter the comparison. The unembedded-font problem does not
apply to a comparison that never rasterises a glyph.

## What "identical fonts" means here, precisely

The PDF gives exact family, style, weight and size. Those are reproduced
exactly. What cannot be reproduced is the *outline data* of a face that was
never embedded — but that was never what layout depended on.

Layout depends on **advance widths**. Arimo is metrically identical to Arial by
design, so `Arial 9pt` and `Arimo 9pt` place every subsequent glyph at the same
x. `fonts.py` proves this per run rather than assuming it: it computes what the
substitute face would advance and compares against the advance the PDF actually
recorded. If the deltas are not near zero, the substitution is wrong and we find
out on run one instead of after 273 commits.

Custom tracking is recovered the same way: measured advance minus the face's
natural advance, spread across the gaps, is the `letter-spacing` the generator
applied. Line height comes from the span's own ascender/descender, never from
the browser default.

The same argument, and the same proof, carries the corpus's serif: **Times New
Roman resolves to Tinos**, its metric clone from the same Chrome OS core-fonts
commission as Arimo and under the same Apache-2.0 terms. Tinos ships as four
static faces rather than one variable file, which is why a package is described
by candidate paths (`@fontsource/<name>` as well as `@fontsource-variable/<name>`)
and why the `@font-face` weight descriptor is derived from the loaded face's own
`fvar` instead of being the constant `100 900` — a static regular declared over
the whole range makes the browser synthesise the bold, inventing advances the
proof never covered.

A family with no mapping stays UNRESOLVED and is warned about by name, and so
does a mapped family whose package is not installed: that warning names the
package that would fix it. Nothing is ever quietly served by a different family.
Wingdings, Symbol, Tahoma, Candara and Berlin Sans FB Demi are a few hundred
characters of dingbats and bullets between them; they need per-character
substitution, not a font, and are deliberately left unresolved.

## Paper size

`@page` is set from the PDF's own MediaBox, per form. 2551Q is 612×936pt
(Folio); 0619E is 612×792 (Letter); others are 612×1008 (Legal). None of them
are A4. Forcing a single paper size would distort every form off its official
dimensions, so paper is per-form data — which is also what lets one script
handle all 35 without a table of special cases.

## Growable fields

Boxes are found first, then the ones that repeat are recognised as growable.
`lattice.py` looks for maximal runs of consecutive cell-rows sharing an
identical column signature at a constant vertical pitch. That is what an ATC
table, a schedule of income, or a list of creditable taxes *is* in the drawing:
the same row stamped N times.

Each growable band is emitted with its pitch, its official on-sheet capacity,
and one template row. At render time a band holds up to `capacity` rows on the
sheet; beyond that it spills to a continuation page, which is what the official
form does and what the existing `2551q-10-rows` fixture encodes. Everything
outside the band stays absolutely positioned, so growth cannot disturb the rest
of the page.

## Paint order

Two rects can overlap, and then the one painted later wins. That is a fact of
the content stream, not something to infer, so `extract.py` records the ordinal
of the op that painted every rule, area fill and image, and `emit.py` paints the
rule layer in exactly that order.

The order has to come from the *op*, not from the drawing: a path that both
fills and strokes is one `get_drawings()` entry and two paint ops, and PDF draws
the fill first and the outline over it. `get_bboxlog()` is the one view that
lists fills, strokes and images separately in stream order, so the ordinal comes
from there and is reconciled against `get_drawings()`; a mismatch raises rather
than falling back, because a plausible document with the wrong z-order is worse
than no document.

`emit.py` used to bucket the layer as fills → decorative greys → structural
black. That is a guess about z-order, and it is wrong wherever a form paints a
*lighter* rect after a darker one. 2552 draws the white knockout inside each
checkbox at op 4776 and a grey row separator crossing it at op 173, so the
bucket order put a light-grey line through every checkbox on the sheet. Tone
cannot tell you which rect is on top. Only the stream can.

## Anti-aliasing

The rule layer is anti-aliased. `shape-rendering="crispEdges"` looks like the
right choice for a page made of thin rules and it is not: it disables
anti-aliasing, so coverage becomes all-or-nothing against the pixel centre and a
rule thinner than one device pixel does not get sharper, it disappears. At
device_scale_factor 1 that erased every 0.24pt comb divider on 2552 page 1 —
which is to say, it erased the comb.

Anti-aliasing also happens to be what the official raster does, so a sub-pixel
rule lands as the mid-grey tone the source produces. Printing is unaffected
either way: print-to-PDF keeps the rects as vectors, and the IR round-trip does
not change by a pixel.

## Combs

A comb field (per-character boxes for TIN, dates, amounts) is drawn as an
enclosing box plus N−1 equally spaced 0.24pt dividers. It is emitted as **one
cell with N slots**, never as N separate containers — it is one field that
happens to be drawn with tick marks.

These 0.24pt dividers are exactly what the old raster pipeline saw as mid-grey
tone 83–153 and could not distinguish from decoration, because sub-pixel black
ink cannot fill a pixel at 144 DPI. In the content stream they are pure black at
an exact coordinate. The ambiguity was an artefact of the measurement.

## One sheet, two documents

A BIR sheet carries a form and a pile of reference material — ATC tables,
"Guidelines and Instructions", penalty schedules — printed on the same paper.
`guides.py` finds where the second one starts (17 of the 51 forms have an inline
guide region, mean 67% of the page it sits on) and `emit.py` emits either half:

```sh
python3 tools/formgen/emit.py --ir ... --layout ... --font-plan ... \
  --guide-plan build/guides/1603q-2018.guide.json \
  --document form  --out build/html/1603q-2018.html
python3 tools/formgen/emit.py --ir ... --layout ... --font-plan ... \
  --guide-plan build/guides/1603q-2018.guide.json \
  --document guide --out build/html/1603q-2018.guide.html
```

Three rules make the split free:

- **The form's page boxes never change.** A page whose lower 70% became empty
  keeps its full height, its place in the page count and its `@page` size. The
  freed space is what a growable band expands into; moving the page box to
  reclaim it would move every coordinate below.
- **Straddlers belong to the form.** An element crossing the cut is claimed by
  nobody, so it stays. Losing a rule off the form is a geometry regression; a
  duplicated rule on the guide is cosmetic. A growable band is indivisible and
  is awarded to the form whole for the same reason.
- **With no `--guide-plan` the output is byte-identical to what it was.**
  Measured across the corpus: 22 forms byte-identical, 17 shrink by exactly what
  their guide claimed, 12 gain only the 283-character cross-link.

The cross-link is `<a class="doc-link">`, absolutely positioned (so it is out of
flow and cannot push a page down) and `display:none` in print (so the document
verify.py measures does not contain it).

The guide does not need parity and one page is actively wrong with it: 1603Q's
guideline block is two columns of 6pt prose, and placing those as positioned
runs is what makes them overlap. `--guide-layout reflow` (the guide's default)
finds the columns from the run x-distribution — gutters at ≤12% of the page's
own peak coverage, then narrow slivers dissolved away — groups the runs into
reading order and emits flowing headings and paragraphs. A region whose columns
put ink on less than 60% of their width is a table, not prose, and is emitted
row-major as a real table on the undissolved gutter grid; across the 17 regions
that separates the two ATC tables (0.36–0.55) from the thirteen prose blocks
(0.61–0.95) with nothing in between. `--guide-layout absolute` keeps the
positioned form for anyone who wants the original arrangement.

## Determinism

Same PDF in, byte-identical HTML out. No timestamps, no randomness, no
dict-order dependence, no hand tuning per form. That is the property that makes
"convert the other 34 forms" a matter of running the script, and it is the thing
to protect above any individual form's score.

## Usage

```sh
python3 tools/formgen/extract.py \
  --pdf "/path/2551Q Jan 2018 ENCS final rev 3_copy.pdf" \
  --form-code 2551Q --revision 2018 \
  --expected-sha256 1f270ecf66d778836a14697863e420ff65d5ed0a5576a6cf58b97c9a8e8c9b24 \
  --out build/ir/2551q-2018.ir.json --summary
```

`--expected-sha256` is not optional in real runs. Every downstream artefact is
only meaningful relative to an exactly pinned source.

## Non-goals

- **No SVG page backgrounds.** Pixel-exact but static; a growable list cannot
  live inside one. That was the first solution and it is why we are here.
- **No raster references, ever.** Not as a gate, not as a diagnostic, not as a
  page background.
- **No per-form hand tuning.** A fix belongs in the algorithm or in the form's
  extracted data, never in a special case keyed on form code.
