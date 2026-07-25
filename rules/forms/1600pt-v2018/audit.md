# Audit

Coverage: 262 fields, 44 rules, nine calculations, 31 ATC records, seven workflow transitions, five attachment classes, and 24 negative fixtures.

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
10. Six shared `PG` catalog records require category-specific slot numbering.
11. Installed Help1600 is the wrong form revision.
12. Final Copy is coupled to submission and network handling.
13. The generated submission email body incorrectly labels 1600-PT as `Tax Type: Income Tax` (`sendEmail`, line 6671).

Active and obsolete rules are explicitly distinguished. No UI write, Save, Final Copy, or submission path was executed.

Verification on 2026-07-23:

- `rtk powershell -NoProfile -ExecutionPolicy Bypass -File rules/validate.ps1 -RequireJsonSchema`
- Result: pass; 7 forms, 1,727 fields, 284 validations, 90 calculations, 153 negative fixtures, 81 JSON files, and 36 schema documents.
- Direct hash/size checks passed for the HTA, official PDF, official guide, runtime ATC catalog, and dummy representative save.
- Manifest-to-artifact counts match: 262 fields, 44 validation rules, 9 calculations, and 31 ATC records.
