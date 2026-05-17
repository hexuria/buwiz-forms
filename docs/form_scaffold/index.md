# Form Scaffold Plan — Remaining 25 Forms

> **Goal:** Scale eBIRForms from 10 implemented forms to the full 35-form registry.
>
> **Current state:** 10 forms are `ImplementedInApp`. 25 forms need Rust structs, XML mappings, views, formtype layouts, and formula verification before they can be filed.

---

## How This Plan Is Organized

Each form has its own task file in this directory with hyper-focused instructions. Forms are grouped into **tiers** by readiness — how much source material already exists.

| Tier | Description | Count | Blocker |
|------|-------------|:-----:|---------|
| **A** | Has XML savefile + PDF + eFPS manifest. Run the pipeline. | 0 | — |
| **B** | Has PDF + eFPS manifest. Needs Windows savefile only. | 14 | Windows machine |
| **C** | Has PDF only. Needs Windows savefile + manual formula research. | 11 | Windows + formula research |

---

## Current Inventory (10 Implemented ✅)

| Form | Title | Type ID | Since |
|------|-------|---------|-------|
| 2551Q | Quarterly Percentage Tax | `2551Qv2018` | Pre-existing |
| 1601C | Monthly Withholding (Compensation) | `1601Cv2018` | Pre-existing |
| 2550Q | Quarterly VAT | `2550Qv2024` | Phase 1 |
| 1701Q | Quarterly Income Tax (Individual) | `1701Qv2018` | Phase 1 |
| 1702RT | Annual Corporate ITR (Regular) | `1702RTv2018C` | Phase 1 |
| 1702MX | Annual Corporate ITR (Mixed) | `1702MXv2018C` | Phase 1 |
| 1701 | Annual Income Tax (Individual) | `1701v2018` | Phase 1 |
| 0605 | Payment Form | `0605v1999` | Phase 2 |
| 0619E | Monthly Expanded Withholding | `0619Ev2018` | Phase 2 |
| 0619F | Monthly Final Withholding | `0619Fv2018` | Phase 2 |

---

## Tier B — PDF + eFPS Manifest (needs Windows savefile)

These forms have official PDFs in `~/Downloads/forms/` AND eFPS HTML manifests in `docs/efps_manifests/`. Once a savefile is generated, the full pipeline can run immediately.

| # | Form Code | Title | eFPS Manifest | Task File |
|---|-----------|-------|:---:|-----------|
| 1 | 1601EQ | Quarterly Expanded Withholding | `1601E.json` | [1601eq.md](./1601eq.md) |
| 2 | 1601FQ | Quarterly Final Withholding | `1601F.json` | [1601fq.md](./1601fq.md) |
| 3 | 1602Q | Quarterly Percentage Tax on Govt Money Payments | `1602.json` | [1602q.md](./1602q.md) |
| 4 | 1603Q | Quarterly Final Tax on Govt Money Payments | `1603.json` | [1603q.md](./1603q.md) |
| 5 | 1604CF | Annual Info Return (Compensation & Final) | `1604CF.json` | [1604cf.md](./1604cf.md) |
| 6 | 1604E | Annual Info Return (Expanded/Creditable) | `1604E.json` | [1604e.md](./1604e.md) |
| 7 | 1702Q | Quarterly Corporate ITR | `1702Q.json` | [1702q.md](./1702q.md) |
| 8 | 1702EX | Annual Corporate ITR (Exempt) | `1702.json` | [1702ex.md](./1702ex.md) |
| 9 | 2550M | Monthly VAT | `2550M.json` | [2550m.md](./2550m.md) |
| 10 | 2551M | Monthly Percentage Tax | `2551M.json` | [2551m.md](./2551m.md) |
| 11 | 2000-DST | Documentary Stamp Tax | `2000.json` | [2000dst.md](./2000dst.md) |
| 12 | 2200A | Excise Tax (Alcohol) | `2200A.json` | [2200a.md](./2200a.md) |
| 13 | 2200M | Excise Tax (Mineral Products) | `2200M.json` | [2200m.md](./2200m.md) |
| 14 | 2200P | Excise Tax (Petroleum) | `2200P.json` | [2200p.md](./2200p.md) |

## Tier C — PDF Only (needs savefile + formula research)

These have official PDFs but NO eFPS manifest. Formulas must be manually derived from the PDF instructions.

