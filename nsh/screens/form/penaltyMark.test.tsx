import { expect, open, test } from "nsh/test";

// The page's "Computed from other items" mark sits on exactly the open penalty boxes
// whose component the filer never typed. A shut box carries no mark: its own lock and
// its reason already say the amounts are computed. An ordinary box is never wrapped.
test("no penalty mark on a shut box on 1701", async () => {
  const app = await open("./FormPageAny.tsx", {
    width: 1440, height: 900, platform: "macos", clock: {},
    host: { params: { formCode: "1701", year: 2025, period: 0, openParts: "all" } },
  });
  await app.waitFor("text:BIR Form 1701");
  const box = "id:fp-frm1701:txtPg1I27A";
  for (let i = 0; i < 40 && !app.shows(box); i++) await app.scroll("id:form-scroll", 800);
  expect(app.count(box)).toBe(1);
  // Untouched, the group is locked: the boxes are shut, and no mark and no wrapper show.
  for (const k of ["frm1701:txtPg1I27A", "frm1701:txtPg1I28A", "frm1701:txtPg1I29A"]) {
    expect(app.count(`id:fp-mark-lock-${k}`)).toBe(0);
    expect(app.count(`id:fp-mark-info-${k}`)).toBe(0);
    expect(app.count(`id:fp-penrow-${k}`)).toBe(0);
  }
  // Rows mount as they scroll into view (the list recycles): Item 29 is below Item 27.
  for (let i = 0; i < 12 && !app.shows("id:fp-frm1701:txtPg1I29A"); i++) await app.scroll("id:form-scroll", 300);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I29A")).toBe(0);
  // A non-penalty box is never wrapped: Item 14's foreign tax number is in the same part.
  expect(app.count("id:fp-penrow-frm1701:txtPg1I14ForeignTaxNumber")).toBe(0);
});

test("no penalty mark on a shut box on 1701 phone", async () => {
  const app = await open("./FormPageAny.tsx", {
    host: { params: { formCode: "1701", year: 2025, period: 0, openParts: "all" } },
  });
  await app.waitFor("text:BIR Form 1701");
  const box = "id:fp-frm1701:txtPg1I27A";
  for (let i = 0; i < 40 && !app.shows(box); i++) await app.scroll("id:form-scroll", 800);
  expect(app.count(box)).toBe(1);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I27A")).toBe(0);
  expect(app.count("id:fp-penrow-frm1701:txtPg1I27A")).toBe(0);
  // An ordinary phone box keeps the row's full width: no wrapper on Item 14's foreign tax number.
  expect(app.count("id:fp-penrow-frm1701:txtPg1I14ForeignTaxNumber")).toBe(0);
});

// The live flow: Edit penalties opens the group — its untyped boxes carry the mark with
// the info sign, never a lock beside a box the filer can type in. The filer types the
// surcharge (Item 28); its mark lifts once the edit's round trip lands, and the interest
// (27) and compromise (29) boxes keep theirs.
test("penalty mark lifts on the typed component", async () => {
  const app = await open("./FormPageAny.tsx", {
    width: 1440, height: 900, platform: "macos", clock: {},
    host: { params: { formCode: "1701", year: 2025, period: 0, openParts: "all" } },
  });
  await app.waitFor("text:BIR Form 1701");
  const box = "id:fp-frm1701:txtPg1I28A";
  for (let i = 0; i < 40 && !app.shows(box); i++) await app.scroll("id:form-scroll", 800);
  expect(app.count(box)).toBe(1);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I28A")).toBe(0);
  await app.press("id:fp-penalty-2:6");
  // Open and untyped: the wrapper and the info mark show; never a lock icon.
  for (let i = 0; i < 40 && app.count("id:fp-mark-info-frm1701:txtPg1I28A") !== 1; i++) await app.wait(250);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I28A")).toBe(1);
  expect(app.count("id:fp-penrow-frm1701:txtPg1I28A")).toBe(1);
  expect(app.count("id:fp-mark-lock-frm1701:txtPg1I28A")).toBe(0);
  // Clear the prefilled 0.00 first: app.type appends.
  await app.clear(box);
  await app.type(box, "1000");
  await app.waitFor("text:✓ Saved", { timeout: 15_000 }).catch(() => {});
  for (let i = 0; i < 40 && app.count("id:fp-mark-info-frm1701:txtPg1I28A") !== 0; i++) await app.wait(250);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I28A")).toBe(0);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I27A")).toBe(1);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I29A")).toBe(1);
  // The typed box keeps its wrapper (the mark lifting never remounts the box mid-edit).
  expect(app.count("id:fp-penrow-frm1701:txtPg1I28A")).toBe(1);
});
