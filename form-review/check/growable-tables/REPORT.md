# Growable tables — the facts

Research on main ee3e649d, report only; no app code changed. Method and evidence limits in README.md.
Line references `L<n>` are lines of the official decompiled HTA source as recorded in the rule-package
inventories (rules/forms/<form>/fixtures/); rust references are file:line in this repository at ee3e649d.

## Official side, all forms (two universal facts first)

1. **No maximum-row check exists anywhere in the official validation inventories.** Scanning every
   alert text of every inventoried validation function of all 43 forms, the only "Cannot add row"
   alerts are incomplete-row guards ("Cannot add row. There is an empty description on …",
   e.g. 2200m-v2018 CheckEmptyDivDesc L3637, 1707-v2021 L3221, 1702ex-v2018c L4188, 2200p-v2020 L4835,
   2552-v2018 L3993, 2200c-v2018 L2814). No alert mentions a maximum, a limit, or a count.
2. The rule packages classify the row families as **unbounded** in so many words:
   1601c "eight source-unbounded Schedule I field families" (README), 1700 "59 unbounded runtime field
   families", 1701a "six unbounded Part IV modal families", 1702ex "11 unbounded popup families",
   1702mx "83 active unbounded runtime families", 1702rt "19 active unbounded modal field families",
   1707 "6 unbounded popup families", 1707a "11 unbounded schedule families", 2200a/2200s "eight
   unbounded Schedule 1 Others families", 2200m "17 unbounded Schedule 1 families", 2200p "15",
   2552 "24 unbounded schedule families", 2550m/2550q "unbounded indexed families".
   What bounds the upload in practice: nothing but the serializer — `saveXML` walks `frmMain.elements`
   and emits every control the runtime added (e.g. 2551q evidence.md documents this exact loop); the
   keys are the indexed family patterns below. The only counter is the sheets-attached box.

## Exceptions: tables that do NOT grow officially

- **2551Q Schedule 1**: "Six ATC rows are concrete and bounded. No Add/Delete row builder or unbounded
  family exists." (rules/forms/2551q-v2018/evidence.md, audit.md). Official cap 6 — matches ours.
- **1701 Schedule 1 employers**: fixed at 2 rows both sides. The 1701 runtime inventory records zero
  dynamic families (its only unbounded families are the EXn/SPn attachment clones, README); our model is
  `employers: [Form1701EmployerRow; 2]` (form_1701.rs:401). Not a gap.
- **1604C/1604E schedule rows**: rows are the 12 months / 4+12 remittance rows — calendar-bounded, not
  user-growable; both sides fixed. (Their continuation-table sheets are print layout, not growth.)

## Per-table facts

