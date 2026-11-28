# r6-510-render — evidence renders of the Rule 6 strip composition

Evidence renders only; no product change. Rendered at commit
`4bd89c1dbe21f46cbcdc0887de9f63f30f20f057` (branch try/rule6-510-items19-20:
PR #510's head b70e42ef plus the print_path_render example), through the app's
own print path: `staticize(prepare_html_for(slug, filled_document(..), true))`,
never the raw frozen sheet.

## Commands

```
cargo run --locked -p bir-app-api --example print_path_render -- <slug> <out.html> key=value ...
# then Chrome DevTools Protocol Page.printToPDF (paperWidth 8.5in, paperHeight 13in,
# margins 0, printBackground, preferCSSPageSize false) so the PDF page is exactly the
# sheet's own 612x936 pt: one PDF point is one sheet point.
# then pdftoppm -png -r <72*zoom> -x -y -W -H for the crops.
```

Note on paper: the staticized document also carries a `preview-paper` style setting
`@page` to A4 (the preview's default paper). These renders print at the sheet's
native 612x936 pt instead, so that one PDF point is one sheet point and the crop
coordinates are exact; nothing else in the document differs.

## Fills

- R1 slug 1701-2018:
  - `frm1701:txtPg2IShed3_19Desc=SPECIMEN OTHER NON-OPERATING INCOME LONG DESCRIPTION WRAPPING OVER SEVERAL LINES OF PRINT`
  - `frm1701:txtPg2IShed3_20Desc=SPECIMEN ITEM TWENTY LONG DESCRIPTION ALSO WRAPPING OVER SEVERAL LINES OF PRINT`
- R2 slug 1601c-2018: `frm1601c:txt20Other=SPECIMEN LONG SPECIFY TEXT FOR OTHER MANNER OF PAYMENT WITH MANY WORDS TO EXERCISE WRAPPING`
- R3 slug 2551q-2018: `frm2551Qv2018:txt17Specify=` (same value)
- R4 slug 1700-2018: `frm1700:txtPg2I57Desc=` (same value)

## R1 numbers

- PDF page count: **4** (the blank form's own count). No page after 2 holds rows of
  page 2: page 3 opens with the form's own page-3 header ("Annual Income Tax Return /
  1701 01/18ENCS P3"); the overflow does not reflow — see the clipping note below.
- page-2 div height: **936 pt**.
- band `data-rule6-of="p2c134"` (item 19): style `top:822.98pt height:11.00pt`;
  its line's value: `WRAPPING OVER SEVERAL LINES OF PRINT` (font-size 8.50 pt).
- band `data-rule6-of="p2c139"` (item 20): style `top:849.74pt height:11.00pt`;
  its line's value: `OVER SEVERAL LINES OF PRINT` (font-size 8.50 pt).
- cell p2c134 first line: `SPECIMEN OTHER NON-OPERATING INCOME LONG DESCRIPTION`, font-size 8.50 pt.
- cell p2c139 first line: `SPECIMEN ITEM TWENTY LONG DESCRIPTION ALSO WRAPPING`, font-size 8.50 pt.
- largest bottom (top+height) of any div or SVG rect on page 2: **944.04 pt** —
  8.04 pt beyond the 936 pt page height.

## What the pictures show

- `r1a-page2-items15-22.png` — PDF page 2, crop x 15..580, y 694..836, zoom 2.5.
  Items 19 and 20 fill their own lines first, the continuations print on the added
  full-width lines with both amount columns greyed, and rows 21/22 follow directly
  with no dead band.
- `r1b-page2-items15-to-bottom.png` — same x, y 694 to the page's bottom edge.
  It ALSO shows the cost at the page bottom: item 25's row is cut mid-height at the
  page's edge — the two inserted lines (22 pt) pushed the bottom rows down and the
  page clips them (the largest content bottom, 944.04 pt, is 8.04 pt past the page).
  No row of page 2 appears on page 3: the clipped content is lost, not reflowed.
- `r1c-page2-whole.png` — the whole of PDF page 2 at zoom 1.5.
- `r2-1601c-item20-band.png` / `r3-2551q-item17-band.png` / `r4-1700-item57-band.png` —
  the band's row on each form (40 pt above the band div's top to 40 pt below its
  bottom, full page width, zoom 2.5). Each shows the specify field filled first and
  the continuation on the added line with greyed amount columns:
  R2 band top 415.27 pt on PDF page 1; R3 band top 446.59 pt on PDF page 1;
  R4 band top 462.09 pt on PDF page 2; each 11 pt tall.

## What the pictures do not show

- They are Chrome's print rendering of the staticized document, not a scan of a
  physical printout; font rasterization differs slightly from the app's own preview
  window, but the document bytes are the same ones the preview renders.
- The A4 `preview-paper` override is not exercised (see the note above).
