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
`emit.py`, not `lattice.py`. It collects fillable boxes: `data-cell-kind` is
`field` *or* `mixed`, and `data-field-kind` is set (comb or text). C01's
first TIN group and C06's agent TIN emit as `text` on the stage-1 batch.
G11 mixed combs are the branch identity when the sheet pre-prints `000` and
emit refuses empty slots. A white knockout covering the strip is
`data-cell-kind="blank"` and is ignored. Dash separators are `data-cell-kind=
"field"` with no `data-field-kind`; they are ignored too, because the even
reflow parks their centers inside the previous group's printed box.

Match is **center-in-printed-box**, not raw overlap. Stage 2 even-3-3-3-5
reflow expands the branch left, so neighbouring groups nick each other's
old edges by a fraction of a point. The emitted field whose center still
sits in `source_printed_box_pt` (tolerance `match.tolerance_pt`) is the
same subject. Exactly one such hit, whose `id` equals `html_id_hint`, is
success.

| Result | Meaning |
| --- | --- |
| resolved | exactly one field center in the printed box, and its `id` equals `html_id_hint` |
| html_id_hint_stale | unique center hit, different `id` — update the catalog in this commit |
| unresolved | no field center sits in the printed box |
| ambiguous | two or more field centers sit in the printed box |

Zero or two is a failure. A stale hint is also a failure: silent remapping
is risk R2. The identity id still names the same box; only the hint moves.

## Coverage

208 identities. The C01–C07 seed is 7 strips (28). I0 added the measured
3+3+3+5 TIN caption chain on 38 more bundles (152). I1 added 28 leftovers:
`extra/1801-2018` as a mixed `tin-strip` plus tin-2/3/branch, extra HTML
3+3+3+N chains that uniquely resolve (spouse / page-2 / extra), from
[`tin-identity-leftovers-20260818.json`](../corrections/evidence/tin-identity-leftovers-20260818.json).
Eight PDF-census not-measurable bundles still emit no 3+3+3+N chain.

This is not yet coverage of every fillable field.

## What this is not

- Not Stage 3. Nothing writes `name="frm2550m:txtBranchCode"`.
- Not verification of C01–C07. Overlap does not re-derive `expected_effect`.
- Not a census of every fillable field.

```sh
python3 tools/formgen/field_identity.py --self-test
python3 tools/formgen/field_identity.py check --tree forms-corrected
python3 tools/formgen/field_identity.py check --tree forms
python3 tools/formgen/field_identity.py coverage --tree forms
python3 tools/formgen/field_identity.py coverage --tree forms-corrected
python3 tools/formgen/field_identity.py ledger-check --tree forms
```

`coverage` lists fillable cells whose center sits in no catalog `source_printed_box_pt`. It is not a gate until remainder minting (I3) brings the count to 0. `ledger-check` requires every `pXcN` in an open finding's `where`/`what` to exist as a live fillable id or a catalog `html_id_hint`.
