# Geometry contracts

One contract per converted form, extracted from that form's **pinned official
PDF** by `scripts/extract_geometry_contract.py` and sha256-bound to the source
bytes recorded in `crates/bir-print/src/html_forms/form_*.rs`.

These are **ground truth for structure**, and they are tracked rather than kept
in scratch space for one reason: a contract nobody can read after `/tmp` is
cleared cannot be reviewed, and an unreviewable artifact is not evidence.

## What they are used for

- **Comb capacity** — `comb_candidates_*` records carry measured
  `interior_divider_x_pt` positions, so a declared cell count can be checked
  against the official dividers instead of guessed from a raster.
- **Stroke weight** — `rules_h` / `rules_v` records carry both `width_pt` **and
  the stroke's grey value**. This is the load-bearing part: a stroke with
  `gray ≈ 0.8509` (predicted tone ≈ 217) is near-invisible decoration, not a
  black rule. The raster localizer cannot tell the two apart and produced seven
  false "missing rule" findings across 1701, 1601C and 1701Q. **Never change a
  border from raster evidence alone — read the grey value here first.**
- **Text runs** — `text_runs` carry size, font and bold/italic flags, which is
  how the 2551Q footnote was found rendering at 7pt against an official 8.04pt.

## What they are not

Not runtime assets. Nothing in the shipped renderer loads these; they are
calibration and review evidence only, in the same category as the official page
rasters. They are also *candidate generation*, not truth by themselves — comb
detection can over-merge adjacent fields, and semantic naming is always human.

## Regenerating

```sh
rtk python3 scripts/extract_geometry_contract.py --repo . \
  --form-code 1601C --revision 2018 \
  --pdf <pinned pdf> --expected-sha256 <sha from form_1601c.rs> \
  --output <dir>
```

Extraction fails closed if the PDF's sha256 does not match the pin.
