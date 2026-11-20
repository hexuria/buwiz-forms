// PR 8 (Q8): the TIN row on the page — under 600 pt its boxes are one grid that fills the row (equal columns,
// the last 1.4 only for the 5-digit branch code, 6 pt gaps, no dashes, 44 pt touch boxes, centred tabular digits);
// at 600 pt and wider it is the old fixed 58/58/58/76 boxes with the dashes. A profile TIN says "From the tax
// profile" once under the boxes on both widths; a typed TIN (1801's executor) has no such line and takes typing.
import { expect, open, test, type App } from "nsh/test";
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
