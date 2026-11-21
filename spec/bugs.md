# B1–B21: the bugs Q11 says to fix

From the probe draft. Line numbers are stale; the code wins. Where this disagrees with spec/PLAN.md, PLAN.md wins.

## B1 (critical): Form 0605 Item 4 and Item 7 date parts are refused on a blank draft, so typed digits vanish and the calendar pick does nothing

**Fix (as planned):** Rust: add 0605 items 4 and 7 to ONE_DATES (rows_as_needed.rs 106-131; the array grows from three entries to five; replace the ONE_DATES[2] uses at 1262 and 1269 with a lookup by form code; date_part 133-134, mark_one_date 142-143 and values 1072-1073 scan all entries). Give the 0605 date parts their own Cell::DatePart writer (fill 575-580) instead of the ordinary set_path: a blank part clears that part (zero), a set part is written into the Option (materialized), and the draft is normalised to None when all three parts are zero. refuse_dates (215-250) keeps refusing a complete but impossible batch. A partial one-date batch never arrives (editOneDate stages complete dates or a blank). Page: the one-date box (P2a) carries the date.

**Test (as planned):** Rust: a blank 0605 draft given 08, 17, 1988 yields due_date Some(1988-08-17) and the official map prints 08 / 17 / 1988; clearing all three yields None; a per-part write into a blank draft materializes the Option and validation reports the incomplete date (not a refusal). Extend as_needed.rs ONE_DATE_BOXES (2085-2112) and the pr9 golden (2124-2150) with 0605 entries. NSH: on a new 0605 at 640 pt, typing 08171988 into Item 4 and blurring shows 'August 17, 1988' with no message; reopening shows the same. pickDate on a blank 0605 fills all three parts (pickDate.test.ts has no 0605 case).

## B2 (high): Form 0605 Item 6 ATC, Item 7 return period and Item 8 tax type are refused on a blank draft

**Fix (as planned):** Make Items 6 and 8 catalog picks (a search list over the ATC and tax-type catalogs), not free text. Write them through the catalog path (fill_as_user over the catalog, catalogs.rs 273) so the indexed code and its xml_index are set together. Item 7 follows B1.

**Test (as planned):** Rust: a catalog ATC pick on a blank 0605 sets atc with its xml_index and prints; free text is refused with a plain sentence ('Pick an ATC from the list.'). NSH: picking an ATC on a new 0605 at 390 and 640 pt shows the pick and no 'draft has no' text anywhere.

## B3 (high): Refusals are joined onto one row, so three date boxes share one message line

**Fix (as planned):** Bind each refusal to its own box (messageKeys per key; FieldControl shows its own line, P5a). Let the item-level sentence attach to a date group only when no box has its own message. refuse_dates returns one sentence, keyed to the month box. Join messages with sentences, never ' · '.

**Test (as planned):** NSH: on a blank 0605, typing '13' into the month shows one message under the month box ('Item 4: the month is 1 to 12.') and none under the day and year boxes; the row text has no 'Invalid date entry'. Rust: refuse_dates returns one key for an impossible date.

## B4 (high): Refusal text reaches the filer as host words and JSON paths

**Fix (as planned):** One table maps every refusal kind to a plain sentence with the item number (P6a), used for rejected fills and host errors alike. Raw text goes to the log only.

**Test (as planned):** NSH table test over the refusal kinds (path, whole number, amount, date part, select): no shown message contains a backtick, a '/', 'draft has no', or an 'frm' key prefix. Rust: the set_path and Whole-arm refusals use the plain form.

## B5 (high): The line of business is asked by the box, the row message and a card at once

**Fix (as planned):** One surface per fact (P1a): asks() emits a card only where place() finds no box the fact fills; the blocking sentence is the row message; the box is the input.

**Test (as planned):** Rust fact_asks: a 0605 with a blank line of business gives one blocking check and no card for line_of_business; a fact with no box keeps its card. Update queue_blocks_a_1701q_without_a_birth_date (fact_asks.rs 407-452), which pins message plus card. NSH: fp-ask-line_of_business is absent; the row shows one sentence.

## B6 (high): A card Save overwrites the typed box and re-syncs other unedited profile boxes

**Fix (as planned):** After P1a, boxed facts have no card, so this path is not used for them. For facts that keep a card: take_facts writes a draft field only when it is blank or already equal to the answer; boxes the filer edited on this return are left alone (edited_profile_paths already exists).

**Test (as planned):** Rust: a card save for EOPT on a return whose line-of-business box holds typed text keeps that text. NSH: type 'Retail' into a box, then save a card for another fact: the box still reads 'Retail'.

## B7 (high): Typing the value never answers the profile and never retires the card

**Fix (as planned):** screen_form_validate and screen_form_fill return asks along with fact errors; after an edit to a fact-filling box the page replaces s.asks. The card disappears when the box holds the value (P1a). Queue keeps its one non-blocking warning for facts the profile still lacks.

**Test (as planned):** Rust: a fill of line of business into a 0605 with a card returns no line_of_business ask. NSH: typing into the box removes the card; the switch appears; Queue warns once and does not block when the switch is off.

## B8 (high): 0619E and 0619F cannot clear the withholding-category block with a radio

**Fix (as planned):** A radio pick is the answer: a pick on the category radio (0619E item 12, 0619F item 11) writes is_government_withholding_entity through set_year_facts (the switch rule of P1a applies). The block reads the fact or a radio the filer chose, never the default.

