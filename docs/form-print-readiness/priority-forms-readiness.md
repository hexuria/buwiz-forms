# Priority Forms Readiness Matrix

This matrix tracks the six variants from `prioritized_form.md`. A form is
`ImplementedInApp` only after source provenance, typed behavior, XML payload,
persistence, queue submission, and calibrated print preview are all proven.

## Status Legend

- `Ready`: verified enough for promotion.
- `Draft`: generated or scaffolded, but still needs calibration or formula proof.
- `Missing`: not implemented in the repo.
- `Blocked`: deliberately gated to prevent unsafe filing.

## Priority Forms

| Variant | Source assets | Typed draft | Field map/XML | Formula evidence | Persistence | Queue submission | Print pack | Support level |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `1601Cv2018` | Ready: PDF + XML in `/Users/uriah/Downloads/forms/1601Cv2018` | Ready | Ready | Ready | Ready | Ready | Ready: `formtypes/1601Cv2018` | `ImplementedInApp` |
| `1701v2018` | Ready: PDF + XML in `/Users/uriah/Downloads/forms/1701v2018` | Draft | Draft | Draft | Draft | Blocked | Draft: `formtypes/1701v2018` has placeholder keys | `ScaffoldOnly` |
| `1701Qv2018` | Ready: PDF in `/Users/uriah/Downloads/forms/1701Qv2018` | Draft: lightweight struct only | Draft: profile/period/current totals only | Missing | Draft | Blocked until full typed/XML parity | Draft: `formtypes/1701Qv2018` has placeholder keys | `ScaffoldOnly` |
| `2550Qv2024` | Ready: PDF + XML in `/Users/uriah/Downloads/forms/2550Qv2024` | Draft | Draft | Draft | Draft | Blocked | Draft: `formtypes/2550Qv2024` has placeholder keys | `ScaffoldOnly` |
| `1702RTv2018C` | Ready: PDF + XML in `/Users/uriah/Downloads/forms/1702RTv2018c` | Draft | Draft | Draft | Draft | Blocked | Draft: `formtypes/1702RTv2018C` has placeholder keys | `ScaffoldOnly` |
| `1702MXv2018C` | Ready: PDF + XML in `/Users/uriah/Downloads/forms/1702MXv2018c` | Draft | Draft | Draft | Draft | Blocked | Draft: `formtypes/1702MXv2018C` has placeholder keys | `ScaffoldOnly` |

## Promotion Gate

Before changing any priority form to `ImplementedInApp`, the implementing agent
must complete all of these checks:

- `to_bir_field_map()` covers the official BIR field keys needed for XML and print.
- `to_bir_xml_payload()` round-trips key values from the local sample XML.
- Required formulas are backed by local sample XML, PDF line labels, or a cited BIR rule.
- Draft persistence uses the correct period key: monthly `M01-M12`, quarterly `Q1-Q4`, annual `Annual`.
- Queue submission has a concrete background-cron branch using the correct form type ID and filename.
- `formtype.json` has calibrated BIR field keys, not `_field_NNN` placeholders.
- `render_flat_pdf(PrintRequest)` creates a non-empty PDF from representative draft data.
- Desktop submit labels and support levels match the actual queue capability.

## Immediate Priority

1. Complete `1701Qv2018` typed parity first because it has priority-list
   source material but the Rust draft still lacks XML and print mappings.
2. Complete `2550Qv2024` next because it already has a rich generated struct and
   local PDF/XML evidence.
3. Complete `1701v2018`, then `1702RTv2018C`, then `1702MXv2018C`; these have
   large field surfaces and should stay scaffold-only until calibration is done.
