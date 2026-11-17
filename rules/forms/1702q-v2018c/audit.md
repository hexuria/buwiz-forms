# Audit

## Coverage

- 113 typed DOM occurrences match 113 serializable elements in the official HTA and representative save.
- 112 unique serialized keys are accounted for; the one duplicate key is occurrence-qualified.
- 39 validation/state rules cover four input handlers, nine change/state handlers, 22 ordered Validate checks, and four Save preflight checks.
- 25 calculations cover Schedules 1-4 and Part II Items 14-25.
- Workflow includes every observed final flag (`1`, `2`, and `3`) and does not treat successful local validation as filing.

## Confirmed official defects

1. Validate silently returns when no quarter is selected, making its later Item 3 message unreachable.
2. `validateYear` also silently returns without a quarter and silently handles a future fiscal-quarter path.
3. `validateFiscalMonth` compares a four-digit year with `Date.getYear()`.
4. July/August 2020 rate handling has an operator-precedence bug that affects every August.
5. TIN validation is length-only and accepts branch lengths three through five without checksum validation.
6. Save and Validate disagree on whether RDO `000` is invalid.
7. Schedule 4 pairing checks compare the exact string `0.00` and permit equivalent-zero bypasses.
8. Schedule 1 Item 11 clamps the B column but not the A column for a nonpositive base.
9. Schedule 1 Item 13B can be negative or zero for identical inputs depending on event ordering.
10. Part II Item 25 discards a negative Item 20 when Item 24 is positive.

These are recorded as official behavior and separately accompanied by recommended behavior; they are not silently corrected in the extracted contract.

## Safety

All fixtures use dummy data. The existing save was read only. No Save, Final Copy, submission, email, or external transmission path was invoked. No renderer, release, migration, or capability artifact was changed.
