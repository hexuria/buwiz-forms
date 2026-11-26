import { expect, open, test } from "nsh/test";

for (const look of [{}, { width: 1440, height: 900, platform: "macos" as const }]) {
  test(`2551Q opening shows profile prerequisite notices before Queue (${Object.keys(look).length ? "desktop" : "phone"})`, async () => {
    const app = await open("./FormPageAny.tsx", { ...look, host: { params: { formCode: "2551Q", year: 2025, period: 3 } } });
    await app.waitFor("id:fp-profile-prerequisite-business_start_date");
    expect(app.shows("id:fp-profile-prerequisite-income_tax_election")).toBe(true);
    expect(app.shows("id:fp-set-profile-business_start_date")).toBe(true);
    expect(app.shows("id:fp-set-profile-income_tax_election")).toBe(true);
    expect(app.texts().join(" ")).toContain("annual income-tax election for 2025");
  });
}

for (const look of [{ platform: "android" as const }, { width: 1440, height: 900, platform: "macos" as const }]) {
  test(`a Q2 2027 notice edits and saves 2027, not the selected 2026 profile year (${look.platform})`, async () => {
    const app = await open("../../TaxpayerDemo.tsx", {
      ...look,
      host: { params: {
        start: "form=2551Q/2027/2", year: 2026, selfEmployed: true,
        elections: { 2026: "GraduatedUnspecified", 2027: null },
        savedReturn: { formCode: "2551Q", year: 2027, period: 2 },
      } },
    });
    await app.waitFor("id:fp-set-profile-income_tax_election");
    expect(app.texts().join(" ")).toContain("annual income-tax election for 2027");
    await app.press("id:fp-set-profile-income_tax_election");
    await app.waitFor("id:tp-fact-income_tax_election-choice");
    expect(app.texts()).toContain("Save for 2027");
    expect(app.texts()).not.toContain("Save for 2026");
    await app.press("id:tp-fact-income_tax_election-save");
    await app.waitFor("text:Choose one first.");
    await app.press("id:tp-fact-income_tax_election-choice >> label:8%");
    await app.press("id:tp-fact-income_tax_election-save");
    await app.waitFor("text:BIR Form 2551Q");
    await app.waitFor("id:fp-set-profile-income_tax_election", { state: "gone" });
    expect(app.texts()).toContain("Election 2027: EightPercent");
    expect(app.texts()).toContain("Election 2026: GraduatedUnspecified");
  });
}

for (const look of [{}, { width: 1440, height: 900, platform: "macos" as const }]) {
  test(`profile links save the return's 2026 facts and Back removes the notices (${Object.keys(look).length ? "desktop" : "phone"})`, async () => {
    const app = await open("../../TaxpayerDemo.tsx", {
      ...look,
      host: { params: {
        start: "form=2551Q/2026/3", year: 2026, selfEmployed: true,
        savedReturn: { formCode: "2551Q", year: 2026, period: 3 },
      } },
    });
    await app.waitFor("id:fp-set-profile-business_start_date");
    expect(app.texts().join(" ")).toContain("annual income-tax election for 2026");
    await app.press("id:fp-set-profile-business_start_date");
    await app.waitFor("id:tp-branch-business_start_date");
    if (Object.keys(look).length) {
      // Desktop: the form's date box, with the keys in it.
      await app.clear("id:tp-branch-business_start_date");
      await app.type("id:tp-branch-business_start_date", "01/01/2026");
    } else {
      // Phone: the link opens the date's sheet by itself (DateTouchField autoOpen); the date is typed there and taken on Done.
      await app.waitFor("id:tp-branch-business_start_date-sheet-input");
      await app.clear("id:tp-branch-business_start_date-sheet-input");
      await app.type("id:tp-branch-business_start_date-sheet-input", "01/01/2026");
      await app.press("id:tp-branch-business_start_date-sheet-done");
      await app.waitFor("id:tp-branch-business_start_date-sheet-input", { state: "gone" });
    }
    await app.press("id:tp-branch-save");
    await app.waitFor("id:fp-set-profile-income_tax_election");
    expect(app.shows("id:fp-set-profile-business_start_date")).toBe(false);
    await app.press("id:fp-set-profile-income_tax_election");
    await app.waitFor("id:tp-fact-income_tax_election-choice");
    expect(app.texts()).toContain("Save for 2026");
    if (Object.keys(look).length) {
      // Desktop controls accept keyboard focus; phones choose by touch.
      await app.keyboard.press("space");
    } else {
      await app.press("id:tp-fact-income_tax_election-choice >> label:8%");
    }
    await app.press("id:tp-fact-income_tax_election-save");
    await app.waitFor("text:BIR Form 2551Q");
    await app.waitFor("id:fp-set-profile-income_tax_election", { state: "gone" });
    expect(app.shows("id:fp-set-profile-business_start_date")).toBe(false);
  });
}

test("a narrow window: the link opens the business start date's sheet once; the row drawn again in the same edit does not reopen it", async () => {
  // A desktop window under 600 points has the phone layout, and a resize past 600 and back draws the date's row anew in the same
  // edit (wide: the inline DateField; narrow again: the edit sheet's DateTouchField), as a rotation between layouts does.
  const app = await open("../../TaxpayerDemo.tsx", {
    width: 390,
    height: 844,
    platform: "macos",
    host: { params: {
      start: "form=2551Q/2026/3", year: 2026, selfEmployed: true,
      savedReturn: { formCode: "2551Q", year: 2026, period: 3 },
    } },
  });
  const dateSheet = "id:tp-branch-business_start_date-sheet-input";
  await app.waitFor("id:fp-set-profile-business_start_date");
  await app.press("id:fp-set-profile-business_start_date");
  // Opened by itself, once (DateTouchField autoOpen), and closed by the filer.
  await app.waitFor(dateSheet);
  await app.back();
  await app.waitFor(dateSheet, { state: "gone" });
  expect(app.shows("id:tp-edit-sheet-body")).toBe(true);
  // Wide: the edit goes inline (the edit sheet and its row go). Narrow again: the edit sheet and the row are drawn anew.
  await app.setViewport(1440, 900);
  await app.waitFor("id:tp-edit-sheet-body", { state: "gone" });
  await app.setViewport(390, 844);
  await app.waitFor("id:tp-edit-sheet-body");
  await app.waitFor("id:tp-branch-business_start_date");
  // Past autoOpen's wait (350 ms): the sheet stays shut, because the edit no longer names the field (page.ts focusTaken).
  await app.wait(700);
  expect(app.shows(dateSheet)).toBe(false);
});
