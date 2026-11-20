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
    await app.clear("id:tp-branch-business_start_date");
    await app.type("id:tp-branch-business_start_date", "2026-01-01");
    await app.press("id:tp-branch-save");
    await app.waitFor("id:fp-set-profile-income_tax_election");
    expect(app.shows("id:fp-set-profile-business_start_date")).toBe(false);
    await app.press("id:fp-set-profile-income_tax_election");
    await app.waitFor("id:tp-fact-income_tax_election-choice");
    expect(app.texts()).toContain("Save for 2026");
    // No tap on a choice: the linked editor must already have keyboard focus.
    await app.keyboard.press("space");
    await app.press("id:tp-fact-income_tax_election-save");
    await app.waitFor("text:BIR Form 2551Q");
    await app.waitFor("id:fp-set-profile-income_tax_election", { state: "gone" });
    expect(app.shows("id:fp-set-profile-business_start_date")).toBe(false);
  });
}
