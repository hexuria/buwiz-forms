# Growable tables — the facts (research, report only)

Job from forms-validation 2026-11-26: for every table that grows on any form, the official
limit, the paper's rows, what the official program prints past them, our page cap, our upload
carriage, our print behavior. No app code changed.

Method notes and limits:

- The official program source is NOT in this repository. What is: per-form rule packages under
  rules/forms/<form>-v<rev>/ distilled from the official HTA runtime (validation-function-inventory
  with function names, line ranges in the official source, body hashes and exact alert texts;
  runtime-control-inventory with the live static controls and the dynamic indexed families,
  including the source line of the runtime concatenation or the add-row handler). Line numbers
  quoted as L<x> are lines in the official decompiled HTA source as recorded in those inventories.
- official-families-full.txt: every dynamic indexed family per form from the runtime control
  inventories, grouped, with source lines. `{N>=k}` means rows 1..k-1 are static pre-allocated
  controls and the runtime-generated family starts at index k.
- official-validation-functions-*.txt: the inventoried validation functions per form.

Per-table findings build up in tables.md; the final report table is REPORT.md.
