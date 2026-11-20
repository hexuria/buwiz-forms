import { expect, open, test } from "nsh/test";

const ENTRY = "../../TaxpayerDemo.tsx";
const ANDROID = { width: 390, height: 844, platform: "android" as const };

for (const platform of ["android", "ios"] as const) {
  test(`${platform} phone: the PIN-protected export card keeps its copy above the controls without squeezing it`, async () => {
    const app = await open(ENTRY, { ...ANDROID, platform, host: { params: { card: "export", pin: true, exportLayout: true } } });
    await app.waitFor("id:tp-export-pin");
    await app.waitFor("id:tp-export-btn");
    await app.waitUntil(() => Number(app.texts().find((text) => text.startsWith("Export card height: "))?.split(": ")[1]) > 0);
    expect(app.texts()).toContain("Export this profile");
    expect(app.texts()).toContain("The archive leaves out credentials; the profile PIN gates saving it");
    const height = Number(app.texts().find((text) => text.startsWith("Export card height: "))!.split(": ")[1]);
    expect(height).toBeGreaterThan(110);
    expect(height).toBeLessThan(210);
    await app.press("id:tp-export-btn");
    await app.waitFor("text:Type all 4 digits of the profile PIN first.");
  });
}

test("desktop: the PIN-protected export card retains its compact horizontal layout", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", host: { params: { card: "export", pin: true, exportLayout: true } } });
  await app.waitFor("id:tp-export-pin");
  await app.waitUntil(() => Number(app.texts().find((text) => text.startsWith("Export card height: "))?.split(": ")[1]) > 0);
  const height = Number(app.texts().find((text) => text.startsWith("Export card height: "))!.split(": ")[1]);
  expect(height).toBeLessThan(110);
  expect(app.enabled("id:tp-export-btn")).toBe(true);
});

test("Android export needs no dialog and reports the actual Downloads name, including collision renaming", async () => {
  const app = await open(ENTRY, { ...ANDROID, host: { params: { card: "export", exportDownload: { name: "BIR_Profile (1).zip" } } } });
  await app.waitFor("id:tp-export-btn");
  let dialogs = 0;
  app.onDialog(() => { dialogs++; return null; });
  await app.press("id:tp-export-btn");
  await app.waitFor("text:Saved to Downloads/eBIRForms/BIR_Profile (1).zip");
  expect(app.texts()).toContain("Export requests: 1");
  expect(dialogs).toBe(0);
  expect(app.enabled("id:tp-export-btn")).toBe(true);
  expect(app.texts()).not.toContain("Profile exported.");
});

test("Android export shows the copy error, not success, and releases the Export button", async () => {
  const app = await open(ENTRY, { ...ANDROID, host: { params: { card: "export", exportDownload: { error: "Downloads copy failed: no space left." } } } });
  await app.waitFor("id:tp-export-btn");
  await app.press("id:tp-export-btn");
  await app.waitFor("text:Downloads copy failed: no space left.");
  expect(app.texts()).toContain("Export requests: 1");
  expect(app.texts()).not.toContain("Profile exported.");
  expect(app.enabled("id:tp-export-btn")).toBe(true);
});

test("an empty or cancelled export response stays quiet", async () => {
  const app = await open(ENTRY, { ...ANDROID, host: { params: { card: "export", exportDownload: { cancelled: true } } } });
  await app.waitFor("id:tp-export-btn");
  await app.press("id:tp-export-btn");
  await app.waitFor("text:Export requests: 1");
  await app.waitUntil(() => app.enabled("id:tp-export-btn"));
  expect(app.texts()).not.toContain("Profile exported.");
  expect(app.texts().some((text) => text.startsWith("Saved to Downloads/"))).toBe(false);
});