| form · table | official limit and source | paper rows | official print past paper | our page cap | our upload | our print (main / band-flow) | match |
|---|---|---|---|---|---|---|---|
| 1801 · Part V Schedules 1/1A/2/2A/3/4 | unbounded; addRow_RealProp L5135-5177, addRow_Fam L5195-5226, addRow_Stock L5239-5263, addRow_Other L5276-5286, addRow_Transfer L5299-5309, addRow_Business L5322-5340 (runtime-control-inventory dynamic_families); form_1801.rs:1156 notes "no row limit" | 2 per schedule (frozen-sheet joins rows 1-2 only) | continuation-table state exists in official validation ("for Schedule 1 the state is shared across its continuation table", validations.json) — the sheet flows; exact print output not distilled in repo | 21 rows Sched 1, 7 rows all others (schema-overrides table:4:sched1 rows 1..21, sched1A/2/2A/3/4 rows 1..7) | unbounded vectors; upload writes every row with continuous `_N` keys through extend_layout RowInsertion (form_1801.rs:723-870) — key pattern matches official exactly | main: rows past paper row 2 print NOTHING (1:1 joins); band-flow: continuation sheets (1701A/1801 continuation fixes e3a9a365, bc81181a) | **NO — page cap 21/7 blocks entry; print drops rows 3+ even when filed** |
| 1707A · Schedule 1 / 2 modal rows | unbounded families `{N>=1}` txtS1C1..C6/txtS2C1..C5 (runtime-control-inventory); CSV import exists too (checkCSVDataItems L7277) | main table 4 static rows (txtS1C1..4 on paper); modal rows are not paper rows | not evidenced in repo | page tables rows 1..11 (sched1) / 1..9 (sched2) + 3 base rows = effectively 14/12 (schema-overrides table:3:sched1more rows 1..11, sched2more 1..9) | FORM_1707A_SCHEDULE_ROWS=4 base; `enableEmptyFromMain`/modal machinery — rows beyond cap cannot be entered | main: rows past paper print nothing; band-flow: continuation | **NO — cap 14/12 vs unbounded** |
| 1707 · Schedules 2/3 popup rows | unbounded "6 unbounded popup families"; Save_Pg2Pt4S2PopTable L2846-2926, Save_Pg2Pt4S3PopTable L3046-3109 | popup rows not on paper | not evidenced in repo | rows 1..12 per popup table (schema-overrides table:3:s2more/s3more rows 1..12) | FORM_1707_SCHEDULE_ROWS=4 base + rows up to page cap | main: nothing past paper; band-flow: 1707 continuation proof (bc81181a) | **NO — cap 12** |
| 2200M · Schedule 1 added rows | unbounded "17 unbounded Schedule 1 families"; addRowSched1 L4177-4191, delRowSched1 L4192-4209 | 10 fixed rows static (txtSched1ATC1..10) | not evidenced in repo | rows 11..22 (schema-overrides table:3:added rows 11..22) | FORM_2200M_MAX_ROWS=200 — upload carries to 200, page caps at 22 | main: rows past paper print nothing; band-flow: continuation (2550M/2200M sweep fixes) | **NO — page cap 22** |
| 2200P · Schedule 1 added rows | unbounded "15 unbounded Schedule 1 families" | 27 rows static on paper | not evidenced in repo | rows 28..41 (schema-overrides table:4:added rows 28..41) | FORM_2200P_MAX_ROWS=200 | main: rows 28+ print nothing; band-flow prints rows 28+ with all 14 columns (29258bfe) | **NO — page cap 41** |
| 2200A · Schedule 1 Others | unbounded `{N>=4}` others families | 3 rows (others1..3 on paper, verified in writer-cells) | not evidenced in repo | page grid rows fixed; hard cap 3 | validation REJECTS others.len() > 3 (form_2200a.rs:1263); upload writes exactly others1..3 | main: nothing past 3; band-flow: continuation past 3 (feac5d2f) | **NO — worst kind: cannot even validate/file a 4th row** |
| 1702EX · modal rows (5 tables) | unbounded "11 unbounded popup families"; AddrowChecking/Save modal functions | modal rows not on paper | not evidenced in repo | rows 1..12 per table (schema-overrides table:6:s1i17/s2i4/s3i3/s3i6/s3i8 rows 1..12) | FORM_1702EX_MAX_ROWS=100 | main: nothing past paper; band-flow: continuation | **NO — cap 12 per table** |
| 1702MX · Schedule 6 | unbounded "83 active unbounded runtime families" | base rows on paper; modal rows not | not evidenced in repo | rows 1..12 (schema-overrides table:19:sc6i4 rows 1..12) | draft carries rows; page caps entry | main: nothing past paper; band-flow prints base+modal rows (17c30ee8) | **NO — cap 12** |
| 1700 · Schedule 1 employers | unbounded `{N>=4}` employer families, materialized in frmMain and serialized by saveXML (runtime-control-inventory notes, lines 12633-12757) | 4 employer rows on paper (items 1-4) | folds to OTHERS + popup subtotal officially (rows_1700.rs:1-7 documents the official fold) | 4 page rows + popup rows 4_k up to employer 100 | FORM_1700_MAX_EMPLOYERS=100 (form_1700.rs:47); upload key pattern `{N>=4}` matches official | main: employers past paper print per the fold; band-flow: continuation | **NO — cap 100, but high enough rarely to bind; still a cap the official does not have** |
| 1701 · Schedule 1 employers | NOT growable officially (fixed 2; see exceptions) | 2 | — | 2 | 2 | 2 | MATCH |
| 1601C · Schedule I | unbounded "eight source-unbounded zero-based families" (evidence.md); concrete indices 0-2 static | 3 rows (indices 0-2 on paper) | not evidenced in repo | rows 0..2 (schema-overrides table:3:sched1 rows 0..2) | validation REJECTS > 3 rows: "Schedule I supports at most 3 rows in the verified 1601-C XML contract" (form_1601c.rs:1205-1209); upload writes rows 0..2 only | main: nothing past 3; band-flow: continuation past 3 (31523403) | **NO — worst kind: cannot validate/file row 4+ though the official serializer carries unbounded rows** |
| 2000 · Schedules 1-4 | unbounded; addSchedule1 L3397-3428, addSchedule2 L3535-3559 (+sched3/4); keys `frm2000:schedN:txt…{index}` zero-based continuous | 3 rows per schedule on paper (static txtTaxBase0/1/2) | not evidenced in repo | 3 rows per schedule (schema grid items) — no table: override; rows beyond 3 need the growable path | upload writes continuous index keys for all draft rows up to FORM_2000_MAX_ROWS=50 (form_2000.rs:962-1010) — same key pattern as official, NO duplicate keys on main | main: rows past 3 print nothing; band-flow: continuation | **NO — page 3 rows; upload caps at 50 vs unbounded** |
| 2551Q · Schedule 1 | bounded officially at 6 — no row builder exists (evidence.md) | 6 | — | 6 (schema-overrides table:3:sched1 rows 1..6) | 6 | 6 rows print | MATCH — nothing to fix |
| 1600WP · payee schedule | growth not evidenced either way in the rule package; the official print is an alphabetical payee-list ATTACHMENT (evidence.md) | attachment prints six rows per sheet (schema-overrides note); page table 10 rows | attachment sheets | rows 1..10 (grid "Seq. No. 1 to 10") | FORM_1600WP_SCHEDULE_ROWS=10 | attachment print; band-flow: seq numbering support (79a2e85f) | unclear officially — needs the one missing evidence; our side fixed 10 |
| 1604C · Part II continuation | rows are the 12 months — calendar-bounded, not user-growable | 12 | — | 12 | 12 | 12 | MATCH |
| 1604E · Schedules 1/2 | same — 4/12 remittance rows, calendar-bounded | 4/12 | — | 4/12 | 4/12 | 4/12 | MATCH |