| # | Form Code | Title | Task File |
|---|-----------|-------|-----------|
| 15 | 0620 | Monthly Remittance of Tax Withheld on Final Income | [0620.md](./0620.md) |
| 16 | 1600PT | Monthly Remittance of Other Percentage Taxes Withheld | [1600pt.md](./1600pt.md) |
| 17 | 1600VT | Monthly Remittance of VAT Withheld | [1600vt.md](./1600vt.md) |
| 18 | 1606 | Withholding Tax Remittance (PERA) | [1606.md](./1606.md) |
| 19 | 1621 | Quarterly Remittance of Tax Withheld on Fringe Benefits | [1621.md](./1621.md) |
| 20 | 1701A | Annual ITR (Individuals — 8% / BMBE) | [1701a.md](./1701a.md) |
| 21 | 1707A | Capital Gains Tax on Real Property | [1707a.md](./1707a.md) |
| 22 | 1709 | Information Return on Transactions with Related Parties | [1709.md](./1709.md) |
| 23 | 2200C | Excise Tax (Cigars & Cigarettes) | [2200c.md](./2200c.md) |
| 24 | 2316 | Certificate of Compensation Payment/Tax Withheld | [2316.md](./2316.md) |
| 25 | 2550DS | Monthly/Quarterly VAT (Digital Services) | [2550ds.md](./2550ds.md) |

---

## Pipeline Reference

Every new form follows the same 8-step pipeline. See the [form-generator skill](../../.agent/skills/form-generator/SKILL.md) for full details.

### Quick Reference

```
Step 1: discover_sources.py   → Identify savefile XML + PDF
Step 2: parse_payload.py      → Extract fields from XML savefile
Step 3: parse_efps_manifest.py → Extract computed field hints from eFPS
Step 4: extract_pdf_layout.py  → Generate formtype (SVG + field coordinates)
Step 5: reconcile_layout.py    → Map XML fields → PDF field positions
Step 6: generate_typed_form.py → Generate Rust struct + XML + view + tests
Step 7: validate_generated_form.py → Check completeness
Step 8: Manual integration     → Wire into mod.rs, FormDraft, support_level.rs
```

### Integration Checklist (Step 8)

After generating code for any form, these files must be updated:

1. **`crates/bir-core/src/forms/mod.rs`**
   - Add `pub mod form_<id>;` and `pub mod form_<id>_xml;`
   - Add variant to `FormDraft` enum
   - Implement `form_code()`, `status()`, `validate()` match arms

2. **`crates/bir-core/src/temporal/support_level.rs`**
   - Add form code to `FILEABLE_FORM_CODES`
   - Add entry to `fileable_form_type_id()`

3. **`crates/bir-desktop/src/views/mod.rs`**
   - Add `pub mod form_<id>_view;`

4. **`crates/bir-desktop/src/app.rs`**
   - Add routing for the new form view

5. **`crates/bir-core/src/db/drafts.rs`**
   - Add serialization/deserialization support for the new draft type

6. **Build & test:**
   ```bash
   cargo build && cargo test --package bir-core
   ```

---

## Windows Savefile Generation Instructions

### Prerequisites
- Windows 10/11 machine (or VM)
- [eBIRForms Package v7.9.3](https://www.bir.gov.ph/e-services/ebirforms) installed
- Java Runtime (bundled with eBIRForms)

### Batch Process

For each form code:

1. Launch eBIRForms → Select the form
2. Fill in the **minimum required fields** with dummy data:
   - TIN: `000-000-000-000`
   - Taxpayer Name: `TEST TAXPAYER`
   - RDO Code: `039`
   - Period: current year/month
   - All monetary fields: enter `1,000.00` (to exercise formulas)
3. Click **Save** → save to a known directory
4. The saved file will be named like `00000000000000-<FORMID>-<PERIOD>.xml`
5. Copy the XML file to `~/Downloads/forms/<FormDirName>/` on your Mac

### Priority Order

Generate savefiles in this order (highest BIR filing volume first):

```
Batch 1 (Monthly withholding — most frequently filed):
  0620, 1600PT, 1600VT, 1606

Batch 2 (Quarterly returns):
  1601EQ, 1601FQ, 1602Q, 1603Q, 1621, 1702Q

Batch 3 (Monthly returns):
  2550M, 2551M

Batch 4 (Annual returns):
  1604CF, 1604E, 1702EX, 1701A

Batch 5 (Specialized):
  2000-DST, 2200A, 2200C, 2200M, 2200P, 1707A, 1709, 2316, 2550DS
```

---

## Success Criteria

- [ ] All 35 forms have `FormDraft` variants in `mod.rs`
- [ ] All 35 forms have `formtype` directories with SVG pages
- [ ] All 35 forms are registered in `FILEABLE_FORM_CODES`
- [ ] All 35 forms pass `transition_to_queued()` with valid data
- [ ] `cargo build && cargo test` passes with 0 failures
