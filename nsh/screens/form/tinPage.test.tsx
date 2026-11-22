// PR 8 (Q8): the TIN row on the page — under 600 pt its boxes are one grid that fills the row (equal columns,
// the last 1.4 only for the 5-digit branch code, 6 pt gaps, no dashes, 44 pt touch boxes, centred tabular digits);
// at 600 pt and wider it is the old fixed 58/58/58/76 boxes with the dashes. A profile TIN says "From the tax
// profile" once under the boxes on both widths; a typed TIN (1801's executor) has no such line and takes typing.
import { expect, open, test, type App } from "nsh/test";
import { bir } from "../../bir";
import { warmForms } from "../../bir/testing";

warmForms();

const ENTRY = "./FormPageAny.tsx";
// 0605 Part I Item 9 "Taxpayer Identification": the profile's TIN, four locked boxes (row "1:0").
const TIN_ROW = "1:0";
const TIN_KEYS = ["frm0605:txtTIN1", "frm0605:txtTIN2", "frm0605:txtTIN3", "frm0605:txtBranchCode"];

/** Parts are collapsible cards (only part 0 opens with the form) and the rows are a virtualized list: open the
 *  part, then scroll until the row's cell is drawn, then it can be read. */
async function showRow(app: App, part: number, row: string) {
  const id = `id:fp-item-${row}`;
  if (part !== 0) await app.press(`id:fp-part-${part}-toggle`).catch(() => {});
  for (let i = 0; i < 40 && !app.shows(id); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
  expect(app.shows(id)).toBe(true);
}

for (const width of [320, 390, 1440]) {
  test(`P8 ${width}: 0605 Item 9's locked TIN is the grid under 600 pt, the dashed boxes at 1440`, async () => {
    const app = await open(ENTRY, {
      width, height: 900, platform: width < 600 ? "ios" : "macos",
      host: { params: { formCode: "0605", year: 2025, period: 1, mode: "return" } },
    });
    await showRow(app, 1, TIN_ROW);
    const narrow = width < 600;
    // The grid draws only on the phone layout; the dashes only on the desktop one.
    expect(app.count(`id:fp-tin-grid-${TIN_ROW}`)).toBe(narrow ? 1 : 0);
    expect(app.count(`id:fp-tin-sep-${TIN_ROW}-1`)).toBe(narrow ? 0 : 1);
    expect(app.count(`id:fp-tin-sep-${TIN_ROW}-2`)).toBe(narrow ? 0 : 1);
    expect(app.count(`id:fp-tin-sep-${TIN_ROW}-3`)).toBe(narrow ? 0 : 1);
    // Four boxes either way, locked from the tax profile: the one line under them, and no second statement
    // as the row's profile mark.
    for (const k of TIN_KEYS) expect(app.count(`id:fp-${k}`)).toBe(1);
    expect(app.count(`id:fp-tin-from-${TIN_ROW}`)).toBe(1);
    expect(app.texts()).toContain("From the tax profile");
    expect(app.count(`id:fp-mark-user-${TIN_ROW}`)).toBe(0);
    // Under 600 pt the item number sits in the label's line, not in a column of its own, so the boxes start at
    // the number's left edge and have the row's whole width; at 1440 the number keeps its column.
    expect(app.count(`id:fp-no-${TIN_ROW}`)).toBe(1);
    expect(app.count(`id:fp-head-${TIN_ROW} >> id:fp-no-${TIN_ROW}`)).toBe(narrow ? 1 : 0);
  });
}

test("P8: an editable other-party TIN keeps the row's profile mark and draws no line", async () => {
  // 1706 Part I Item 7's buyer TIN is a TIN item the profile does not lock: no "From the tax profile" line on
  // its row (PR 8's line is for the locked profile TIN only).
  const app = await open(ENTRY, {
    width: 390, height: 900, platform: "ios",
    host: { params: { formCode: "1706", year: 2025, period: 0, mode: "return" } },
  });
  await showRow(app, 1, "1:6");
  for (const k of ["frm1706:txtTINB1", "frm1706:txtTINB2", "frm1706:txtTINB3", "frm1706:txtBranchCodeB"]) {
    expect(app.count(`id:fp-${k}`)).toBe(1);
  }
  expect(app.count("id:fp-tin-from-1:6")).toBe(0);
});

for (const width of [390, 1440]) {
  test(`P8 ${width}: 1801's typed executor TIN takes digits per box and has no profile line`, async () => {
    const app = await open(ENTRY, {
      width, height: 900, platform: width < 600 ? "ios" : "macos",
      host: { params: { formCode: "1801", year: 0, period: 0, mode: "sample" } },
    });
    await showRow(app, 1, "1:6");
    const keys = ["frm1801v2018:txtTINE1", "frm1801v2018:txtTINE2", "frm1801v2018:txtTINE3", "frm1801v2018:txtBranchCodeE"];
    for (const k of keys) expect(app.count(`id:fp-${k}`)).toBe(1);
    // Typed TIN: its own row (part 1, item 6) draws no under-box profile line.
    expect(app.count("id:fp-tin-from-1:6")).toBe(0);
    // Digits only, at most the box's max: letters and dashes never land.
    await app.clear("id:fp-frm1801v2018:txtTINE1");
    await app.type("id:fp-frm1801v2018:txtTINE1", "12a-3");
    expect(app.value("id:fp-frm1801v2018:txtTINE1")).toBe("123");
    await app.clear("id:fp-frm1801v2018:txtBranchCodeE");
    await app.type("id:fp-frm1801v2018:txtBranchCodeE", "0000-1x9");
    expect(app.value("id:fp-frm1801v2018:txtBranchCodeE")).toBe("00001");
    // No auto-advance: the next box stays empty until it is typed in.
    expect(app.value("id:fp-frm1801v2018:txtTINE2")).toBe("");
    // A paste of dashes, spaces and letters lands as digits only, at most the box's max.
    await app.clear("id:fp-frm1801v2018:txtBranchCodeE");
    await app.paste("id:fp-frm1801v2018:txtBranchCodeE", { text: "ab 00-00 1x" });
    expect(app.value("id:fp-frm1801v2018:txtBranchCodeE")).toBe("00001");
  });
}

test("P8: a TIN that is not four boxes keeps the same rules with its own column count", async () => {
  // 1601C Part I Item 6's TIN is four boxes whose branch code takes 3 digits, not 5: equal columns, no 1.4 (the
  // weights are unit-tested in tinShapes.test.ts; here the row draws its four cells and no dashes on the phone).
  const app = await open(ENTRY, {
    width: 390, height: 900, platform: "ios",
    host: { params: { formCode: "1601C", year: 2025, period: 3, mode: "return" } },
  });
  await showRow(app, 1, "1:0");
  for (const k of ["frm1601c:txtTIN1", "frm1601c:txtTIN2", "frm1601c:txtTIN3", "frm1601c:txtBranchCode"]) {
    expect(app.count(`id:fp-${k}`)).toBe(1);
  }
  expect(app.count("id:fp-tin-grid-1:0")).toBe(1);
  expect(app.count("id:fp-tin-sep-1:0-1")).toBe(0);
});

test("P8 390: 1700 Schedule 1's row drawer draws the employer TIN as one grid line and Taxpayer / Spouse as tick boxes", async () => {
  const app = await open(ENTRY, {
    width: 390, height: 844, platform: "ios",
    host: { params: { formCode: "1700", year: 2025, period: 1, mode: "return" } },
  });
  const table = "fp-table-frm1700:rdoPg2I1PartVIEmployeeT";
  // Part VI (index 5) is a collapsed card: open it and bring the schedule's "Add row" into view.
  await app.press("id:fp-part-5-toggle");
  for (let i = 0; i < 40 && !app.shows(`id:${table}-add`); i++) await app.scroll("id:form-scroll", 600).catch(() => {});
  await app.press(`id:${table}-add`);
  await app.waitFor("text:Row 1");
  // Columns 4 to 7 (TIN 1-3, 4-6, 7-9, branch code) are one group with one title; no box of its own line.
  expect(app.count(`id:${table}-tin-4`)).toBe(1);
  for (const c of [4, 5, 6, 7]) expect(app.count(`id:${table}-tin-4 >> id:${table}-cell-${c}`)).toBe(1);
  expect(app.texts()).toContain("b. TIN");
  expect(app.texts()).not.toContain("TIN (4-6)");
  // The radio columns are tick boxes with their words, not typing boxes that read "false".
  expect(app.texts()).toContain("Taxpayer");
  expect(app.texts()).toContain("Spouse");
  expect(app.texts()).not.toContain("false");
  await app.press(`id:${table}-done`);
  await app.waitFor("text:Row 1", { state: "gone" });
});

test("1700 Schedule 1: ticking Taxpayer in BETA's drawer (4_2) changes BETA only; ACME in 4_3 stays Spouse with its amounts", async () => {
  const schema = bir.screen_form_schema("1700") as {
    parts: {
      items: {
        no?: string;
        f?: { k: string; l: string; t: string }[];
        rows?: number;
        cols?: { t: string }[];
        keys?: string[][];
      }[];
    }[];
  };
  const items = schema.parts.flatMap((part, partIndex) =>
    part.items.map((item, itemIndex) => ({ item, partIndex, itemIndex })),
  );
  const choice = (no: string, label: RegExp) =>
    schema.parts[1].items.find((item) => item.no === no)!.f!.find((field) => field.t === "radio" && label.test(field.l))!.k;
  const schedules = items.filter(({ item }) =>
    item.rows && (item.cols || []).filter((column) => column.t === "radio").length === 2,
  );
  const page = schedules.find(({ item }) => item.rows === 4)!.item;
  const more = schedules.find(({ item }) => item.rows === 12)!;
  const setup: ({ pick: string } | { edits: Record<string, string> })[] = [
    // Schedule 1 column c (regular rates) stays closed at 0.00 until a filer is an employee.
    { pick: "frm1700:rdoPg1I6TaxpayerTypeE" },
    { pick: choice("15", /married/i) },
    { pick: choice("16", /^yes$/i) },
    { pick: choice("17", /joint filing/i) },
  ];
  for (let row = 0; row < 4; row++) {
    const keys = page.keys![row];
    // The official order: the owner tick and name first, then the amounts.
    setup.push({ edits: { [keys[0]]: "true", [keys[2]]: `Employer ${row + 1}` } });
    setup.push({ edits: { [keys[8]]: String(1000 + row), [keys[10]]: String(100 + row) } });
  }
  const betaKeys = more.item.keys![1];
  const acmeKeys = more.item.keys![2];
  setup.push({ edits: { [betaKeys[1]]: "true", [betaKeys[2]]: "BETA" } });
  setup.push({ edits: { [betaKeys[8]]: "500", [betaKeys[10]]: "50" } });
  setup.push({ edits: { [acmeKeys[1]]: "true", [acmeKeys[2]]: "ACME" } });
  setup.push({ edits: { [acmeKeys[8]]: "10000", [acmeKeys[10]]: "1000" } });

  const app = await open(ENTRY, {
    width: 1440, height: 900, platform: "macos",
    host: { params: { formCode: "1700", year: 2025, period: 1, mode: "return", setup } },
  });
  const row = `${more.partIndex}:${more.itemIndex}`;
  await showRow(app, more.partIndex, row);
  const table = `id:fp-table-${more.item.keys![0][0]}`;
  const rowId = table.replace(/^id:fp-table-/, "id:fp-row-");
  const betaRow = `${rowId}-1`;
  const acmeRow = `${rowId}-2`;
  // The fixture's setup writes one step per host fill; the slowest runner needs well over the default 5 s.
  // Both amounts landed: compensation, then withholding, the whole summary.
  const acmeSummary = /^Spouse · ACME · 10,?000(\.00)? · 1,?000(\.00)?$/;
  await app.waitUntil(() => app.texts().some((t) => acmeSummary.test(t)), { timeout: 25000 });
  await app.waitFor("label:/BETA.*Spouse|Spouse.*BETA/");
  await app.waitFor("label:/ACME.*Spouse|Spouse.*ACME/");

  const compensation = `id:fp-${acmeKeys[8]}`;
  const withholding = `id:fp-${acmeKeys[10]}`;
  await app.press(acmeRow);
  await app.waitFor(compensation);
  const compensationBefore = app.value(compensation);
  const withholdingBefore = app.value(withholding);
  expect(compensationBefore).toBe("10,000.00");
  expect(withholdingBefore).toBe("1,000.00");
  await app.press(`${table}-done`);
  await app.waitFor(compensation, { state: "gone" });

  // The drawer's visible Taxpayer tick writes BETA's own row.
  const taxpayer = `id:fp-${betaKeys[0]}`;
  await app.press(betaRow);
  await app.waitFor(taxpayer);
  await app.press(taxpayer);
  await app.press(`${table}-done`);
  await app.waitFor(taxpayer, { state: "gone" });
  await app.waitFor("label:/BETA.*Taxpayer|Taxpayer.*BETA/");
  await app.waitFor("label:/ACME.*Spouse|Spouse.*ACME/");

  await app.press(acmeRow);
  await app.waitFor(compensation);
  expect(app.value(compensation)).toBe(compensationBefore);
  expect(app.value(withholding)).toBe(withholdingBefore);
}, 60000);
