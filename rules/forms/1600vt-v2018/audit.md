# Audit

Coverage: 174 fields, 44 rules, nine calculations, 14 ATC records, seven workflow transitions, five attachment classes, and 24 negative fixtures.

Confirmed defects and hazards include:

1. Full Validate never checks the main Page 1 TIN.
2. Save also omits the main TIN.
3. Schedule 1 TIN validation checks only that a combined string is longer than 11.
4. Email blur error cites Item 10 instead of Item 11 and uses a narrow, unanchored regex.
5. Full Validate requires email only for nonblankness and does not call the syntax helper.
6. Copied CTC/date rules cite nonexistent Item 24 certificate semantics.
7. Several source comments and helpers retain 2200C/1601E identifiers and are obsolete/dead for this form.
8. Item 27 discards a negative Item 22 whenever penalties are positive.
9. Save persists otherwise invalid drafts after only six checks.
10. Ten shared `PG` catalog records require category-specific slot numbering.
11. Installed Help1600 is the wrong form revision.
12. Final Copy is coupled to submission and network handling.

Active and obsolete rules are explicitly distinguished. No UI write, Save, Final Copy, or submission path was executed.
