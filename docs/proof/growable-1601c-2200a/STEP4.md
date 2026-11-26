# Step 4 proof: growable upload rows

## Validation

`core-validation.log` (`rc:0`) runs the 20-row validation regression. There is
no Schedule I capacity refusal; row-local incomplete-row validation is unchanged
(the row checks still run per partially filled row).

## Official upload keys and round-trip

`core-rows.log` (`rc:0`) runs the writer/reader regression for 4, 7 and 20 rows
on both forms. It checks the exact highest continuous official key, the absence
of the next key, a structural `official_xml::read`, and typed round-trip equality.
1601C writes zero-based `frm1601c:sched1:*{N}`; 2200A writes one-based
`frm2200Av2020:txtSched1_xa_others{N}_*`, exactly as `OFFICIAL_KEYS.md` records.

`official_xml.rs` expands one contiguous official row block once per extra row.
It does not reuse the parked per-preallocated-key expansion that produced
duplicate keys.

## Page schema/host fill path

`host-1601c.log` and `host-2200a.log` (`rc:0`) fill rows 4-7 through the host
fill path and clear row 7. The 2200A runtime-generated rows keep `Due` filer
editable because the official HTA's `sched1Fields` input is not disabled; the
probe marks only the family template computed because editing also invokes
`computeOthers`.

`agent-derived.log` (`rc:0`) is the canonical regeneration/derivation test. The
pinned agent forms now carry rows 0..19 (1601C) and 1..20 (2200A); validation
and upload remain uncapped.

## Official samples unchanged

`sample-current.log` (`rc:0`) passes all 64 pinned sample payload tests, so the
official samples' uploads remain byte-identical to main's pinned payloads.
