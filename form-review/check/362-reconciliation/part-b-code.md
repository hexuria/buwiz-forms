# Part B — reconciliation claims (8225f8a4 = #362 dfdb0664 + #355 a067f980)

## Claim: every 2200t-2022 reference in #355's code moved to 2200t-2020 — HOLDS
grep of crates/, app/, tools/ at a067f980: 10+ files carry `2200t-2022` (frozen_html.rs,
continuation.rs, build.rs, bir_module.rs, all the goldens fixtures). At 8225f8a4: ZERO references
to 2200t-2022 anywhere; the same files reference 2200t-2020 instead (plus tests/frozen_sheet_faults.rs).

## Claim: cell lookups made attribute-order-independent — HOLDS
`cell_tag()` (crates/bir-print/src/frozen_html.rs at the merge) finds `id="<cell>"` anywhere in the
opening `<div` tag and scans back to the `<div`, rejecting matches inside a prior tag. Its doc
comment: "The 2020-generation sheets write `class` and `data-*` before `id`; a naive `<div id=`
search finds nothing on them." Every former `html.find(&format!("<div id=\"{cell}\""))` site
(plain_cell_dimensions, comb_cell, has_preprinted_percent_beside, cell_box_pt,
preprinted_decimal_mark, strip_page_rect_separators) now goes through cell_tag/cell_div_open.
Verified in the diff a067f980→8225f8a4.

## Claim: 0.4092 was the 2022 sheet's word-spacing, recomputed 0.0 for 2020 — HOLDS
html-frozen/2200t-2022/form.css at #355 literally carries `word-spacing:0.4092pt` in `.fi`.
html-frozen/2200t-2020/form.css at the merge carries NO word-spacing declaration at all — so 0.0
is the correct value for the 2020 sheet, not just a relabel. The test
`each_sheet_measures_in_its_own_face_and_spacing` now pins ("2200t-2020", 0.0).

## Claim: about 11 sheets that neither PR re-freezes have overflow goldens differing from #355's — HOLDS (exactly 11)
overflow-goldens.json a067f980 vs 8225f8a4: 14 of 43 sheets differ. Three of them ARE re-frozen by
#362 (1800-2018, 1801-2018, 2200s-2018 — expected). The remaining exactly-11 non-re-frozen:
1600-pt-2018, 1600-vt-2018, 1601-fq-2020, 1601c-2018, 1601eq-2019, 1602q-2019, 1700-2018,
1702ex-2018, 1702rt-2018c, 2000-dst-2018, 2551q-2018.
Note: the goldens are output HASHES (64-hex strings), so "no value moving" cannot be read out of
the fixture itself — verified at print level in part C/D instead. Also verified: the fitting
goldens (fitting-goldens.json, fitting-band-goldens.json) for 1600-pt-2018 are IDENTICAL between
#355 and the merge — the fitting rules' output did not change there.
