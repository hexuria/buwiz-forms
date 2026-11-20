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
