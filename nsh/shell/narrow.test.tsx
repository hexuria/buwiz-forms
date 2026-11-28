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

  // The header has New return (+) and Search; a phone has no room for the taxpayer's name (a tablet has).
  expect(app.shows("id:nb-new")).toBe(true);
  expect(app.shows("id:nb-search")).toBe(true);
  expect(app.shows("id:nb-taxpayer")).toBe(false);
  await app.press("id:nb-search");
  await app.waitFor("id:palette-input");
  await app.back();
  await app.waitFor("id:palette-input", { state: "gone" });
  // The badge's menu: the notifications, Settings, Lock app and Log out — no Search (the header's own button) and no tax
  // profile (search is the tax-profile picker, and opens it as a page).
  expect(app.shows("id:nb-notifications")).toBe(false);
  await app.press("id:nb-avatar");
  await app.waitFor("id:nb-menu-settings");
  expect(app.shows("id:nb-menu-notifications")).toBe(true);
  expect(app.shows("id:nb-menu-lock")).toBe(true);
  expect(app.shows("id:nb-menu-logout")).toBe(true);
  expect(app.shows("id:nb-menu-search")).toBe(false);
  expect(app.shows("id:nb-menu-profile")).toBe(false);
  await app.press("id:nb-menu-settings");
  await app.waitFor("id:settings");
  expect(app.shows("id:nb-back")).toBe(true);
  await app.press("id:nb-back");
  await app.waitFor("id:home");
});

test("tablet: the tax profile sheet opens from the header's taxpayer label, with Settings and the theme at its end", async () => {
  // A tablet has room for the taxpayer's name in the header; a phone reaches the tax profile as a page from search (palette.test).
  const app = await open(ENTRY, { width: 1376, height: 1032, platform: "ios", ...asking() });
  await app.waitFor("id:nb-taxpayer");
  await app.press("id:nb-taxpayer");
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
});

test("phone: the badge's menu opens the notifications sheet", async () => {
  const app = await open(ENTRY, asking());
  await app.waitFor("id:nb-avatar");
  expect(app.shows("id:nb-unread-dot")).toBe(false); // nothing unread yet
  await app.press("id:nb-avatar");
  await app.waitFor("id:nb-menu-notifications");
  expect(app.texts()).toContain("Settings");
  expect(app.texts()).toContain("Lock app");
  expect(app.texts()).toContain("Log out");
  await app.press("id:nb-menu-notifications");
  await app.waitFor("id:notifications-list");
  expect(app.texts()).toContain("No notifications yet.");
  expect(app.enabled("id:notif-mark-all")).toBe(false);
  await app.back();
  await app.waitFor("id:notifications-list", { state: "gone" });
});

test("phone: a dry-run send's notification says it was a test send; a real send's says it was submitted", async () => {
  const app = await open(ENTRY, asking({ filed: true }));
  await app.waitFor("id:nb-avatar");
  await app.press("id:nb-avatar");
  await app.waitFor("id:nb-menu-notifications");
  await app.press("id:nb-menu-notifications");
  await app.waitFor("id:notifications-list");
  await app.waitFor("text:2551Q for Q2 2025: Test send: not filed. Nothing reached the BIR.");
  expect(app.texts()).not.toContain("2551Q for Q2 2025: dry run finished");
  expect(app.texts()).not.toContain("2551Q for Q2 2025 was submitted");
  expect(app.texts()).toContain("2551Q for Q1 2025 was submitted");
  expect(app.texts()).toContain("Waiting for BIR's confirmation email · Juana Santos");
});

test("phone: with no taxpayer the badge's menu still opens, and Home offers the first one", async () => {
  const app = await open(ENTRY, asking({ demo: false }));
  await app.waitFor("id:home-welcome");
  // The badge's menu opens as it does with a taxpayer, and never names a tax profile or a search.
  await app.press("id:nb-avatar");
  await app.waitFor("id:nb-menu-settings");
  expect(app.shows("id:nb-menu-profile")).toBe(false);
  expect(app.shows("id:nb-menu-search")).toBe(false);
  await app.back();
  await app.waitFor("id:nb-menu-settings", { state: "gone" });
  // The first taxpayer starts at Home's welcome (the empty panel's offer is the desktop's, frame.test).
  await app.press("id:welcome-add-taxpayer");
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

test("the badge's menu reaches Settings, and the header's search the palette", async () => {
  const app = await open(ENTRY, { width: 340, height: 760, ...asking() });
  await app.waitFor("id:nb-avatar");
  await app.press("id:nb-avatar");
  await app.waitFor("id:nb-menu-settings");
  await app.press("id:nb-menu-settings");
  await app.waitFor("id:settings");
  await app.press("id:nb-search");
  await app.waitFor("id:palette");
});

// The app menu at phone width (chrome_layout.ts): Windows and Linux windows whose menu is drawn in the window get ☰ before the title,
// which lists every menu; the badge and its menu stay as they are. macOS keeps its menu at the top of the screen.
test("a narrow Windows or Linux window: ☰ before the title lists every menu; the badge stays the header's account button", async () => {
  for (const platform of ["windows", "linux"] as const) {
    const app = await open(ENTRY, { width: 520, height: 800, platform, ...asking({ second: true }) });
    await app.waitFor("id:nb-menu");
    expect(app.shows("id:menu-row")).toBe(false);
    expect(app.shows("id:nb-avatar")).toBe(true);
    // One narrow header everywhere: New return and Search beside the badge.
    expect(app.shows("id:nb-search")).toBe(true);
    await app.press("id:nb-menu");
    await app.waitUntil(() => app.menu() !== null);
    const labels = (app.menu() ?? []).map((e) => e.label);
    for (const path of ["eBIRForms > About eBIRForms", "File > New tax profile…", "File > New Return…", "Edit > " /* roles: the platform names them */, "View > Toggle Sidebar", "Help > Contact Support…", "Profiles > New tax profile…"]) {
      expect(labels.some((l) => l.startsWith(path))).toBe(true);
    }
    expect((app.menu() ?? []).find((e) => e.label === "Profiles > Juana Santos")?.checked).toBe(true);
    await app.chooseMenuItem("eBIRForms > Settings…");
    await app.waitFor("id:settings");
  }
});

test("a narrow macOS window has no ☰: the menu stays in the menu bar at the top of the screen", async () => {
  const app = await open(ENTRY, { width: 520, height: 800, platform: "macos", ...asking() });
  await app.waitFor("id:nb-avatar");
  expect(app.shows("id:nb-menu")).toBe(false);
});
