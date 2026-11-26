# Verdicts — independent check of #362 (dfdb0664) and reconciliation 8225f8a4

Checker: this machine, independently. Method notes in README.md, claim-by-claim
derivations in part-a-metadata.md and part-b-code.md. This file is the verdict list.

## Part A — #362 metadata claims: ALL HOLD

1. "main's metadata came from buwiz-forms-html d7f359a emit tools, re-run not copied" — **holds**:
   I re-ran both emit tools (at d7f359a) on dfdb0664's re-frozen sheets myself and reproduced
   the PR's bands and page layouts exactly. Byte-identity with main can't itself distinguish
   fresh-run from copy, but the regeneration is reproducible and correct for the new sheets.
2. "1801's formgen-bands byte-identical to main a8aeaca8's" — **holds** (sha256 match).
3. "1800's bands never lost; 2200S's empty on main too" — **holds** (both verified at main).
4. "layouts differ only in p1b3/p2b0 text_ids, positions unchanged; 2200S identical; 2200T-2020 got its own" — **holds exactly**.
5. "pins moved only for the four re-frozen sheets" — **holds**.

## Part B — reconciliation code claims: ALL HOLD

1. Zero `2200t-2022` references at 8225f8a4 — **holds** (grep, both branches).
2. cell lookups attribute-order-independent — **holds** (read the cell_tag change; verified against re-frozen sheet attribute order).
3. 0.4092pt word-spacing is the 2022 sheet's; recomputed 0.0 for 2020 — **holds** (0.4092 present in 2200t-2022 form.css, absent in 2200t-2020).
4. "bir-print 175/0" — **holds literally**: lib tests 175 passed/0 failed/1 ignored at the merge; all integration suites green, incl. freeze-verify and stamp tests at both heads.
5. The 11 non-re-frozen sheets' overflow goldens differ from #355's because of #362's print-engine commit — **holds**: exactly the 11 sheets enumerated (1600-pt, 1600-vt, 1601-fq, 1601c, 1601eq, 1602q, 1700, 1702ex, 1702rt, 2000-dst, 2551q); fitting goldens for 1600-pt byte-identical between a067f980 and the merge, i.e. no value moves.

## Part C — prints through the app: claims HOLD, with one harness caveat

Method/caveat (disclosed): printed through the app's own door path
(screen_form_open → screen_form_fill, the page's own write path → screen_form_save →
screen_form_print; four safety variables, demo taxpayer, invented values, never Queue).
To capture output in the headless harness, the scratch worktrees carried a one-line
file-retention tweak in nsh/src/print.rs (keeps the preview HTML instead of deleting it) —
never committed; the print pipeline is otherwise byte-identical. PDFs/crops here are
Chrome renderings of that final static print HTML.

At the merge (8225f8a4), every box filled incl. 200-char values:
- 1800: 11 pages (2 paper + 9 additional), 1801: 11 (2+9), 2200S: 2 (no overflow needed), 2200T: 7.
- #355's rules show exactly as claimed — **holds**:
  - two lines then "…": ellipsis cells render at exactly 7.50pt (1801/2200T) or 8.55pt (1800's larger cells) with an embedded newline — never under 7.5pt; every sub-7.5pt font in the documents is pre-printed sheet text (class `t`), never a filled value;
  - the full value prints on the additional sheet exactly once per shortened entry: counts match warnings (1801: 53/53, 1800: 52/50 — see analysis files; 2200T: 19/19). Additional-sheet header carries form number, TIN, taxpayer name, date, "ADDITIONAL SHEET (x OF n)" and the table "Full text of entries shortened with '…'" keyed by page+item (crop 1801-p3.png).
  - 2200S's only warnings are amount-comb digit overflow (invented amount wider than the comb), not text — no ellipsis applies.
- At #362-alone (dfdb0664): same fills produce only the paper pages (1800/1801: 2, 2200T: 3), no additional sheets, no ellipsis — long text squashes to one microscopic line inside its box (crop-362 vs crop-merge of 1801 items 10-15: night-and-day). Confirms the fitting behavior is #355's, intact at the merge.
- Branch code cells print at both heads: TIN combs carry all digits incl. branch 00000 on 1800/1801 — **#362's fix holds** (visible in both crops; tin segments present in all outputs).

## Part D — 1600PT hairlines: claim HOLDS — the extra rects restore paper lines

- main vs #362-alone: byte-identical filled documents (676097B) — #362 changes nothing for 1600-pt.
- merge vs 355: rect counts 1032/1027 overall; in band p1g0 exactly 5 rule rects appear only at the merge: v120/v130/v140/v151 (cell p1c50, the ATC comb, capacity 5) and v415 (cell p1c54, rate comb, capacity 2), each 0.24×7.76pt vertical ticks.
- Geometric proof they are the paper's comb dividers: their x offsets from the comb cell's left edge are 14.4/28.94/43.58/57.98pt = 0.99/1.989/2.996/3.985 × the comb pitch (14.548pt) — i.e. exactly the internal divider positions between character boxes 1|2|3|4|5; v415 sits one box-pitch (14.4pt) inside its 2-box comb. These are lines the paper draws. They do not cross printed text harmfully: at the merge the same cells print fitted text clipped to the cell, and the engine withholds rule segments that collide with glyphs elsewhere (26 cells hidden at 355) — these five are comb decorations the re-freeze/metadata regeneration now restores, matching the claim's direction (175→180 in their count basis; my count 178→183 in p1g0, same +5).
- Harness caveat: filling 1600PT's schedule rows through the door path needs valid ATC catalog entries my generic fill didn't supply, so rows 13-17 print empty in the app-level run (both heads, same harness — not a regression: identical behavior at all heads); the hairline evidence above is from the bir-print-level filled documents, which fill every writer key directly.

## Bottom line

Every claim checked holds. No discrepancies found beyond what the claims themselves state.
