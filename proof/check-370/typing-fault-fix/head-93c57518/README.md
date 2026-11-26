# head-93c57518 early round (Windows, buwiz-forms-win-3)

Binary: fix/money-box-keep-text-on-focus @ 93c575187749d8fa121c032bc489765c857810b3 (rev-parse confirmed),
own checkout C:\Users\Administrator\370early\buwiz-forms (git worktree sharing .git with 370acc), no local changes.
Build folder DISCLOSURE: the per-checkout HOME override silently never worked (Git Bash resets HOME), so this build —
like all rounds before it — went to the shared C:\Users\Administrator\BuwizNsh\target-nsh. The binary is genuine
93c57518 (log provenance "build: app 93c57518"; cargo fingerprints key on checkout paths), but folder isolation was
violated unknowingly. Orchestrator ruled 23:39: no rebuild; future builds set CARGO_TARGET_DIR explicitly per checkout.

## 1. B8 characterised (2551Q Sch1 row 1 amount, box holding 0.00, burst 15000.50)
Ground truth: per-run screenshot of the box after the burst + draft read after each run + reset confirmed 0.0 before next.

| entry, first key after entry | wrong runs | wrong-run shape |
|---|---|---|
| click, 0 ms | 20/20 WRONG | box EMPTY after burst (total loss); draft stays 0.00 |
| click, 50 ms | 0/20 | |
| click, 100 ms | 0/20 | |
| click, 200 ms | 0/20 | |
| Tab in (shift+Tab), 0 ms | 0/10 | |
| Tab in, 50/100/200 ms | 0/10 each | |

Window closes between 0 and 50 ms. Keyboard entry immune at all timings. b8-shots/: empty vs filled box evidence.
370b8-results.txt: per-run reset/saved values.

## 2. Commas (2551Q Sch1 row 1 amount)
- a. 1,234.56, caret between 2 and 3, type 9 → saved 12934.56 (12,934.56). PASS, no sentence.
- b. End, 4xBackspace (text 1,23), Tab → saved 123.0 (123.00). PASS, no sentence.
- c. 0.00 box, type 1500,50 → saved 1500.5 (1,500.50). PASS, no sentence.
- d. 0.00 box, type 1,500 → saved 1500.0 (1,500.00). PASS, no sentence.
- e. type 15,00,50 → box back at 0.00, saved 0.0, sentence verbatim:
  "Taxable Amount: 15,00,50 is not an amount. The box kept 0.00. Use a point for centavos, and commas only between thousands." (370case-e.png)
- f. box 1,234.56, select all, paste `1500,50` → REFUSED as unreadable: box keeps 1,234.56, saved 1234.56, sentence
  "Taxable Amount: 1500,50 is not an amount. The box kept 1,234.56. Use a point for centavos, and commas only between thousands."
  (case-f-paste-refused.png) — DIFFERS from the typed-comma case c which is accepted. Not the expected 1,500.50.
- g. box 1,234.56, select all, paste `1,5` → saved 15.0, box 15.00, no sentence (case-g.png). Confirms the known limit.

## 3. Recomputed box (2550M Part II: 1000 into 13A, Tab into recomputed 13B, burst 999)
per-run saved values:
- idle, first key 0 ms after Tab: 10/10 saved 999.0
- idle, first key 120 ms: 10/10 saved 999.0
- CPU load (8 spinner processes), 0 ms: 10/10 saved 999.0
- CPU load, 120 ms: **7/10 FAULT — saved 120.01** (the 999 appended into the recomputed 120.00 as `120.00999`; runs 2,6,9 correct).
  fshots-load/d120-run1.png shows the box literally displaying `120.00999` mid-type — the same shape Linux reported.

## 4. 1601FQ tax rate box (new Q4 2026 draft, item 4 Any Taxes Withheld = Yes)
Part II rows 14-19 are the ATC rows. With ATC WI240 chosen, the Tax Rate box remains a greyed display-only placeholder
("Shows the ATC's rate once this row's ATC is chosen") even after the ATC is chosen: clicking it and typing 42 produced
nothing — the box does not take focus or keys. No row of Part II let me type a rate at all at this head.
Side observation: typing a tax base before the ATC is accepted produced the sentence
"Item 14: you typed 10,000.00; the return holds no amount. Choose this row's ATC first." — a correct visible refusal.

## 5. Fold-away box (2550M Part II 13A)
Typed `12,3 4` (unreadable) into 13A, folded Part II from the navigator without leaving the box, waited, reopened:
13A shows 1,000.00 red-bordered, sentence present verbatim:
"Item 13A: 12,3 4 is not an amount. The box kept 1,000.00. Use a point for centavos, and commas only between thousands."
(foldaway-reopened.png). The unreadable text was not saved. Unmounting counts as leaving. PASS.
