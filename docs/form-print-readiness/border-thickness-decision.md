# Border thickness: uniform, by owner decision (2026-07-21)

## The decision

**All rendered borders and rules use ONE uniform width. All double-width
(2px / 3px) section- and part-boundary borders are removed.** This is an
explicit product-owner decision, made with the fidelity tradeoff stated in
full.

This **overrides** the "match the official per-section thickness" guidance for
border weight specifically. It does not override anything else — grey-vs-black
rule detection, charbox capacity, static text, artwork, layout, and every
fail-closed rule remain in force.

## Why this note exists

The official BIR forms genuinely use heavier part/section boundaries
(≈1.44–1.92pt → 2–3 device px) against thin interior rules (≈0.48–0.75pt →
1 px), and the Wave 0 weight pass spent real effort matching that per the
geometry contract. The owner reviewed the result, judged the varying weights
inconsistent/hard to get right, and chose a single uniform weight instead —
prioritising a clean consistent look over per-boundary fidelity.

Without this note, a later agent reading CLAUDE.md, `wave0-diagnostic-review.md`,
or the geometry contracts would see uniform borders as a regression against the
official weights and re-derive the heavier boundaries — undoing the decision.
**Do not.** Border weight is uniform by design. If the owner later wants the
official per-section weights back, that is a new decision recorded here.

## What "uniform" means in practice

- Every border / horizontal rule / vertical rule / box outline renders at **1
  CSS px** (the crisp single-pixel line). Nothing renders at 2 or 3 px.
- The heavy thousands-group separators inside money combs (made 1.5pt in the
  Wave 0 pass) also collapse to the uniform width — they were a per-official
  weight distinction, which this decision removes.
- Displacement compensations that the Wave 0 pass ADDED to counteract thicker
  borders (padding/height/caption nudges) are removed alongside the border they
  compensated, so content returns to its natural position.

## What is unaffected

- **Whether a rule exists at all** is still contract-driven: a grey decorative
  fill (`gray ≈ 0.8509`) is still not a black rule and still gets no border.
  Uniform thickness changes the WIDTH of rules we draw, never WHETHER we draw
  them. Read `border-thickness-decision.md` and the grey rule together.
- The complete-page number, structural-ink and cell-edge components still
  compute and report as before; none of them gate.

## Related change landed together: charbox right-most divider

Independently of thickness, the right-most character-cell divider is removed
wherever it coincides with the field's own box border (it drew a doubled line
on the border). Combs now draw interior dividers only (`:not(:last-child)`),
letting the box border provide the outer edge — except where the last divider
is genuinely the boundary with a following element (a separator column, an
adjacent comb), which is kept.
