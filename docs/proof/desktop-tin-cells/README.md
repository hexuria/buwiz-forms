# TIN-cells investigation — origin/main 3aceafe8, desktop `just nsh-run`, scratch db

Typed digits were the nine keys 0 0 0 1 2 3 4 5 3 in every case. Slow = one discrete key press at a time, ~0.5-0.6 s apart. Fast = one computer-tool `type` action sending the whole string in a single rapid burst.

- `tin-1-click-first-cell-slow.mp4` — Case 1: clicked the first cell, slow keys. CORRECT: cells show 0,0,0-1,2,3-4,5,3 in order.
- `tin-2-tab-into-cells-slow.mp4` — Case 2: no click on the cells; Shift+Tab into them from the name field, slow keys. CORRECT: same in-order result.
- `tin-3-click-first-cell-fast.mp4` — Case 3: clicked the first cell, all nine digits in one fast burst. WRONG: cells show 0,0,1-2,3,4-5,3 with the third digit 0 displaced to the last cell.
- `tin-3-click-first-cell-fast-retry1.mp4` — Case 3 retry 1: identical wrong result (2/2).
- `tin-3-click-first-cell-fast-retry2.mp4` — Case 3 retry 2: identical wrong result (3/3, deterministic).
- `tin-4-click-middle-cell-slow.mp4` — Case 4: clicked the FIFTH cell, slow keys. ANOMALY: digits do not start at the clicked cell — they fill from cell 1 in correct order; the click position inside the cell group is ignored.
- `tin-1-final.png` / `tin-2-final.png` / `tin-3-final.png` — final cell states for cases 1-3.

Supplementary burst-length probes (not recorded, screenshots in the report): fast bursts of 2, 3 and 4 digits are CORRECT; a 5-digit fast burst is already WRONG — the fifth keystroke is processed as if focus were back at cell 1, inserting there and shifting the correct earlier digits right (typed 00012, cells show 2,0,0-0,1). The corruption therefore begins at the FIFTH keystroke of a continuous fast burst.
