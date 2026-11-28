// First launch on an empty database, at phone size and at desktop size: who uses this computer, "Just me" (New taxpayer with
// no chrome, Cancel back to the choice, Create opens the app) and "I help other taxpayers" (the admin PIN, the recovery key
// shown once, then the admin's empty Home).
import { expect, open, test } from "nsh/test";

const ENTRY = "../../AuthDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };

test("phone: Just me types the first taxpayer with no chrome; Cancel goes back; Create opens the app", async () => {
  const app = await open(ENTRY, { host: { params: { readout: true } } });
  await app.waitFor("id:setup-card");
  expect(app.texts()).toContain("Who will use eBIRForms on this device?");
  // Nothing of the frame: no tab bar, no header, no sidebar.
  expect(app.shows("id:frame")).toBe(false);
  await app.press("id:setup-solo");
  await app.waitFor("id:new-tp-card");
  expect(app.shows("id:frame")).toBe(false);
  await app.press("id:new-tp-cancel");
  await app.waitFor("id:setup-card");
  await app.press("id:setup-solo");
  await app.waitFor("id:new-tp-card");
  await app.type("id:new-tp-tin", "000123453");
  await app.type("id:new-tp-name", "Juana Santos");
  await app.press("id:new-tp-rdo_code-trigger");
  await app.type("id:new-tp-rdo_code-search", "039");
  await app.press("id:new-tp-rdo_code-opt-039");
  await app.waitFor("id:new-tp-rdo_code-opt-039", { state: "gone" });
  await app.press("id:new-tp-zip_code-trigger");
  await app.type("id:new-tp-zip_code-search", "1100");
  await app.press("id:new-tp-zip_code-opt-1100 >> nth=0");
  await app.press("id:new-tp-save");
  await app.waitFor("id:frame");
  await app.waitFor("id:tp-page-forms");
  // Just me: the owner is the admin and there is no code yet, so nothing locks.
  expect(app.texts()).toContain("session: open");
});

test("desktop: I help other taxpayers asks for 4 digits, shows the recovery key once, then signs the admin in", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { readout: true } } });
  await app.waitFor("id:setup-card");
  expect(app.texts()).toContain("Who will use eBIRForms on this computer?");
  await app.press("id:setup-helpdesk");
  await app.waitFor("id:setup-pin-card");
  expect(app.texts()).toContain("Choose the admin PIN");
  await app.type("id:setup-admin-pin", "12");
  await app.press("id:setup-pin-submit");
  await app.waitFor("text:Type all 4 digits of the admin PIN.");
  await app.press("id:setup-back");
  await app.waitFor("id:setup-card");
  await app.press("id:setup-helpdesk");
  await app.waitFor("id:setup-pin-card");
  expect(app.texts()).not.toContain("Type all 4 digits of the admin PIN.");
  await app.type("id:setup-admin-pin", "1234");
  await app.press("id:setup-pin-submit");
  await app.waitFor("id:setup-key-card");
  expect(app.texts()).toContain("Write down your recovery key");
  // The key is shown here, and the frame stays hidden until it was written down.
  expect(app.texts().some((t) => /^[0-9A-Z]{4}(-[0-9A-Z]{4}){3,}$/.test(t))).toBe(true);
  expect(app.shows("id:frame")).toBe(false);
  await app.press("id:setup-key-done");
  await app.waitFor("id:frame");
  await app.waitFor("text:session: admin");
  expect(app.texts()).toContain("Help desk admin");
  expect(app.texts()).toContain("TIN 000-000-000");
});

test("desktop: Just me with a taxpayer already there makes it the owner at once", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { demo: true, readout: true } } });
  await app.waitFor("id:setup-card");
  await app.press("id:setup-solo");
  await app.waitFor("id:frame");
  expect(app.texts()).toContain("Juana Santos");
  expect(app.texts()).toContain("Admin · this computer");
});
