# Notes on points 3, 4, 6 — #360 re-check

## Point 3 — the author's three claims

**(a) 1701A-122025-joint-many — CONFIRMED, the audit's own mistake.** The sample's steps.json adds
Part IV.A rows up to `txtCtrmodalPartIVA=12`; the correct builder is the twelve-row one (at main it
exists only inline in `form_1701a_joint_many_rows_sample_payload_is_current`; the PR factors it out
as `sample_1701a_joint_many()`). The original audit used `sample_1701a_joint()` — no rows, so
`other_income_42_description` stayed empty. Probe at main (75bd78b8) with the correct twelve-row
draft (`crates/bir-core/tests/probe3a.rs` scratch, since deleted): print map holds
`frm1701A:txt42Desc = Some("OTHERS")`, matching the official XML. The "HELD BUT NOT PRINTED" row for
txt42Desc was an artifact of the wrong draft.

**(b) 1702RT drpPg1I5AtcOther — the facts check out; the official-program print is not directly
verified.** Cell p1c16 is the small text box at the start of the second ("other") ATC row
(writer-cells.json: "the code cell at the start of the second ATC row"). The official XML holds
`IC010` even when "other" is not elected — the disabled `<select>` serializes its default. Printing
`IC010` there next to an unmarked election box would assert an ATC the filer did not elect, so blank
is the defensible paper behavior; the upload keeps the official default either way. What the
official program itself prints in that cell was not established here (no official-print harness on
this machine) — stated openly.

**(c) Case — the renderer upper-cases on main already; "case only" was never visible on paper.**
Proof: print map holds `frm0605:txtTaxPayerName = 'Sample Dummy Taxpayer'` (mixed case) at all three
commits, while the rendered cell p1c35 prints `SAMPLE DUMMY TAXPAYER` at 75bd78b8, at db022086 and at
71a5ae77 alike (extracted from the filled documents). Same for 1600PT's p1c26. The renderer also has
`is_email_key` ("Email inputs … are never uppercased on print") at main already, which is why the
email rows genuinely differ on paper (by design) while other case-only rows do not. Conclusion: of
the original audit's 103 case-only rows, the non-email ones were print-map artifacts invisible on
paper; the email ones are deliberate.

## Point 4 — the two permanent tests

`every_official_sample_joined_key_matches_official_xml_exact` (bir-core): compares the **print map**
(not the rendered cell), exact and case-sensitive with numeric/truth-mark normalization, over all 73
samples. `official_samples_print_parity.rs` (bir-print): compares **rendered cells** via
`filled_document` — but covers only the three 1702MX samples.

Exception list review:
- email keys — justified: the renderer deliberately never uppercases email (`is_email_key`,
  frozen_html.rs at main), so paper keeps the held case while the upload uppercases; structural.
- 1702RT other ATC — see 3(b); defensible, official-print behavior unverified.
- 1604C/1604E two-digit year — the sheet's caption reads "1 For the Year (20YY)" with the year cell
  `data-cell-kind="mixed"` (preprint + fillable slots); consistent with the "20 preprinted" claim.
  NOTE: on main the print put all four digits (`2025`) into that cell — whether that was a visible
  misprint depends on how the preprint and input overlap, which this audit did not render visually.
- 1701 spouse RDO "000" — plausible: no spouse column on paper when closed; consistent with the
  spouse-column logic. Accepted as design, not independently verified against the official sheet.
- unused 0.00 rows (1601C sched1, 2551Q ATC rows) — consistent with paper practice (unused rows
  blank); matches the official print conventions elsewhere. Accepted as design.

Would they have failed on main? **Yes — both, verified by execution:**
- `official_samples_print_parity.rs` copied unmodified into the 75bd78b8 tree compiles against
  main's code and FAILS (hundreds of cell mismatches across all three 1702MX samples); at 71a5ae77
  it passes (1/1 in the battery below).
- The bir-core test copied into the 75bd78b8 tree (only adaptation: a local shim for the not-yet-
  factored `sample_1701a_joint_many`, replicating the inline test body exactly) FAILS at main with
  1702MX mismatch lines; at 71a5ae77 it passes inside sample_payloads_test.
  Neither test is vacuous.

Scope note: the bir-print rendered test covers only 1702MX; everything else is guarded only at the
print-map level. The remaining pre-existing print gaps this audit shows on paper on other forms
(1701/1701Q centavo truncation, e.g. `250.25` printing as `250` — identical at main and at the PR,
outside the PR's claims) are NOT covered by either permanent test's failure mode: the map test
passes because the print map itself holds the truncated/rounded value. Worth a separate look —
they are real on paper if centavos are meant to print there (not established here).

## Point 6 — what changed in the sample builders

`crates/bir-core/tests/sample_payloads/form_2550q.rs, form_2551q.rs, form_2552.rs, form_2553.rs` and
`mod.rs`: **visibility only** — `fn` → `pub(crate) fn` on the sample builders and `mod` →
`pub(crate) mod` on the module list, so the new cross-crate permanent test can call them. No sample
content changed; no builder logic changed. The one behavioral-looking change in the diff is in
`form_1701a.rs` (17 lines): the twelve-row building logic was factored out of the inline test body
into `sample_1701a_joint_many()` — same construction, now callable. Verdict: no test helper was
changed to make a test pass; the changes are mechanical and justified.
