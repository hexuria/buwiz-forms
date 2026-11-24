# Form-Review Tooling Faults

Observed during the eight-form audit on `try/forms-neon-1-b` (Linux desktop via `nsh2404.sh`, `gpui-agent 0.4.0`, `nsh 0.6.1 (ca39c0e)`).

---

## 1. `gpui-agent snapshot --refs` omits `cell-0` inside a `Sheet` drawer's `scroll`

### Symptom
When opening a schedule row's `Add row` drawer (`nsh/screens/form/controls/Schedule.tsx`, `DrawerCell`), `gpui-agent --session rev-win snapshot --refs` omits the first rendered column (`fp-table-<key>-cell-0`) from the accessibility tree while listing `cell-1`, `cell-2`, `cell-3`, etc.

A real window screenshot proves column 0 **is** visually rendered at the top of the drawer; only the accessibility snapshot omits it.

### Reproducers
1. **2550M — Schedule 2 (`Purchases of Capital Goods not exceeding P1M`)**
   - Open `2550M`, navigate to `Schedule 2`, press `Add row`.
   - Visual drawer renders 4 controls top-to-bottom:
     - `cell-0`: `Date Purchased` (`MM/DD/YYYY` date input)
     - `cell-1`: `Description` (textbox)
     - `cell-2`: `Amount` (money textbox)
     - `cell-3`: `Input Tax` (money textbox)
   - `gpui-agent --session rev-win snapshot --refs` starts at `fp-table-txtDatePurchased0-cell-1` (`Description`) and completely omits `fp-table-txtDatePurchased0-cell-0` (`Date Purchased`).
   - Impact: automated UI filling enters only `cell-1..3` and presses `Done`, causing `crates/bir-app-api/src/screen_form_module.rs:11682-11688` to reject the row for missing the required date and leave `"No rows yet."`.

2. **1601EQ — Part II Items 13–18 (`ATC` schedule)**
   - Open `1601EQ`, navigate to `Part II`, press `Add row`.
   - Visual drawer renders 4 controls top-to-bottom:
     - `cell-0`: `ATC` (`Choose...` selector)
     - `cell-1`: `Tax Base` (money textbox)
     - `cell-2`: `Tax Rate` (computed)
     - `cell-3`: `Tax Withheld` (computed)
   - `gpui-agent --session rev-win snapshot --refs` starts at `cell-1` (`Tax Base`) and omits `cell-0` (`ATC` selector).
   - Impact: automated UI filling cannot select an ATC from the drawer via `snapshot --refs`, so rows fail validation and Part II Items 13–18 remain blank.

### Relevant UI code
- `nsh/screens/form/controls/Schedule.tsx`: `DrawerCell` renders `<View testID={`${props.p.testID}-cell-${props.column.index}`}>` for every non-hidden column starting at `index = 0`.
- `nsh/screens/form/rows.tsx`: builds `columns` with `index = 0` included (`testID: fp-table-${keys[0]?.[0] ?? id}`).

---

## 2. `form.fill` refuses keys that the form page can fill

### Symptom
`gpui-agent --session rev-data invoke form.fill` rejects schedule and editor-owned keys even though the live form page edits those same keys and the internal Rust API (`crates/bir-app-api/src/screen_form_module.rs`) accepts them.

### Exact refused keys and messages

1. **2550M — Schedule 2–8 row keys (131 keys refused)**
   - Keys (examples from Schedule 2):
     - `txtDatePurchased0`
     - `txtDescription0`
     - `txtAmt0`
     - `txtInputTax0`
     - (and analogous indexed keys across Schedules 2, 3A, 3B, 6, 7, and 8)
   - Exact error message returned by `form.fill`:
     > `` `txtAmt0` cannot be filled from here: it is written through the form's own editors ``
     > `` `txtDatePurchased0` cannot be filled from here: it is written through the form's own editors ``
     > `` `txtDescription0` cannot be filled from here: it is written through the form's own editors ``
   - Contrast: `crates/bir-app-api/src/screen_form_module.rs:11681-11710` fills these exact keys in unit tests (`"txtDatePurchased0": "08/05/2026"`, `"txtDescription0": "Office printer"`, `"txtAmt0": "85000.55"`), and the live form page edits them through the Schedule drawer.

2. **1601EQ — Part II Items 13–18 ATC schedule keys**
   - Keys: `frm1601EQ:txtAtcCd1`..`frm1601EQ:txtAtcCd6`, `frm1601EQ:txtTaxBase1`..`frm1601EQ:txtTaxBase6` (and overflow rows `txtAtcCd7+`).
   - Cause / behavior: `txtAtcCd1..6` and `txtTaxBase1..6` are unmapped in `forms/1601eq-v2018.json`, and runtime schedule keys are refused by `form.fill` as editor-owned (same pattern as 2550M), while `forms/schema-overrides.json:5325+` confirms they are editable through the host writer's ATC popup (`catalogs.rs atc_1601eq_row`).

3. **1601C — Schedule I Month/Year keys**
   - Keys: `frm1601c:sched1:txtMonthYear0`, `frm1601c:sched1:txtMonthYear1`, `frm1601c:sched1:txtMonthYear2`.
   - Behavior: unmapped in `forms/1601c-v2018.json`, so `form.fill` refuses/drops them and Schedule I Col 1 prints blank when seeded via `form.fill`.

4. **1702RT — `applied` and `not_shown` reported simultaneously for the same key**
   - During `form.fill` pass 2 on `1702RT`, one field was reported in both `applied` and `not_shown` in the same response payload (exact count: 299/316 applied, 17 refused, with 1 overlap between `applied` and `not_shown`).
