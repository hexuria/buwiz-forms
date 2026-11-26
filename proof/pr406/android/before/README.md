# BEFORE — main 44ccfd80

## Step 1 — New tax profile → RDO — the broken baseline
- Sheet opens with rows compressed into the top strip (row 1 under the status bar), NO visible search field, keyboard up. Filtering works (type "05" → 051/052/055/056/057) but the search box is invisible.
- Scrolling does NOT fix it: `b-step1-scrolled-end.png` — still compressed, no title/X/search after scrolling to the list end; rows 064-066 end at y1819-2120, under/behind the keyboard zone. Last rows not properly reachable above the keyboard.
- X close: no visible X exists in the broken layout; tapping where it should be closed the sheet.
- "zzz" DOES show "Nothing matches. Try a code or other words." (in the tree; the layout is broken but the text exists).
- Picking works: 048 — West Makati picked and shown on the form (`b-step1-picked.png`). Rows are two-line (code over city), hyphen-less.
- Video: `b-step1b.mp4` (76 s).

## Step 2 — ZIP
- Keyboard at open: QWERTY visible in shots (spec's "number pad" question: the before build already shows a text keyboard in onboarding context).
- Type "12" filters (1120-1124). Scroll end rows visible but the list truncated: "Showing 60 of 141. Type more words to narrow it." (`b-step2-endstate.png`).
- Pick: 1612 - Green Park shown on form. Video `b-step2.mp4`.

## Step 3 — form RDO picker (2551Q)
- NOT MEASURED on device: the 2551Q's RDO is a profile box pre-filled from the tax profile; no separate picker row was found in return details before time ran out. The control is the same shared Pick component as steps 1-2 (nsh/screens/form/controls/Pick.tsx).

## Step 4 — birth date
- No date sheet at all: a raw text input with placeholder YYYY-MM-DD (`b-step4-after-tap.png`).
- Typing 05/17/1990 then save is REJECTED: "Type the date as YYYY-MM-DD, like 1990-05-17." — ISO-only parsing.
- ISO 1990-05-17 is accepted; persists as "May 17, 1990" (`b-step4-persist-check.png`). Video `b-step4b.mp4`.
- 02/30 and next-year refusal: not separately measured — same raw ISO validation path (a US-format 02/30/1990 never even parses).
