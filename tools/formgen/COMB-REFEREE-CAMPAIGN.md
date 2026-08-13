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

**Class A (30). IDENTITY NOW PROVEN, 2026-08-14.** Measured on the gate-r65
referee report by set equality, not by count: the 30 cells whose comparison
reason is `ledger subject has no active topology` are EXACTLY the 30 cells the
gate counts as `emission_layout_mismatches` (`emitted: None`,
`emitted_indexes_valid: false`), and `subjects_retained_unresolved` is 30. One
population, three names. The earlier note that only their counts matched is
superseded.

They span 17 forms: 2550M 4; 2200C, 2551M 3; 1600WP, 2000-OT, 2200A, 2200P,
2200T, 2553 2; 0605, 1604CF, 1604F, 1606, 1707A, 1800, 2200AN, 2200S 1.

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


## Gate r65: the check is EVALUABLE for the first time, and FAILS

Until r65 `comb-referee` reported UNEVALUABLE, so its own arithmetic never ran.
With the Z1 partition defects fixed it evaluates, and reports **35** — which is
34 distinct cells, one of them counted under two stats:

| cells | stat | origin |
| --- | --- | --- |
| 30 | `emission_layout_mismatches` | class A above. PRE-EXISTING, newly visible. |
| 4 | `referee_layout_mismatches` | Z2's combs. Lattice and audit AGREE; the referee dissents. |
| 1 | `referee_layout_mismatches` | 2200C `p1c6` — pre-existing, and also one of the 30. |

The four Z2 cells, all `status: stop` ("lattice and audit agree against the
independent referee"):

| form | cell | ours | referee |
| --- | --- | --- | --- |
| 1801-2018 | p1c13 | 3 | 4 |
| 2200a-2020 | p1c111 | 28 | 29 |
| 2200c-2018 | p1c107 | 28 | 29 |
| 2200p-2020 | p1c110 | 28 | 29 |

The referee recognises only a full-height WALL as a comb's outer edge and has no
concept of a rail bounded by a guide-tick run, so on these four it keeps
counting the caption region. Deferred here by owner decision 2026-08-13 rather
than fixed alongside the producer change it would vindicate: a judge is not
taught a new rule in the round that needs it to agree.

**Teaching it must be an independent measurement.** The referee parses Poppler's
vectors itself and shares no code with `lattice.py`; any tick-run rail it learns
must be derived from its own evidence and must remain able to dissent. Copying
`outer_paper_unguided`'s conclusion across would make the two implementations
one, and this check exists precisely because they are two.


## Campaign packages (R-series) — measured 2026-08-14, plan of record

User instruction 2026-08-14: "go start with it" — the campaign is now IN
scope. All numbers below are from the r65 report (`build/comb-referee.json`,
determinism `b9d71850a8c6`), enumerated per cell, not inferred.

The 66 non-agree comparisons (agree 4,521 + stop 4 + unevaluable 62 = 4,587):

| n | class | mechanism |
| --- | --- | --- |
| 30 | retained subjects | `comparison()` has NO adjudication path for a non-active ledger state — unconditional unevaluable |
| 22 | walls-inset | source walls inset the cell; band uncorroborated |
| 4 | wall-not-one-weight | 1702MX-attachment: top/bottom wall mixed weights across compartments |
| 3 | no-border-tone | 1707 p1c217, 1707A p1c207, 2551M p1c103: layout declares no border tone where the source measures a wall |
| 3 | topology-proof | single-frame proof / ambiguous slabs / strict majority — one cell each |
| 4 | stop (F232) | referee's wall-only outer-edge model dissents on Z2's four combs |

The 30 retained subjects, by suppression reason:

| n | reason codes | corroborable today? |
| --- | --- | --- |
| 18 | `emission-suppressed-no-rectangular-owner` + `painted-edge-partition` | NO — no source re-derivation exists |
| 11 | `emission-suppressed-caption-block-not-character-cells` | YES — glyph census 28–87 per compartment, already runs |
| 1 | `emission-suppressed-no-final-visible-band` | NO |

Every retained subject publishes exactly two permitted transitions:
`active_composite` or `retired_proven_false`, with
`requires_independent_evidence: true` and `blocks_gate: true`. The design
comments are explicit: the transition belongs to "whoever reviews it";
nothing in lattice.py may retire its own subject, and `validate_comb_ledger`
today refuses a ledger arriving in the retired state (proven by its own
mutation) — a certificate path does not exist yet.

### Packages, in order

- **R1 — tick-bounded outer rails in the referee (F232).** Derived from the
  referee's OWN Poppler vectors with its own clauses; it must remain able to
  dissent. Porting `outer_paper_unguided`'s conclusion across is forbidden —
  it would collapse two implementations into one. Moves stop 4 → 0 only if
  the referee's own measurement lands there.
- **R2a — source re-derivations for the two uncovered suppression criteria.**
  `no-rectangular-owner`/`painted-edge-partition`: re-derive from Poppler that
  painted edges partition the legacy rectangle (the ledger already publishes
  `mapped_partition_subject_keys` to check against). `no-final-visible-band`:
  re-derive that no final-visible band exists there. Modeled on the
  caption-block re-derivation, which is the template: reason code selects the
  question, Poppler answers it.
- **R2b — reviewed retirement.** Registry + certificate validation so a
  corroborated subject can take `retired_proven_false` ONLY with a named
  reviewer; evidence panels generated for the user's review; entries land
  after the user confirms. `comparison()` gains the adjudication verdict for
  a corroborated, review-retired subject. retained 30 → 0.
- **R3 — writing-band corroborations (29).** Per-cell measurement first;
  producer vs referee decided per class from the ink; never weaken either.
- **R4 — the three topology-proof cells.** Individual deep-dives.
- **R5 — elevation + final.** forms_ok 53, agree 4,587, unevaluable 0,
  stop 0, retained 0, four-way agree on every cell, attestation complete and
  enforceable. Gate 13/13.

House rules carried into every package: comb_referee.py is single-writer
(operator); audit.py stays locked; measure on the tree written and verify 53
forms; every new check gets a load-bearing mutation (proven by neutering);
never weaken a check or tolerance; no form-code special cases; the gate runs
alone.
