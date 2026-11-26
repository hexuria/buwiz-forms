# Part A — metadata claims (PR #362 dfdb0664 vs main a8aeaca8)

## Claim: 1801's regenerated formgen-bands is byte-identical to main a8aeaca8's — HOLDS
sha256 of the extracted formgen-bands JSON: main 2ac9b58cf597b919…, PR 2ac9b58cf597b919… — identical.

## Claim: re-ran both tools rather than copying — SUPPORTED (reproducible regeneration)
Independent re-run of emit_band_metadata.py + emit_page_layout.py (buwiz-forms-html
forms-layout-fixes @ d7f359a) on dfdb0664's re-frozen 1801-2018 sheet reproduces the PR's
formgen-bands AND formgen-page-layout exactly (JSON-equal in full). Note honestly: because the
output is byte-identical to main's, the bytes alone cannot distinguish a fresh run from a copy —
but the regeneration is REPRODUCIBLE, i.e. the metadata is correct for the new sheet, which is
what matters. One mechanism detail found while re-deriving: structural_rects on band rows are
added by emit_page_layout.py (BAND-FORMAT.md §6), not emit_band_metadata.py — running only the
first tool produces bands missing structural_rects, so "both" is load-bearing, consistent with the claim.

## Claim: 1800's bands were never lost; 2200S's bands are empty on main too — HOLDS
1800-2018 formgen-bands sha256 identical main vs PR (7b74386cbcf7548d…).
2200s-2018 formgen-bands is the empty array `[]` at BOTH commits (aab28b96fc7fc834… identical).
2200t-2022 on main also had empty `[]`; 2200t-2020 in the PR has NO formgen-bands script at all
(sheet carries no growable tables — consistent).

## Claim: page layouts differ only in two header bands' text_ids (p1b3, p2b0), positions unchanged — HOLDS EXACTLY
Compared full layout JSON with text_ids stripped: identical for 1800-2018, 1801-2018 and 2200s-2018.
The ONLY differences are text_ids of pages[0].bands[3] (id p1b3) and pages[1].bands[0] (id p2b0):
- 1800 p1b3: main 13 ids p1t29..p1t41 → PR 8 ids p1t34..p1t41
- 1800 p2b0: main 9 ids p2t0..p2t8 → PR 8 ids p2t0..p2t7
- 1801 p1b3: main 9 ids p1t30..p1t38 → PR 8 ids p1t31..p1t38
- 1801 p2b0: main 13 ids p2t0..p2t12 → PR 8 ids p2t0..p2t7
(text renumbering from the January-2020 sheet's different header decomposition; positions untouched.)
2200s-2018: fully identical including text_ids. 2200t-2020 got its own layout (3 pages, version 1) — HOLDS.

## Claim: pins moved only for the four re-frozen sheets — HOLDS
html-frozen/inventory.json: exactly 4 index_sha256 changes + the 2200t-2022→2200t-2020 slug rename.
writer-cells.json changed for 1801/2200s/2200t (sheet re-freeze content, not pins).
