# comb-referee campaign scope (measured, pre-Z2)

Source: standalone `comb_referee.py` run against the r63 fresh 53-form audit,
after the three-way partition fix (`98816f72`). Payload
`7a104a4eecedf55eda29ac34e3fcad2c2069c33a087f5f0b8505e10e35b8dc19`.

## Totals

| metric | value |
| --- | --- |
| forms_expected / forms_measured | 53 / 53 |
| audit_evidence_complete_forms | 53 |
| combs_expected / combs_found | 4587 / 4587 |
| comparisons.agree | 4522 |
| comparisons.unevaluable | 65 |
| forms_ok | 24 |
| forms_unevaluable | 29 |
| forms_disagreement / forms_error | 0 / 0 |
| subjects_active / resolved / unresolved | 4557 / 4472 / 85 |
| subjects_retained_unresolved | 30 |
| ledger_blocking | 116 |
| referee_attestation_complete | false |
| status | unevaluable |

Baseline for comparison (pre-Z1): agree 4514, unevaluable 73, forms_ok 23.
The Z1 reviewed-topology registry is worth **+8 agreeing comparisons and +1
clean form**.

## The 65 unevaluable comparisons, by cause

| class | n | reason |
| --- | --- | --- |
| A | 30 | `ledger subject has no active topology for adjudication` |
| B | 3 | `audit published this subject as an offender with no printed topology` |
| C | 32 | `referee: the source does not corroborate the comb writing band` (+3 topology-proof variants) |

**Class A (30).** Same count as `subjects_retained_unresolved` (30). The
report publishes no retained-subject list, so the two sets are NOT proven
identical here — only their counts match. Prove or refute before treating them
as one population.

**Class B (3).** 2200a-2020 p1c111, 2200c-2018 p1c107, 2200p-2020 p1c110 — the
F229 trio. Z2's outer-rail trim is expected to close these.

**Class C (32).** The referee's own writing-band corroboration. Sub-reasons,
verbatim prefixes:

- `the source walls inset this cell` — the large majority
- `the source top wall is not one w…` (2)
- `the source bottom wall is not on…` (2)
- `the layout declares no top borde…` (2)
- `the layout declares no bottom bo…` (1)
- `chosen source topology lacks a clean single-frame subject proof` (1)
- `one or more source slabs have ambiguous topology` (1)
- `source topology does not occupy a strict majority of the full comb band` (1)

## By form (22 forms carry all 65)

    7  1701ms-2024          3  1604f-2018           1  1606-2018
    6  1800-2018            3  2200a-2020           1  1706-2018
    6  2316-2021            3  2200p-2020           1  1707-2021
    5  2551m-2002           2  0605-1999            1  2200an-2018
    4  1702mx-2018c-attach  2  1600wp-2010          1  2200s-2018
    4  2200c-2018           2  1707a-2021
    4  2550m-2007           2  2000-ot-2018
    3  1604cf-2008          2  2200t-2022
                            2  2553-1999

## What full comb-referee PASS requires

gate.py:7067-7156 + elevation :6498-6757 — forms_ok 53, `comparisons.agree`
== 4587, `unevaluable` == 0, `subjects_retained_unresolved` == 0, and per-form
elevation with every cell four-way agree. Classes A and C are the campaign;
B closes with Z2.
