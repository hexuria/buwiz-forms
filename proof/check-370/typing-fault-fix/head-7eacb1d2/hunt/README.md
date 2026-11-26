# hunt H1-H7 at 7eacb1d2 - partial (H4 pending)
H1 click+type gap0 (first key within ms of click): keys VANISH - box empty, saved 0.0, 6/6 runs incl 99999.99/100000.01/1000000.005. h1zero-full.png: keys went nowhere visible. gap>=50ms: all land correct. Tab-entry gap0: fine (15000.5). Failure mode = visible input loss, nothing wrong saved.
H2 burst typos: 15000..50 -> saved 15000.5 RIGHT; 150a00.50 -> saved 15000.5 RIGHT. digits after refused key NOT lost (review's prediction not reproduced).
H3 page-driven focus: pill chain lands in TEXT boxes (Contact Number) with caret; no incomplete editable money box reachable cheaply on 2551Q. inconclusive.
H5 int box (2551Q item5 Number of Sheets, holds 0): click type 42 -> key1 '4' shows "04" already (digit prepends before the zero!), box after = "04" - NEITHER 42 nor 420. FAULT.
H6 = W22b: comma visible while typing, dropped at parse -> saved 1500050.0 for typed 15000,50. FAULT confirmed.
H7: mismatch message never appeared anywhere.
H4 (2550M Nov 2026 Part II item 13B Output Tax Due, editable, recomputed from 13A base): base 1000 Tab -> 13B 120.00 focused NO selection -> type 999 -> after leaving 120.01 (120.00999 rounds). typed amount silently lost. FAULT 1/1 normal speed. crops: h4-13b-focused-noselection.png h4-after-999-shows-12001.png h4-full-after-tab.png. cause: recompute write into the box at/after focus destroys platform select-all.
row-delete: visually PASS (remaining row PT010 2,000.00 60.00 no error) BUT draft schedule_1 keeps stray empty row[0] (no atc 0/0) - deleted row blanked not removed? or pre-existing artifact; UI shows '1 row'.

## H3 - CONFIRMED FAULT on Windows (candidate build, 7eacb1d2+(b))
route (as found on Linux): 2551Q Part II item 20 Surcharge made editable via Edit penalties, filled 1,500.00, red-flagged (EOPT tier missing). form navigation: 'next' pill twice then bottom-pill arrows to '4 of 4 - Item 20 - Surcharge'. focus IS driven into the box programmatically. trace (MFTRACE fp-frm2551Qv2018:txt20):
  onFocus-begin "1,500.00" value 1500.00  <- NO selChange events at all after focus: programmatic focus() gives caret-at-end, no select-all, no collapse
  change "1,500.002" / "1,500.0025" / "1,500.00250" / "1,500.002500"   <- typed 2500 APPENDS at the end
  onBlur "1,500.002500" -> writeText "1,500.00"   <- blur rewrites the original amount; typed digits swallowed, draft holds 1500.00
verdict: FAULT - typed amount silently lost, same as Linux. mechanism differs from the recompute fault: programmatic focus produces no selection events at all, and no outside write is involved, so candidate (b) cannot cover it; a programmatic focus into a filled box needs its own select-whole.
## part2 additions: click on recomputed 13B then 999 -> 999.00 saved (native click select-all). Shift+Tab path 3/3 -> 999.00 (note: harness passes forward through 13B first; re-entry replaces via natural select-all).
## H8 minus (2551Q item20): -1500.50 -> shown -1,500.50 + red 'Must be zero or greater', saved -1500.50 (visible, not silent). 1500.50- -> trailing minus dropped, positive 1,500.50 shown/saved. 15-00 -> mid minus dropped, digits join, 1,500.00 shown/saved. all outcomes visible after leaving.
