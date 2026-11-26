# Form 2550M — Print Verification (Part B)

## Lead vs. Reproduction (`origin/main` `3aceafe8` and PR #315 `6f5ddf72`)

The round-3 eyes lead (`12997d2d:form-review/eyes/round3/2550M/mismatches.tsv`) reported Item 13A (`1,493.39` vs `1,085.15`) and Item 15 (`1,527.41` vs `1,119.17`) as print join faults (`p1c28` / `p1c31`).

- **Saved draft JSON** (`data-excerpt.txt`): holds `sales_to_government = 1085.15`, `zero_rated_sales = 1102.16`, `exempt_sales = 1119.17`, `total_sales = 3306.48`.
- **Printed at `origin/main` (`3aceafe8`)**: `p1c28 = "1,085.15"`, `p1c31 = "1,119.17"` — exact match to the saved draft JSON.
- **Printed at PR #315 (`6f5ddf72`)**: `p1c28 = "1,085.15"`, `p1c31 = "1,119.17"` (`git diff origin/main..6f5ddf72 -- crates/bir-core/src/forms/form_2550m.rs html-frozen/2550m-2007/ crates/bir-print/` is empty).
- **Origin of `1,493.39` / `1,527.41`**: `1493.39 - 1085.15 = 408.24 = 24 * 17.01` and `1527.41 - 1119.17 = 408.24 = 24 * 17.01`. In `crates/bir-core/data/agent-forms/2550m-v2007.json` (on both commits), `txtTaxableSaleSched4` and `txtTotSaleSched5` are marked `optional` and bound to `/sales_to_government` and `/exempt_sales` alongside `frm2550m:txtTax13A` and `frm2550m:txtTax15`. Filling every editable box sequentially with `+17.01` caused those Schedule 4/5 aliases to overwrite the in-memory `form.fields` read.

No 2550M print change was made on `feat/forms-print-missing-values`.