Other growable tables found (same shape, not in the brief's list): 1601EQ ATC rows 7..30
(schema-overrides table:3:atcmore), 1701A Part IV modal rows 1..12 ×2, 1702RT five modal tables rows
1..12, 1800 Schedules A/B rows 0..14, 2000OT sched1a 0..14 / sched1b 0..12, 2200S others rows 0..11,
2550M eight tables (rows up to 36/9), 2550Q six tables rows 0..11, 2552 three schedules rows 1..9,
1602Q schedules. All share the pattern: official unbounded, our page capped at the override row range.

## The duplicate-keys question (parked branch try/forms-expandable-rows 0c2af498)

On **main**, duplicate keys cannot arise:
- 1601C: validation rejects row 4+ outright — the upload never carries them.
- 2200A: validation rejects a 4th "others" row outright — never carried.
- 2000: the upload writes zero-based continuous indices (`txtTaxBase{index}` for index 0..n) which is
  exactly the official key pattern — row 4 is `txtTaxBase3`, no collision.
The duplicate keys the parked branch showed were introduced by that branch's expansion mechanism, not
inherited from main. What main does instead is worse in a different way: rows past the cap are either
rejected at validation (1601C, 2200A) or impossible to enter on the page (every other table).

## Tables where a filer cannot enter or file what the official program accepts — worst first

1. **1601C Schedule I** — validation hard error past 3 rows; official unbounded. Cannot file at all.
2. **2200A Schedule 1 Others** — validation hard error past 3 rows; official unbounded. Cannot file at all.
3. **1801 Part V six schedules** — can enter at most 21/7 rows; even below that, rows 3+ vanish from
   print on main (paper holds 2). Estate with 8+ real properties blocked at entry; 3+ properties print-blank.
4. **2000 Schedules 1-4** — 3 rows enterable per schedule vs unbounded official; upload capped at 50.
5. **1707A Schedules 1/2** — cap 14/12 rows vs unbounded official (a stock trader with a year of
   transactions exceeds this trivially; the official CSV import exists for exactly this use).
6. **1707 Schedules 2/3** — cap 12 vs unbounded.
7. **2200P Schedule 1** — cap 41 vs unbounded (upload ceiling 200).
8. **2200M Schedule 1** — cap 22 vs unbounded (upload ceiling 200).
9. **1702EX five modal tables** — cap 12 each vs unbounded.
10. **1702MX Schedule 6 and other modal tables** — cap 12 vs unbounded.
11. **1700 employers** — cap 100 vs unbounded; practically rare but a real cap.
12. Every other table in the "other growable tables" list (1701A, 1702RT, 1800, 2000OT, 2200S, 2550M,
    2550Q, 2552, 1601EQ, 1602Q) — same pattern, smaller caps.
13. 1600WP payees — our side fixed at 10; the official side needs one more evidence pass (the package
    neither proves nor excludes growth; the print is already an attachment).

## Honest evidence gaps (for the design phase)

- "What the official program prints past the paper" is NOT distilled in the rule packages for any form.
  What exists: continuation-table state sharing in official validation (1801, 1604C), the 1600WP
  payee-list attachment, the sheets-attached counters. A pass over the official print functions
  themselves (they are in the HTA sources the inventories hash, not in this repo) is needed before
  print design; the inventories give the function line ranges to read.
- 1707A/1707/1702EX paper row counts for the base tables are partially evidenced (static control ids);
  the modal rows never appear on paper by construction.
