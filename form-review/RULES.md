# How printed fields behave — the owner's picks (2026-11-25)

The owner chose these from a page of drawn and real options. They replace the earlier "white band" approach. Every rule is shared: it lives in the print engine and applies to all 43 forms. No rule is implemented per form, except the one marked "override".

The owner's stated aim: the app is on its way to being the official BIR eBIRForms, so the printed form may be changed where that makes it better. The official look is kept wherever the value fits.

## 1. Character boxes

A character-box field (boxes with a hairline rising from the bottom of each; on number fields also thicker group marks) prints as the official form draws it **only when the value fits the boxes**. For a field with two lines of boxes, the two lines are counted together.

When the value has even one character more than the boxes hold, **that field** becomes a plain rectangle: the outer border stays, every separator inside it goes (hairlines, thick marks, their caps), and the value prints as plain text.

Only the field that overflows changes. A neighbour that fits keeps its boxes. A TIN, a date, a ZIP code or an RDO code that fits is never touched.

## 2. Amounts

An amount that does not fit its digit boxes (too many digits, or a negative that needs brackets) becomes a plain rectangle too: the hairlines, the thick thousand marks, the preprinted decimal point with its grey cell and the centavo boxes all go. The amount prints once, right-aligned, with thousands commas and two decimals (whole pesos where the form says no centavos).

An amount that fits keeps its boxes, digits in the slots, as today.

## 3. Long text

In this order: a second line when the field is tall enough; then smaller letters; then cut with "…" and print the full value on an additional sheet ("Entries continued from the form"). Text is never smaller than **7.5 pt**.

## 4. Alignment

Amounts, counts and other numbers: right. Text: left. Short codes and rates: centred. In plain cells and in rectangles alike.

## 5. Sections that grow: the table grows on the same page

When a schedule has more rows than the paper prints, the extra rows are drawn **directly under the last printed row, in the table's own row design**, and everything below slides down.

- A page is a stack of horizontal bands (header, section title, table head, row, total, the next part…). Growth inserts row bands and moves the bands below.
- The page uses its empty space first (white space at the bottom; an empty stamp or validation box may shrink to a stated minimum).
- When the content no longer fits the paper, the page breaks **between bands**, never through a row, and never between a table's head and its first row or between its last row and its total. The bands that do not fit start a new page that begins with the form's own page header (form number, title, TIN, taxpayer's name) marked as a continuation of that page.
- Totals stay with their table and are the true totals of all rows.
- A form whose pages have no band data yet keeps the built behaviour (continuation page in the official grid, or a plain additional sheet). Nothing is ever dropped.

## 6. Override, for a few named fields only

"Other Tax Credits/Payments (specify)" and fields of the same shape (a small specify box in the label column, sharing its row with an amount): 2551Q item 17, 2550Q 19, 1700 57, 1701A 63, 1701Q 61, 1701MS 7 and 9, 1702EX, 1702RT, 1702Q, 1601C 20, 1603Q 16. Check each on its sheet before switching it on.

When the specify text does not fit: it moves to a full-width rectangle on a new line under the label. The amount box keeps its normal size and stays level with the item's number. The new line is a band inserted as in rule 5.

## 7. Also chosen

- The form page tells the user while typing: how many characters fit the boxes, and that a longer value will print as a plain line.
- A footer on every printed page: form number, TIN, period, "Page 2 of 5".
- An unfiled print is marked "DRAFT – not filed"; after filing the footer carries the filing date and reference.
- Unused amount boxes stay blank; 0.00 prints only where the BIR's program prints it.
- Rates print as a rate ("5 %", centred).
- Places print as names.
- Negative amounts as the form says: brackets where the caption says "(Overpayment)" or "(Overremittance)", a minus sign elsewhere; never dropped.
- The saved PDF and the paper print match the preview.

Not chosen: a warning list before printing.

## For every change

- Print only. The upload never changes: `cargo test -p bir-core --test sample_payloads_test` before and after.
- Prove with a real saved return entered through the form page, the same return for before and after, on the preview and on the saved PDF.
- Pictures are committed to the branch; chat attachments do not reach the orchestrator.
- Before every push: `cargo fmt --all -- --check`, `cargo clippy --locked --workspace -- -D warnings`, bir-core lib, sample payloads, bir-print, bir-app-api lib.
