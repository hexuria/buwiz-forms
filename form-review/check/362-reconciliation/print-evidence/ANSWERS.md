# Answers to the three evidence defects (re-check round 2)

## 1. How the copy happened, and the corrected files

Mechanism, reproduced exactly: the evidence was staged into the repo with one
multi-source `cp` whose sources collided on basenames
(`evidence/merge/crop-1801-p1-items10-15.png` and `evidence/362/crop-1801-p1-items10-15.png`
in the same invocation, likewise the two `analysis.txt`). GNU cp refuses to overwrite a
file it created earlier *in the same invocation* — it prints
`cp: will not overwrite just-created 'dest/x' with 'sub/x'` and keeps the FIRST source.
The merge file won; the 362 file was silently skipped because the command ended with
`2>/dev/null`, which hid the warning. My renames then mislabeled merge's bytes as 362's.
The underlying evidence files were never wrong — evidence/362/*.html were and are genuine
dfdb0664 outputs — only the repo copies were. (Reproduced in /tmp: first source wins,
warning printed, second skipped.)

Corrected now by re-running the whole chain at dfdb0664 in its own worktree (wt-362,
docker harness, same fill): fresh prints reproduce the first run byte for byte
(1801 sha256 0be38be953a18ca2 both times; byte counts 902651/975409/569480/852026
identical), which is itself reproducibility evidence. Re-rendered PDFs, re-cropped the
same region, re-ran the analysis — all copied with distinct names this time:
- crop-362-1801-p1.png  sha256 996366eb…  (genuine 362: squashed microscopic one-line text)
- crop-merge-1801-p1.png sha256 b0e2e320…  (merge: two lines then "…")
- analysis-362.txt: pages 2/2/2/3, 0 continuation pages, 0 shortened values — as the verdict says.

## 2. The per-value accounting at the merge (full detail in shortened-accounting.txt)

The earlier numbers mixed units: "68" was a raw count of the literal string `...`
anywhere in the html (including pre-printed sheet text), "52" was overflow-warning
attributes of all kinds (including amount-comb digit overflow, which is not text
shortening). Counting the right units — fill values visibly shortened with "…" on the
form versus full-entry rows on the additional sheets — the reconciliation is exact:

| sheet | shortened with "…" on the form | full-entry rows on additional sheets | full text printed nowhere |
|---|---|---|---|
| 1800 | 50 | 50 (Page 1: 10, Page 2: 40) | 0 |
| 1801 | 53 | 53 (Page 1: 9, Page 2: 44) | 0 |
| 2200S | 0 | 0 | 0 |
| 2200T | 19 | 19 (Page 1: 7, Page 3: 12) | 0 |

Every shortened value's full text prints on an additional sheet exactly once; the row
names its page and item (e.g. 1801 Page 1 "If yes, specify"; "Item 25 — Cash/Bank Debit
Memo: bank"). **No shortened value loses information on any of the four sheets.**
2200S has no shortened text at all: its only overflow warnings are amount-comb digit
overflow (the invented amount is wider than the comb), which the fitting rules do not
cover by design. Honest limitation of the check: the invented long values were one
uniform 200-character string in every long-text cell, so the per-row match is verified
by count, by prefix, and by page/item labels rather than by distinct per-cell content;
distinct shortened texts (9/20/6 per sheet — emails, names, the long string truncated
at different lengths) were each individually found in full.

## 3. Why the first count read 0

Needle mismatch, my error: the print output uppercases every fill value (the sheets
print in capital letters), so the mixed-case needle `Abcdefghij…` literally never
occurs — count 0 — while the values were there all along. Recounted correctly against
the uppercase needle, additionally flattening `&#10;` newline entities and tags so
line-wrapped occurrences rejoin: 1800: 50, 1801: 53 raw (54 flattened — one value wraps
across a line break inside its additional-sheet cell), 2200T: 19, 2200S: 0 — matching
the additional-sheet row counts exactly. The flattened recount is in recount-uppercase.txt
and the per-sheet accounting in shortened-accounting.txt.

## Supplement (same harness, completed later): 1600PT at main ee3e649d

The main-head app print completed after the openssl build was retried (its first failure
was the disk-full episode, now cleaned). With the same fill at all three heads: main prints
the 2 paper pages only, 0 overflow warnings (byte-level consistent with #362-alone, whose
bir-print-level filled document is byte-identical to main's); the merge prints 5 pages with
15 warnings and its additional sheets. 1600PT-main.pdf added. As noted in the verdicts, the
schedule rows 13-17 stay empty at all three heads under the generic fill (valid ATC catalog
entries needed) — a harness limitation, identical everywhere, not a regression.
