# N4 evidence: 1701 page 2 items 19/20 — truncate, or admit one 14 pt strip?

Evidence only, for the owner's decision on the 14 pt continuation-strip cap.
**Nothing here is a proposed change**: picture (B) is rendered at commit
`2e324dd62` of this branch, which carries the one-strip scratch change
(`rule6_band_lines_cap` admits one strip into 0.6 pt of the 4 pt bottom
reserve); that change is committed HERE ONLY, never on #495. Picture (A) is
rendered at #495's head `eeb286291`. The two heads differ ONLY in the cap
line, and both pictures are byte-stable across the round-4 amendments. The
fill is the same in both pictures:

```
frm1701:txtPg2IShed3_19Desc=SPECIMEN OTHER NON-OPERATING INCOME LONG DESCRIPTION WRAPPING OVER SEVERAL LINES OF PRINT
frm1701:txtPg2IShed3_20Desc=SPECIMEN ITEM TWENTY LONG DESCRIPTION ALSO WRAPPING OVER SEVERAL LINES OF PRINT
```

Both pictures are rendered through the committed print-path command —

```
cargo run --locked -p bir-app-api --example print_path_render -- 1701-2018 out.html key=value ...
```

— `staticize(prepare_html_for(slug, &filled_document(..), true))`, Chrome
`--print-to-pdf`, cropped at identical coordinates (pdf page 2, rect
x 15..580, y 694..836, zoom 2.5). Rows 18 to 25 print whole in both; row 25
is the last row of the page, and its bottom border and the white reserve
below it are inside the crop.

* `n4-A-1701-p2-items19-20.png` — **(A) the cap as it stands** (#495 head
  `eeb286291`): with 13.4 pt of room below item 19's row, no 14 pt strip is
  ever admitted. Item 19's cell text is cut after the whole word LONG —
  "SPECIMEN OTHER NON-OPERATING INCOME LONG..." — and item 20's after
  WRAPPING; the whole values print on the additional sheet.
* `n4-B-1701-p2-items19-20.png` — **(B) scratch build, evidence only**: the
  cap admits one strip, which spends the 13.4 pt of room plus 0.6 pt of the
  4 pt bottom reserve (3.4 pt of the reserve remain). Item 19 prints whole —
  its cell line plus one grey full-row continuation line — and item 20 still
  truncates, because the one strip consumes the shared budget (13.4 - 14
  < 0). Rows 21 to 25 keep their height and nothing prints off the page.
