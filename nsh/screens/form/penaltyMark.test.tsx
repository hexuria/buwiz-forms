import { expect, open, test } from "nsh/test";

// The page's "Computed from other items" mark sits on exactly the penalty boxes
// whose component the filer never typed. Nothing typed yet: all three of 1701's
// Items 27-29 carry it; a box outside the penalty group carries none.
test("penalty computed marks on 1701", async () => {
  const app = await open("./FormPageAny.tsx", {
    width: 1440, height: 900, platform: "macos", clock: {},
    host: { params: { formCode: "1701", year: 2025, period: 0, openParts: "all" } },
  });
  await app.waitFor("text:BIR Form 1701");
  const box = "id:fp-frm1701:txtPg1I27A";
  for (let i = 0; i < 40 && !app.shows(box); i++) await app.scroll("id:form-scroll", 800);
  expect(app.count(box)).toBe(1);
  expect(app.count("id:fp-mark-lock-frm1701:txtPg1I27A")).toBe(1);
  expect(app.count("id:fp-mark-lock-frm1701:txtPg1I28A")).toBe(1);
  // Rows mount as they scroll into view (the list recycles): Item 29 is below Item 27.
  for (let i = 0; i < 12 && !app.shows("id:fp-frm1701:txtPg1I29A"); i++) await app.scroll("id:form-scroll", 300);
  expect(app.count("id:fp-mark-lock-frm1701:txtPg1I29A")).toBe(1);
  // A non-penalty box carries no mark: Item 14's foreign tax number is in the same part.
  expect(app.count("id:fp-mark-lock-frm1701:txtPg1I14ForeignTaxNumber")).toBe(0);
});

test("penalty computed marks on 1701 phone", async () => {
  const app = await open("./FormPageAny.tsx", {
    host: { params: { formCode: "1701", year: 2025, period: 0, openParts: "all" } },
  });
  await app.waitFor("text:BIR Form 1701");
  const box = "id:fp-frm1701:txtPg1I27A";
  for (let i = 0; i < 40 && !app.shows(box); i++) await app.scroll("id:form-scroll", 800);
  expect(app.count("id:fp-mark-lock-frm1701:txtPg1I27A")).toBe(1);
});

// The live flow: Edit penalties opens the group, the filer types the surcharge
// (Item 28); its mark lifts once the edit's round trip lands, and the interest
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
  expect(app.count("id:fp-mark-lock-frm1701:txtPg1I28A")).toBe(1);
  await app.press("id:fp-penalty-2:6");
  // Clear the prefilled 0.00 first: app.type appends.
  await app.clear(box);
  await app.type(box, "1000");
  await app.waitFor("text:✓ Saved", { timeout: 15_000 }).catch(() => {});
  for (let i = 0; i < 40 && app.count("id:fp-mark-lock-frm1701:txtPg1I28A") !== 0; i++) await app.wait(250);
  expect(app.count("id:fp-mark-lock-frm1701:txtPg1I28A")).toBe(0);
  expect(app.count("id:fp-mark-lock-frm1701:txtPg1I27A")).toBe(1);
  expect(app.count("id:fp-mark-lock-frm1701:txtPg1I29A")).toBe(1);
});
