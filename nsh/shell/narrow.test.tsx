// The narrow layout: under 600 points (a phone, or a narrow desktop window): no sidebar, a bottom tab bar, the avatar's sheet.
import { expect, open, test } from "nsh/test";

const ENTRY = "../ShellDemo.tsx";
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });

test("phone: four tabs, no sidebar, the header names the page and back leaves a pushed one", async () => {
  const app = await open(ENTRY, asking()); // 390 x 844, iOS
  await app.waitFor("id:tab-bar");
  expect(app.shows("id:sidebar")).toBe(false);
  expect(app.shows("id:title-bar")).toBe(false);
  for (const label of ["Home", "Forms", "Filings", "Calendar"]) expect(app.texts()).toContain(label);
  expect(app.shows("id:nb-back")).toBe(false);

  await app.press("id:tab-library");
  await app.waitFor("id:library");
  expect(app.texts()).toContain("Form library");
  await app.press("id:tab-filings");
  await app.waitFor("id:filings");
  await app.press("id:tab-calendar");
  await app.waitFor("id:calendar-narrow");
  await app.press("id:tab-home");
  await app.waitFor("id:home");
  expect(app.shows("id:nb-back")).toBe(false); // a tab is a root

  // The avatar opens a sheet: the taxpayer, the profile links, Settings, the theme.
  await app.press("id:nb-avatar");
  await app.waitFor("id:sheet-extras");
  expect(app.texts()).toContain("Juana Santos");
  expect(app.texts()).toContain("Forms");
  expect(app.texts()).toContain("Settings");
  expect(app.texts()).toContain("Theme: system");
  await app.press("id:sheet-theme");
  await app.waitFor("text:Theme: light");
  await app.press("id:sheet-settings");
  await app.waitFor("id:settings");
  await app.waitFor("id:sheet-extras", { state: "gone" }); // moving closes it (the sheet slides away, so not at the same instant)
  expect(app.shows("id:sheet-extras")).toBe(false);
  expect(app.shows("id:nb-back")).toBe(true);
  await app.press("id:nb-back");
  await app.waitFor("id:home");
});

test("phone: the bell opens the notifications sheet", async () => {
  const app = await open(ENTRY, asking());
  await app.waitFor("id:nb-notifications");
  await app.press("id:nb-notifications");
  await app.waitFor("id:notifications-list");
  expect(app.texts()).toContain("No notifications yet.");
  expect(app.enabled("id:notif-mark-all")).toBe(false);
  await app.back();
  await app.waitFor("id:notifications-list", { state: "gone" });
});

test("phone: with no taxpayer the avatar is an account button, and the sheet offers one", async () => {
  const app = await open(ENTRY, asking({ demo: false }));
  await app.waitFor("id:nb-avatar");
  await app.press("id:nb-avatar");
  await app.waitFor("id:profile-empty");
  expect(app.texts()).toContain("No taxpayer yet");
  await app.press("id:panel-new-taxpayer");
  await app.waitFor("id:new-tp");
});

test("a narrow desktop window takes the narrow layout, and widening brings the sidebar back", async () => {
  const app = await open(ENTRY, { width: 520, height: 800, platform: "macos", ...asking() });
  await app.waitFor("id:tab-bar");
  expect(app.shows("id:sidebar")).toBe(false);
  expect(app.shows("id:tb-sidebar")).toBe(false);
  await app.press("id:tab-library");
  await app.waitFor("id:library");
  await app.setViewport(1440, 900);
  await app.waitFor("id:sidebar");
  expect(app.shows("id:tab-bar")).toBe(false);
  expect(app.shows("id:library")).toBe(true); // the page stays where it was
});
