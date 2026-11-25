# Proof files — independent check of pull request #315 at 6f5ddf72

Everything here is evidence for REPORT.md (same folder). The app ran on Linux
inside the ubuntu-24.04 docker sidecar in real Xvfb windows; the demo taxpayer
Juana Santos (TIN 000-123-453-00000) only; every value invented;
BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1 on every
run; BIR_SFTP_LIVE never set; Queue never pressed; nothing was sent anywhere.

| file | shows | does not show |
|---|---|---|
| flow-2550m.mp4 | one recording of the real flow in the app window only (43 s): open a new 2550M for June 2022, type 13A, the paired boxes filling, the Item 18 bases, typing over a filled box, opening the print preview | the system Save-as-PDF dialog (it crashes in this docker sandbox — an environment limit, not the app); clipboard paste |
| shot-before-13a.png | the Part II boxes before typing: 13B empty at its default, totals at 0.00 | — |
| shot-after-13a.png | after 13A = 123,456,789.12 is left: 13B filled 14,814,814.69 (exactly 12%), 16A 123,456,789.12 and 16B 14,814,814.69 recomputed; lock icons on 12A/12B/16A/16B, none on 13B | — |
| shot-after-18e.png | the Item 18 area after the bases are typed: each input tax filled at 12% of its base | — |
| preview-page-1.png, preview-page-2.png | the print preview's own sheet renders of a full invented return (1587×2242) | preview pages are the app's sheet images, not pixel-identical to the PDF raster |
| print-2550m.pdf | the saved PDF of the same filled return (2 pages, produced through the app's save-PDF action on the test save path because the real system dialog cannot run in this sandbox) | the system dialog itself |
| pdf-page-1.png, pdf-page-2.png | every page of print-2550m.pdf rasterized at 1400 px wide | — |
| official-page-1.png, official-page-2.png | the official blank form rendered from html-frozen/2550m-2007/index.html at 1400 px wide — the page the provenance (bir2550m.pdf, sha256 9fb4101a…) was frozen from, paired with pdf-page-N.png | the original binary PDF itself is not in any repository clone; this is its frozen HTML twin |
| xml-excerpt.txt | the upload XML a screen-filled draft produces (dry, never sent), including a typed-over 13B and the totals recomputed from it | the XML of the exact print-2550m.pdf return (the excerpt is a smaller second draft built for readability) |

Not here: the system save dialog (sandbox limit), an actual clipboard paste
gesture, the penalty items filled (25A–25C are computed-from-the-due-date boxes
in this app and the Edit penalties flow did not open inputs in the June-2022
scenario — see REPORT.md), the visible Check toast text for a pre-change draft
(the notice logic itself is verified in code and at the write path).