**Test (as planned):** Rust: 0619E with an unset profile and a Private pick gives no withholding block; with no pick, one block. NSH: ticking Government office on 0619E stops the category block at Queue.

## B9 (high): Save-to-profile routes exist for seven text fields only

**Fix (as planned):** Add a route per fact (update_taxpayer for birth date; set_year_facts for EOPT, income type, start date, withholding and ATC) and an option-to-value map for radio and checkbox boxes (the choices in P4 keep the stored value).

**Test (as planned):** Rust round trip for each fact: set the box, save with the switch on, and the year's facts hold the value; a refused value is refused with a plain sentence. NSH: the switch appears on each fact box.

## B10 (high): Boxes and their messages sit in a closed 'Additional details' fold

**Fix (as planned):** Open the fold when a message or card keys to a box inside it (messageKeys for the row), or draw the keyed box outside the fold.

**Test (as planned):** NSH: for each of the twelve forms, with a required blank in the fold, the row opens the fold and the keyed box is drawn (its fp-<key> exists). 2200S: 'Additional details' is open, the line-of-business box is visible, and the message names the box.

## B11 (medium): 'Save to profile too' is never offered for a blank fact box, and it starts off for a filled one

**Fix (as planned):** Offer the switch on blank fact-bound boxes (P1a): default on when the profile is blank, off when the value came from the profile.

**Test (as planned):** Rust: profile_save lists a blank line-of-business box. NSH: a blank line-of-business box shows 'Also save to tax profile' on; a profile-filled box shows it off.

## B12 (medium): The 1701 income-type card does not set Item 6

**Fix (as planned):** Ask the 1701 fact in Item 6's own terms (single proprietor, professional, compensation earner), or ask the ATC. Save what the form derives, and keep the profile's income type as its own question.

**Test (as planned):** Rust: a 1701 with a SelfEmployed answer sets taxpayer_type, or the card asks for the ATC. NSH: Item 6 is ticked after the card save.

## B13 (medium): Row titles and overrides put the wrong label over boxes

**Fix (as planned):** Correct the overrides: 1601C item 8 needs a caption that names both boxes, or a split; give 1701Q's line of business its own caption; keep hidden credential keys out of visible rows; fix the stale override evidence.

**Test (as planned):** A schema test over all 43 forms: a row captioned 'Line of Business' holds only line-of-business boxes; any row holding a withholding-agent name box is not captioned for line of business.

## B14 (medium): DateField does not swap to MM/DD/YYYY on focus, and blur uses the short month

**Fix (as planned):** onFocus sets the text to formatMDY when it parses. While focused, reformat to MM/DD/YYYY as soon as eight digits parse (caret to the end, InputRef selection). formatLongDate uses MONTHS_LONG. Cards use DateField. Update fields.test.tsx 64-73, which expects 'Dec 10, 2020'.

**Test (as planned):** Kit: '12102020' focused shows '12/10/2020'; blurred shows 'December 10, 2020'; '12-10-2020' and 'Dec 10 2020' normalise the same way; a card date saves and shows the long form.

## B15 (medium): A cleared date part is stored and shown as '00'

**Fix (as planned):** Exempt date parts from the zero rule: a blank date part stays blank and gets its required sentence ('Item 2: enter the day.').

**Test (as planned):** NSH: clearing 0619E txtDueDay leaves the box empty, shows 'Item 2: enter the day.', and the draft holds no zero for that part.

## B16 (medium): A gated box keeps 0 and says nothing (0619E Item 14; 2551Q Item 12 select)

**Fix (as planned):** Refuse a write to a shut box with its reason ('Item 14 is used when Item 4 is Yes.') on the page and at the host, and show the box faded (P9a).

**Test (as planned):** Session test: a write to a gated box is refused with the parent's name; the box shows the reason; the draft keeps no amount; the print omits it.

## B17 (medium): An unlisted select code is accepted on one form and refused on another

**Fix (as planned):** Refuse a code outside the box's option list in choose() and in the host's choice arm, with one sentence: 'Pick one of the listed choices.'

**Test (as planned):** A table test over every select: choose('ZZ') is refused the same way on 1701Q and 1702Q.

## B18 (medium): The test suite let these bugs ship

**Fix (as planned):** Extend the every-date test to split and one-date boxes and to blank-seeded drafts. Make the probe fail on any refused write for a blank seed. Remove the silent catches and the early returns (real-window cases move to the shots lane). Remove the 'if (ask)' guard.

**Test (as planned):** Land these tests red first on the current tree (they fail for 0605), then fix B1. The NSH and Rust tests named in B1 to B17 are the regression set.

## B19 (low): The save line reads 'Saved' while a refused edit stands

**Fix (as planned):** Trace the save-line state from rejected fills; keep it at 'Unsaved changes' while any box is refused.

**Test (as planned):** NSH: type an invalid date part; the save line never reads 'Saved' until the refusal is cleared.

## B20 (low): The official map upper-cases line of business, name and address, while the box keeps the typed case

**Fix (as planned):** Choose one rule: store and show as typed, and upper-case at print only; or show upper case everywhere. The box and the reopened box then agree.

**Test (as planned):** NSH: type 'Retail', reopen: the box reads the same as before reopening. Rust: the official map for line of business is the printed upper-case text.

## B21 (low): The header chip counts hidden keys

**Fix (as planned):** Count the drawn fields (session.fieldCount).

**Test (as planned):** NSH: the chip's value equals the number of drawn boxes for 0605.
