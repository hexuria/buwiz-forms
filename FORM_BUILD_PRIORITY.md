# eBIRForms Build Priority

Updated: 2026-07-21

## Corrected Scope

- Current product scope after removing retired and older counterparts: **43 forms**
- Forms currently registered in the renderer: **10**
- Existing forms treated as completed for this roadmap: **6**
- Existing but unfinished 170x forms: **4**
- Forms not yet registered: **33**, including **1701-MS**
- Total remaining work: **37 items** — finish 4 existing forms and add 33 new forms

The six forms treated as completed are:

1. 0605
2. 0619E
3. 0619F
4. 1601C
5. 2550Q
6. 2551Q

The four existing but unfinished 170x forms are:

1. 1701Q — quarterly
2. 1701 — annual
3. 1702RT — annual
4. 1702MX — annual and conditional

1701-MS is already included in the 43-form scope. It is missing from the renderer, so it is one of the 33 new forms rather than a 44th form.

## Ordered Build Queue

### Priority 1 — Common Monthly Forms

This priority is complete:

1. 1601C
2. 0619E
3. 0619F

### Priority 2 — Quarterly Forms

1. **1701Q** — finish the existing implementation first
2. **1601EQ** — first completely new form
3. **1702Q**
4. **1601FQ**
5. **1603Q**

The already completed quarterly forms are 2550Q and 2551Q.

1602Q is quarterly but is deferred because it is principally for banking, financial, trust, and similar specialized taxpayers.

### Priority 3 — Conditional or Optional Monthly Forms

1. **1600VT**
2. **1600PT**
3. **2550M** — optional monthly VAT filing

1600WP is monthly but deferred because it is specifically for race-track operators.

### Priority 4 — Annual Forms

Build ordinary individual and business forms before conditional or specialist annual forms:

1. **1701-MS** — micro and small individual taxpayers
2. **1701** — finish existing implementation
3. **1701A** — individuals earning purely from business or profession
4. **1700** — compensation-income filers who must file an annual return
5. **1702RT** — finish existing regular corporate return
6. **1604C**
7. **1604E**
8. **1604F**
9. **1702MX** — finish existing conditional/mixed-rate corporate return

### Priority 5 — Event-Based Forms

1. 1706
2. 1606
3. 1800
4. 1801
5. 2000OT
6. 2000

### Deferred — Specialist Forms

1. 1602Q — banks, financial institutions, trusts, and similar taxpayers
2. 1600WP — race-track operators
3. 1702EX — exempt or special non-individual taxpayers
4. 1707
5. 1707A
6. 2552 — listed shares and public offerings
7. 2553 — percentage tax under special laws
8. 2200A — excise tax
9. 2200AN — excise tax
10. 2200C — excise tax
11. 2200M — excise tax
12. 2200P — excise tax
13. 2200S — excise tax
14. 2200T — excise tax

## Immediate Execution Sequence

The next work item is **1701Q**, because it is already implemented but unfinished and is a quarterly form.

The first completely new form is **1601EQ**.

The first annual form is **1701-MS**, after the quarterly and conditional-monthly queues are complete.

## Definition of Built

A form is considered built only after all of the following are complete:

1. Pin the exact official BIR PDF revision and guide.
2. Capture and verify a genuine XML file from the official eBIRForms package.
3. Implement the typed data model, calculations, and validation rules.
4. Implement the editor, save, reopen, and persistence workflow.
5. Build semantic HTML with exact page size and deterministic pagination.
6. Verify visual parity against the official PDF.
7. Verify native preview, native printing, and PDF export.
8. Verify packaged offline operation.
9. Enable queue submission only after XML round-trip and filing validation pass.
10. Update the migration and release-readiness tracker.

Appearing in the calibration viewer is development evidence, not by itself proof that a form is fileable or release-ready.

## 1701-MS Source Status

The local source directory already contains:

- `1701MSv2024/1701-MS August 2024 Fillable_01.pdf`
- `1701MSv2024/1701-MS Guide August 2024 ENCS_Final.pdf`

No XML example is currently present in that directory. Before implementing filing support, generate and save a representative 1701-MS XML file using the official Offline eBIRForms Package 7.9.6.

## Official References

- BIR eBIRForms catalog: https://www.bir.gov.ph/ebirforms
- RMC No. 37-2026, electronic filing of 1701-MS: https://bir-cdn.bir.gov.ph/BIR/pdf/RMC%20No.%2037-2026%20Digest.pdf
- RMC No. 52-2023, optional filing of 2550M: https://bir-cdn.bir.gov.ph/local/pdf/RMC%20No.%2052-2023.pdf
- RMC No. 79-2023, periodic classifications and filing guidance: https://bir-cdn.bir.gov.ph/local/pdf/RMC%20No.%2079-2023.pdf
