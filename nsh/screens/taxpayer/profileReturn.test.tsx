import { expect, open, test } from "nsh/test";

for (const look of [{ platform: "ios" as const }, { width: 1440, height: 900, platform: "macos" as const }]) {
  for (const edit of ["taxpayer", "address", "branch"]) {
    test(`${look.platform}: saving a form-linked ${edit} section returns to the form`, async () => {
      const app = await open("../../TaxpayerProfileRouteDemo.tsx", { ...look, host: { params: { edit, fromForm: true } } });
      await app.waitFor(`id:tp-${edit}-save`);
      await app.press(`id:tp-${edit}-save`);
      await app.waitFor("text:Back on the return");
    });
  }
}

test("an ordinary section edit stays on the profile after save", async () => {
  const app = await open("../../TaxpayerProfileRouteDemo.tsx", { host: { params: { edit: "taxpayer" } } });
  await app.waitFor("id:tp-taxpayer-save");
  await app.press("id:tp-taxpayer-save");
  await app.waitFor("id:tp-sec-taxpayer-edit");
  expect(app.texts()).not.toContain("Back on the return");
});

test("a form-linked section with invalid input stays editable instead of returning", async () => {
  const app = await open("../../TaxpayerProfileRouteDemo.tsx", { host: { params: { edit: "address", fromForm: true } } });
  await app.waitFor("id:tp-address-email");
  await app.clear("id:tp-address-email");
  await app.type("id:tp-address-email", "not-an-email");
  await app.press("id:tp-address-save");
  await app.waitFor("text:Type an email like name@example.com.");
  expect(app.texts()).not.toContain("Back on the return");
});
