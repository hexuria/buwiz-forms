# 2200A batch-4 print check (head f8dc14c5 vs pre d1777a31 vs main 92b3e201)

- 2200A-head.pdf / 2200A-pre.pdf / 2200A-main.pdf — the same saved draft (data-excerpt.txt)
  printed to PDF in the running app at each commit. head prints Part IV rows 25-28;
  pre and main print them blank (the commit tell). 2200A-head-empty.pdf — a second pass
  with every Part IV field cleared: all four rows stay blank.
- pageN-<commit>.png — every PDF page rendered at ~1400 px wide.
- crop-part4-head.png / crop-part4-main.png — the Part IV region close-up at both commits.
- crop-row26-amount-head.png — the amount column of row 26 close-up.
- recording.mp4 (55 s) — the head app: review the saved return, open print preview,
  Save as PDF. Shows the flow only, not pixel proof.
- results.tsv — one line per box/case with the verdicts.
- data-excerpt.txt — the draft's held values and the arithmetic.

What these files do NOT show: pixel-perfect diff of non-Part-IV regions (the
other-boxes-unchanged evidence is a pdftotext -layout diff: only Part IV lines differ);
the upload comparison (in part-d files at the folder above); preview-vs-PDF consistency
(the PDF was saved from the same preview pipeline; no separate preview capture).
