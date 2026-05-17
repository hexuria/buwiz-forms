# Form Scaffold — Scaling to 35 Forms

> This document is the entry point for the remaining form implementation work.
> The detailed plan, individual task files, and instructions live in [`docs/form_scaffold/`](./docs/form_scaffold/index.md).

## Current State

| Metric | Value |
|--------|-------|
| **Implemented forms** | 10 (`ImplementedInApp`) |
| **Remaining forms** | 25 |
| **Total target** | 35 |
| **Tests passing** | 262 (0 failures) |

### Implemented Forms

| Form | Title | Type ID |
|------|-------|---------|
| 2551Q | Quarterly Percentage Tax | `2551Qv2018` |
| 1601C | Monthly Withholding (Compensation) | `1601Cv2018` |
| 2550Q | Quarterly VAT | `2550Qv2024` |
| 1701Q | Quarterly Income Tax (Individual) | `1701Qv2018` |
| 1702RT | Annual Corporate ITR (Regular) | `1702RTv2018C` |
| 1702MX | Annual Corporate ITR (Mixed) | `1702MXv2018C` |
| 1701 | Annual Income Tax (Individual) | `1701v2018` |
| 0605 | Payment Form | `0605v1999` |
| 0619E | Monthly Expanded Withholding | `0619Ev2018` |
| 0619F | Monthly Final Withholding | `0619Fv2018` |

---

## What's Left

### Tier B — 14 forms (has PDF + eFPS manifest, needs Windows savefile)

| Form | Title | Task |
|------|-------|------|
| 1601EQ | Quarterly Expanded Withholding | [→ task](docs/form_scaffold/1601eq.md) |
| 1601FQ | Quarterly Final Withholding | [→ task](docs/form_scaffold/1601fq.md) |
| 1602Q | Final Tax on Govt Money Payments | [→ task](docs/form_scaffold/1602q.md) |
| 1603Q | Final Tax on Fringe Benefits | [→ task](docs/form_scaffold/1603q.md) |
| 1604CF | Annual Info Return (Comp + Final) | [→ task](docs/form_scaffold/1604cf.md) |
| 1604E | Annual Info Return (Expanded) | [→ task](docs/form_scaffold/1604e.md) |
| 1702Q | Quarterly Corporate ITR | [→ task](docs/form_scaffold/1702q.md) |
| 1702EX | Annual Corporate ITR (Exempt) | [→ task](docs/form_scaffold/1702ex.md) |
| 2550M | Monthly VAT | [→ task](docs/form_scaffold/2550m.md) |
| 2551M | Monthly Percentage Tax | [→ task](docs/form_scaffold/2551m.md) |
| 2000-DST | Documentary Stamp Tax | [→ task](docs/form_scaffold/2000dst.md) |
| 2200A | Excise Tax (Alcohol) | [→ task](docs/form_scaffold/2200a.md) |
| 2200M | Excise Tax (Minerals) | [→ task](docs/form_scaffold/2200m.md) |
| 2200P | Excise Tax (Petroleum) | [→ task](docs/form_scaffold/2200p.md) |

### Tier C — 11 forms (has PDF only, needs savefile + formula research)

| Form | Title | Task |
|------|-------|------|
| 0620 | Withholding on Decedent Deposits | [→ task](docs/form_scaffold/0620.md) |
| 1600PT | Percentage Taxes Withheld (Govt) | [→ task](docs/form_scaffold/1600pt.md) |
| 1600VT | VAT Withheld (Govt) | [→ task](docs/form_scaffold/1600vt.md) |
| 1606 | Withholding on Real Property Transfer | [→ task](docs/form_scaffold/1606.md) |
| 1621 | Quarterly Final Tax Withheld | [→ task](docs/form_scaffold/1621.md) |
| 1701A | Annual ITR (8%/BMBE Individual) | [→ task](docs/form_scaffold/1701a.md) |
| 1707A | Capital Gains Tax (Shares) | [→ task](docs/form_scaffold/1707a.md) |
| 1709 | Related Party Transactions (TP) | [→ task](docs/form_scaffold/1709.md) |
| 2200C | Excise Tax (Tobacco) | [→ task](docs/form_scaffold/2200c.md) |
| 2316 | Compensation Certificate | [→ task](docs/form_scaffold/2316.md) |
| 2550DS | VAT on Digital Services | [→ task](docs/form_scaffold/2550ds.md) |

---

## Next Action Required

**You need a Windows machine.** The blocker for all 25 forms is generating XML savefiles from the official eBIRForms client. See [savefile generation instructions](docs/form_scaffold/index.md#windows-savefile-generation-instructions) for details.

Once you have savefiles, point the AI agent at any task file (e.g., `docs/form_scaffold/1601eq.md`) and say:
> "I have the savefile for 1601EQ. Execute this task."

The agent will run the pipeline and integrate the form automatically.
