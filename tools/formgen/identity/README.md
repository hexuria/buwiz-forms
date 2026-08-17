# Field identity

A **field identity** is a name for one taxpayer-facing box that still means
that box after the lattice renumbers `p1c13`. It is not a bbox string and it
is not an HTML `id`. Stage 3 will join identities to official
`serialized_key` values. This directory is not that join.

    STAGE 1  GENERATE   forms/          (batch-versioned; `p1cN` may move)
    STAGE 2  CORRECT    forms-corrected/
    IDENTITY            this catalog    (durable id + printed box)
    STAGE 3  MAP        identity → eBIRForms XML key   (not started)

## What is frozen

Each record's `id` (for example `2550m-2007/p1/tin-branch`) never changes
once published. The artwork identity is `source_printed_box_pt`, measured
from the pinned PDF. `html_id_hint` is the current batch's `p1cN` and is
non-authoritative: a geometry fix that renumbers cells must update the hint
in the same commit (a schema change), but it must not mint a new identity.

## How a record is resolved

`field_identity.py` parses the named tree with the stdlib HTML parser — not
`emit.py`, not `lattice.py`. It collects `data-field-kind="comb"` boxes and
requires **exactly one** whose emitted rectangle overlaps the printed box
(tolerance `match.tolerance_pt`). A white knockout covering the strip is
`data-cell-kind="blank"` and is ignored.

| Result | Meaning |
| --- | --- |
| resolved | exactly one comb, and its `id` equals `html_id_hint` |
| html_id_hint_stale | exactly one comb, different `id` — update the catalog in this commit |
| unresolved | zero combs overlap the printed box |
| ambiguous | two or more combs overlap — the printed box no longer names one field |

Zero or two is a failure. A stale hint is also a failure: silent remapping
is risk R2. The identity id still names the same box; only the hint moves.

## What this is not

- Not Stage 3. Nothing writes `name="frm2550m:txtBranchCode"`.
- Not verification of C01–C07. Overlap does not re-derive `expected_effect`.
- Not a census of every fillable field. The seed is the seven TIN branch
  subjects already bound by printed boxes in the Stage 2 ledger.

```sh
python3 tools/formgen/field_identity.py --self-test
python3 tools/formgen/field_identity.py check --tree forms-corrected
python3 tools/formgen/field_identity.py check --tree forms
```
