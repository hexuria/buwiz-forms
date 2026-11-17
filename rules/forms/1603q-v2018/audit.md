# Audit

Coverage: 76 fields, 31 rules, nine calculations, seven workflow transitions, and 21 negative fixtures.

Confirmed defects and hazards:

1. `validateYear` compares four-digit years with `Date.getYear()` and resets valid years to offsets such as 126.
2. A pre-2018 blur can show two alerts and perform two resets.
3. TIN validation is nonblank-only.
4. Telephone validation is nonblank-only and its exact message contains a double space.
5. Schedule 1 total always becomes `0.00` because row 2’s ID string is parsed as a number.
6. The broken total keeps Item 14 at zero, making the normal “taxes withheld = Yes” Validate path fail.
7. Downstream Items 18 and 23 inherit the false zero.
8. Save persists otherwise invalid drafts after only four checks.
9. Final Copy is coupled to submission/network handling.

The defects are represented as official behavior with separate corrected recommendations. No write or submission path was executed.
