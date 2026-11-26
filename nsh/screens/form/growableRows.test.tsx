import { expect, open, test } from "nsh/test";
import { createDemoTaxpayer, warmForms } from "../../bir/testing";

warmForms();
createDemoTaxpayer();

const ENTRY = "./FormPageAny.tsx";

interface Case {
  name: string;
  form: string;
  table: string;
  rows: Record<string, string>;
  key: (row: number) => string;
  text: string;
  rowId: (zeroBasedRow: number) => string;
}

const CASES: Case[] = [
  {
    name: "1601C",
    form: "1601C",
    table: "fp-table-frm1601c:sched1:txtMonthYear0",
    rows: {
      "frm1601c:sched1:txtMonthYear0": "01/2026",
      "frm1601c:sched1:txtMonthYear1": "02/2026",
      "frm1601c:sched1:txtMonthYear2": "03/2026",
    },
    key: (row) => `frm1601c:sched1:txtMonthYear${row}`,
    text: "05/2026",
    rowId: (row) => `fp-row-frm1601c:sched1:txtMonthYear0-${row}`,
  },
  {
    name: "2200A",
    form: "2200A",
    table: "fp-table-frm2200Av2020:txtSched1_xa_others1_Atc",
    rows: {
      "frm2200Av2020:txtSched1_xa_others1_Atc": "001",
      "frm2200Av2020:txtSched1_xa_others2_Atc": "002",
      "frm2200Av2020:txtSched1_xa_others3_Atc": "003",
    },
    key: (row) => `frm2200Av2020:txtSched1_xa_others${row + 1}_Atc`,
    text: "007",
    rowId: (row) => `fp-row-frm2200Av2020:txtSched1_xa_others1_Atc-${row}`,
  },
];

async function scrollUntil(app: any, selector: string) {
  for (let i = 0; i < 40 && !app.shows(selector); i++) await app.scroll("id:form-scroll", 700).catch(() => {});
}

for (const c of CASES) {
  test(`${c.name}: rows 4 to 7 add on the page and row 7 deletes`, async () => {
    const app = await open(ENTRY, {
      width: 390,
      height: 844,
      platform: "ios",
      host: {
        params: {
          formCode: c.form,
          year: 2026,
          period: 1,
          openParts: "all",
          setup: [{ edits: c.rows }],
        },
      },
    });
    await app.waitFor(`text:BIR Form ${c.name}`, { timeout: 15_000 });
    await scrollUntil(app, `id:${c.table}-add`);
    await app.waitFor(`id:${c.table}-add`, { timeout: 15_000 });

    for (let row = 3; row <= 6; row++) {
      await app.press(`id:${c.table}-add`);
      await app.waitFor(`text:Row ${row + 1}`, { timeout: 10_000 });
      await app.type(`id:fp-${c.key(row)}`, c.text);
      await app.press(`id:${c.table}-done`);
      await app.waitFor(`text:Row ${row + 1}`, { state: "gone", timeout: 10_000 });
      await scrollUntil(app, `id:${c.table}-add`);
    }
    expect(app.texts()).toContain("7 rows");

    await app.press(`id:${c.rowId(6)}`);
    await app.waitFor("text:Row 7", { timeout: 10_000 });
    await app.press(`id:${c.table}-delete`);
    await app.waitFor("text:Row 7", { state: "gone", timeout: 10_000 });
    expect(app.texts()).toContain("6 rows");
  });
}
