import { expect, open, test } from "nsh/test";

const ENTRY = "../../TaxpayerDemo.tsx";
const ANDROID = { width: 390, height: 844, platform: "android" as const };

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
