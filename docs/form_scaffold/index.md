# Additional Form Evidence Backlog

This directory is an evidence backlog, not an implementation or release-status
source. The Rust capability registry and
`packages/form-specs/form-migration-status.json` are authoritative.

The only supported new-form workflow is the repository-local
`.codex/skills/ebirforms-convert-form-to-html/SKILL.md`. The old form-generator,
formtype, coordinate-overlay, SVG-background, and generated-tax-code recipes
were removed so they cannot be mistaken for the production architecture.

## Reviewed source pack

The operator source pack is `/Users/uriah/Downloads/forms`. Its absolute path is
never read by a production build. Exact relative paths, official URLs, hashes,
page counts, point geometry, XML availability, and known blockers are recorded
in `packages/form-renderer/references/source-catalog.json`.

## Current ten-form HTML migration inventory

| Order | Exact target | Evidence state |
|---:|---|---|
| 1 | `2551Qv2018` | HTML certification in progress |
| 2 | `1601Cv2018` | HTML conversion in progress |
| 3 | `0619Ev2018` | Domain/XML conversion in progress |
| 4 | `0619Fv2018` | PDF and XML source locked |
| 5 | `0605v1999` | PDF and XML source locked |
| 6 | `1701Qv2018` | Preview can use the locked 2018 PDF; XML/filing remain blocked because the reviewed pack has no saved XML and the extracted eFPS template is an older, incompatible item layout |
| 7 | `2550Qv2024` | PDF and XML source locked |
| 8 | `1701v2018` | PDF, companion forms, and XML source locked |
| 9 | `1702RTv2018C` | PDF and XML source locked |
| 10 | `1702MXv2018C` | Blocked: typed model lacks attachment schedules |

No row above is fileable merely because source files exist. Each exact revision
must independently pass typed-model, formula, XML round-trip, persistence,
editor, contract, semantic HTML, pagination, visual, native output, and offline
package gates.

## Additional source leads

The source pack also contains official PDFs for forms outside the current
ten-form inventory, including 0620, 1600-PT, 1600-VT, 1601-EQ, 1601-FQ, 1602Q,
1603Q, 1604-C, 1604-F, 1606, 1621, 1701A, 1701-MS, 2000-DST, 2200 variants,
2316, 2550M, 2550-DS, and 2551M. These are source leads only. Add an exact
revision to the Rust registry and source catalog before implementation.

## Fail-closed intake rules

1. Lock exact code, revision, official PDF hash, page count, and paper geometry.
2. Inventory every plain and encrypted XML sample; preserve unknown fields.
3. Do not infer a formula, deadline, penalty, applicability rule, or submission
   capability from a single sample.
4. Keep unsupported forms selectable as manual/external Forms Set entries.
5. Never create a Typst template, formtype layout, full-page runtime background,
   or hidden legacy fallback.
