# Excel demo: filling a return from a workbook

Two sample workbooks for the test taxpayer **Dela Cruz Jane, TIN
123-456-789-00000** (head office). **Every name and amount in them is made up.**
Use them only with that test taxpayer, on a test database, and never with a
real taxpayer's data.

| Workbook | Return |
| --- | --- |
| `2551Q-2026-Q3-sales.xlsx` | 2551Q, Q3 2026 (`@bir:taxform:123456789/2551Q-2026-Q3`) |
| `1601C-2026-09-payroll.xlsx` | 1601-C, September 2026 (`@bir:taxform:123456789/1601C-2026-09`) |

`make_workbooks.py` rewrites both (`python3 -I crates/bir-shell/demo/make_workbooks.py`).
The totals are formulas whose values are stored too, so a reader that does not
compute formulas sees what Excel shows. The test
`mini_app::demo_tests` in `src/mini_app/demo_tests.rs` reads each workbook,
fills its return through the agent's own tools on a scratch database, and
checks that the return validates.

## The demo script

1. In a Rox chat, type `@bir:taxform:123456789/2551Q-2026-Q3` (or
   `/taxform 2551Q Q3 2026` with Jane as the current taxpayer), attach
   `2551Q-2026-Q3-sales.xlsx`, and ask: "Fill this return from the workbook."
2. The agent calls `bir_form_open` with the token's value, then
   `bir_form_needs_you` and `bir_form_fields`, reads the workbook, and fills
   with `bir_form_fill` (`source: "document"`) using the mapping below.
3. It calls `bir_form_validate` and walks the user through anything left,
   one item at a time, with the official rule for each.
4. It saves the draft (`bir_form_save_draft`) and offers to show it:
   `bir_app_open {"app": "forms", "form": "123456789/2551Q-2026-Q3"}`.
5. It never submits. Without a send grant for Jane and the form, the user
   presses Submit in the Forms app.

Repeat with `@bir:taxform:123456789/1601C-2026-09` and the payroll workbook.

## 2551Q: `2551Q-2026-Q3-sales.xlsx`

Columns: Month, Gross sales, ATC, Tax rate, Percentage tax; a `Total` row.

| Workbook | Return box (`bir_form_fill` key) | Value |
| --- | --- | --- |
| Total of **Gross sales** (Jul to Sep) | Schedule 1 row 1 Taxable Amount (`txtATCAmt1`) | 545,650.00 |
| **ATC** (every row PT010) | Schedule 1 row 1 ATC | PT010, already set |
| Total of **Percentage tax** | Schedule 1 row 1 Tax Due, Item 14 Total Tax Due | 16,369.50, computed by the form; check it matches |
| (none: no BIR Form 2307 received) | Item 15 Creditable Percentage Tax Withheld (`frm2551Qv2018:txt15`) | 0.00 |

Notes for the agent:

- **ATC.** Schedule 1's ATC dropdowns (`drpATC1` to `drpATC6`) cannot be
  filled with `bir_form_fill` yet. A new 2551Q starts with row 1 set to
  **PT010**, so this workbook needs no ATC filled: put the quarter's total on
  row 1 and confirm the computed tax due equals the workbook's total. If a
  workbook had another ATC, or more than one, do not guess: ask the user to
  pick the ATC on Schedule 1 in the Forms app, then fill the amounts.
- **Item 13** (graduated or 8% income tax rate) is the taxpayer's election.
  The agent never answers it. It comes from the tax profile's recorded
  election (or the filed Q1); if `bir_form_needs_you` lists it, ask the user
  to answer it in the Forms app. In the test, Jane's profile records the
  graduated rates for 2026 (as filing her Q1 records them), and she has
  answered Q3's Item 13 "Not applicable" in the Forms app before the agent
  fills the return.
- Fill only what the workbook shows. Leave penalties (Items 20 to 22) to the
  form: a late return computes them, and the agent tells the user.

## 1601-C: `1601C-2026-09-payroll.xlsx`

One row per employee: Employee, Position, Basic pay, Overtime pay, De minimis
benefits, Gross compensation, SSS, PhilHealth, Pag-IBIG, Statutory
contributions, Taxable compensation, Tax withheld; a `Total` row.

| Workbook | Return box (`bir_form_fill` key) | Value |
| --- | --- | --- |
| (taxes were withheld) | Item 3 Any taxes withheld? (`frm1601c:TaxWithheld`) | `1` (Yes) |
| (a private employer) | Item 11 Category of withholding agent (`frm1601c:CatAgent`) | `P` (Private) |
| Total **Gross compensation** | Item 14 Total amount of compensation (`frm1601c:txtTax14`) | 134,800.00 |
| (no minimum wage earners) | Item 15 Statutory minimum wage (`frm1601c:txtTax15`) | 0.00 |
| (no minimum wage earners) | Item 16 MWE holiday, overtime, night and hazard pay (`frm1601c:txtTax16`) | 0.00 |
| (no 13th month pay in September) | Item 17 13th month pay and other benefits (`frm1601c:txtTax17`) | 0.00 |
| Total **De minimis benefits** | Item 18 De minimis benefits (`frm1601c:txtTax18`) | 7,000.00 |
| Total **Statutory contributions** (SSS + PhilHealth + Pag-IBIG) | Item 19 Mandatory contributions (`frm1601c:txtTax19`) | 9,550.00 |
| (none) | Item 20 Other non-taxable compensation (`frm1601c:txtTax20`) | 0.00 |
| (none) | Item 23 Taxable compensation not subject to withholding (`frm1601c:txtTax23`) | 0.00 |
| Total **Tax withheld** | Item 25 Total taxes withheld (`frm1601c:txtTax25`) | 2,595.15 |
| Total **Taxable compensation** | Item 22 Total taxable compensation | 118,250.00, computed by the form; check it matches |

Notes for the agent:

- Overtime pay of employees who are not minimum wage earners is taxable: it
  is in Gross compensation (Item 14), not in Item 16.
- Item 21 (total non-taxable, 16,550.00), Item 22 and Items 24 to 36 are
  computed. Check Item 22 against the workbook's taxable total and Item 27
  against its tax withheld; if they differ, tell the user which column
  disagrees instead of forcing a box.
- September 2026's 1601-C was due on October 12, 2026 (October 10 fell on a
  Saturday). Filed later, the form shows its penalties in Items 32 to 34:
  tell the user.
