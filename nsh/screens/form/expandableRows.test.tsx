// F4: the officially expandable tables take rows past the pre-allocated ones on the page.
import { expect, open, test } from "nsh/test";

const ENTRY = "./FormPageAny.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };

test("1601C desktop: a fourth Schedule I row opens once rows 1 to 3 hold entries, and it types", async () => {
  const edits: Record<string, string> = {};
  for (const row of [0, 1, 2]) {
    edits[`frm1601c:sched1:txtMonthYear${row}`] = "01/2025";
    edits[`frm1601c:sched1:txtDatePaid${row}`] = "02/15/2025";
    edits[`frm1601c:sched1:txtBankCode${row}`] = "BDO";
    edits[`frm1601c:sched1:txtNumber${row}`] = "123";
    edits[`frm1601c:sched1:txtTaxPaid${row}`] = "100.00";
  }
  const app = await open(ENTRY, {
    ...DESKTOP,
    host: { params: { formCode: "1601C", year: 2025, period: 1, setup: [{ edits }] } },
  });
  await app.waitFor("text:BIR Form 1601C");
  // Part IV holds Schedule I: rows 1 to 3 hold entries, so "Add row" opens row 4 —
  // the first empty row — in the drawer, past the paper's three rows.
  const table = "fp-table-frm1601c:sched1:txtMonthYear0";
  await app.press("id:fp-part-4-toggle");
  for (let i = 0; i < 40 && !app.shows(`id:${table}-add`); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
  expect(app.count(`id:fp-row-frm1601c:sched1:txtMonthYear0-0`)).toBe(1);
  await app.press(`id:${table}-add`);
  await app.waitFor("text:Row 4", { timeout: 8000 });
  const monthYear3 = "id:fp-frm1601c:sched1:txtMonthYear3";
  await app.type(monthYear3, "03/2025");
  await app.press(`id:${table}-done`);
  await app.waitFor("text:Row 4", { state: "gone", timeout: 8000 });
  await app.waitFor("text:✓ Saved", { timeout: 8000 });
  // The row lists its line — the table does not end at the paper's rows.
  expect(app.count(`id:fp-row-frm1601c:sched1:txtMonthYear0-3`)).toBe(1);
  // And row 5 is the next one Add row opens.
  await app.press(`id:${table}-add`);
  await app.waitFor("text:Row 5", { timeout: 8000 });
  await app.back();
});
