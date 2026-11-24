# mac-pdf — minimal reproduction of the saved-PDF text defect

`frag.html` is the smallest fragment that reproduces the macOS saved-PDF
defect: a `.sheet` (595.28pt) containing a `.page` at `zoom:0.878` — exactly
how every frozen form fits its authored page onto A4 — with one 6.96pt
signature-column run followed by a 6.48pt column at the real 1606 geometry
(gap 253.37pt), a long name in an `overflow:hidden` clip box, and a bold
label with an italic annotation.

Both pictures are produced by the same hidden-WKWebView `createPDF` /
snapshot pipeline the app uses (probe.swift, frame 793.7x1122.5 CSS px):

- `frag-capture.png` / `frag-pdf.png` — document zoom 1, effective page
  zoom 0.878 (what the saved PDF and the paper print contain): "Tax Agent"
  collides into the next column; the name is clipped to "…DELA C".
- `frag-zoom1-snapshot.png` — `html{zoom:1.139}` so the effective page zoom
  is 1.0 (what the preview's Fit-width zoom ≈1.2 approximates): the same
  DOM fits, "Tax Agent" ends 5.7pt before the next column, the name reads
  "…DELA CRU".

Layout widths of the first run (getBoundingClientRect, viewport px):

| effective page zoom | width | in .page units | gap before next column |
|---|---|---|---|
| 0.878 (capture/PDF) | 320.00px | 273.5pt | 253.37pt -> OVERLAP 20pt |
| 1.000 (natural)     | 330.16/1.139px | 247.6pt | 253.37pt -> fits |

Same DOM, same fonts — only the document zoom differs. WKWebView text
metrics are zoom-dependent: runs at font-size 6.96pt render ~11% wider at
effective zoom 0.878 than at 1.0. The preview's fit-zoom (~1.2) lands near
effective 1.0 and masks the defect; the hidden capture renders at zoom 1
and bakes it in.
