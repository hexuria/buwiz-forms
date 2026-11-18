// The menu row (MenuRow.tsx): row 1 of two on Windows and on Linux without a Global Menu, fed from the application menu's data.
// macOS has one row (the system's menu bar is at the top of the screen). Headless: a title's menu is Menu.popup, read with app.menu().
import { expect, open, test } from "nsh/test";

const ENTRY = "../ShellDemo.tsx";
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });

test("Windows and Linux wide: the menu row above the toolbar, titles in order, Windows with its icon", async () => {
  for (const platform of ["windows", "linux"] as const) {
    const app = await open(ENTRY, { width: 1440, height: 900, platform, ...asking() });
    await app.waitFor("id:menu-row");
    expect(app.shows("id:title-bar")).toBe(true); // row 2: the toolbar
    for (const title of ["eBIRForms", "File", "Edit", "View", "Window", "Help", "Profiles"]) expect(app.shows(`id:menu-${title}`)).toBe(true);
    expect(app.shows("id:menu-app-icon")).toBe(platform === "windows");
    expect(app.shows("id:context-pill-tin")).toBe(true);
    expect(app.shows("id:nb-menu")).toBe(false);
  }
});

test("File > New Tax Profile… opens the new taxpayer page (no taxpayer yet: anyone may add the first)", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "linux", ...asking({ demo: false }) });
  await app.waitFor("id:menu-File");
  await app.press("id:menu-File");
  await app.waitUntil(() => app.menu() !== null);
  const items = app.menu() ?? [];
  expect(items.map((e) => e.label).slice(0, 2)).toEqual(["New Tax Profile…", "New Return…"]);
  expect(items[1]?.disabled).toBe(true); // New Return… needs a taxpayer
  await app.chooseMenuItem("New Tax Profile…");
  await app.waitFor("id:new-tp");
});

test("a title opens its menu; Profiles switches the taxpayer; New Tax Profile… follows the panel's rule", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "windows", ...asking({ second: true }) });
  await app.waitFor("id:menu-File");
  await app.press("id:menu-File");
  await app.waitUntil(() => app.menu() !== null);
  // Just me with taxpayers: the profile panel offers no New taxpayer, so the menu's is disabled.
  expect((app.menu() ?? []).find((e) => e.label === "New Tax Profile…")?.disabled).toBe(true);
  await app.closeMenu();

  await app.press("id:menu-Profiles");
  await app.waitUntil(() => app.menu() !== null);
  const items = app.menu() ?? [];
  expect(items.find((e) => e.label === "Juana Santos")?.checked).toBe(true);
  const other = items.find((e) => e.label !== "Juana Santos" && e.label !== "New Tax Profile…");
  if (!other) throw new Error("no second taxpayer in Profiles");
  expect(other.checked).toBe(false);
  await app.chooseMenuItem(other.label);
  await app.waitFor(`text:Switched to ${other.label}`);
});

test("the eBIRForms menu on Windows: About, Settings…, Hide, Sign Out, Quit", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "windows", ...asking() });
  await app.waitFor("id:menu-eBIRForms");
  await app.press("id:menu-eBIRForms");
  await app.waitUntil(() => app.menu() !== null);
  expect((app.menu() ?? []).map((e) => e.label)).toEqual(["About eBIRForms", "Settings…", "Hide eBIRForms", "Sign Out", "Quit eBIRForms"]);
  await app.chooseMenuItem("Settings…");
  await app.waitFor("id:settings");
});

test("tablet width: the same rows, the pill without its TIN", async () => {
  const app = await open(ENTRY, { width: 760, height: 900, platform: "linux", ...asking() });
  await app.waitFor("id:menu-row");
  expect(app.shows("id:title-bar")).toBe(true);
  expect(app.shows("id:context-pill")).toBe(true);
  expect(app.shows("id:context-pill-tin")).toBe(false);
});

test("F10 puts the keyboard on the row, Right moves along it, Down opens the menu", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "windows", ...asking() });
  await app.waitFor("id:menu-row");
  await app.keyboard.press("f10");
  await app.keyboard.press("right");
  await app.keyboard.press("down");
  await app.waitUntil(() => app.menu() !== null);
  expect((app.menu() ?? [])[0]?.label).toBe("New Tax Profile…"); // File, the second title
  await app.closeMenu();
  await app.keyboard.press("escape");
});

test("a keyboard choice lets go of the row: Space and Enter in a text field afterwards open no menu", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "windows", ...asking({ demo: false }) });
  await app.waitFor("id:menu-row");
  // F10, Right to View (eBIRForms File Edit View), Down opens it, then an item is chosen: the menu closes.
  await app.keyboard.press("f10");
  for (let i = 0; i < 3; i++) await app.keyboard.press("right");
  await app.keyboard.press("down");
  await app.waitUntil(() => app.menu() !== null);
  await app.chooseMenuItem("Command Palette…");
  await app.waitFor("id:palette-input");
  expect(app.menu()).toBe(null);
  // The palette's field has the keyboard: its Space and Enter are the field's, never the row's.
  await app.type("id:palette-input", "ui");
  await app.keyboard.press("space");
  await app.wait(100);
  expect(app.menu()).toBe(null);
  await app.keyboard.press("enter");
  await app.wait(100);
  expect(app.menu()).toBe(null);
  // Outside a field too (the palette closed), Down opens no menu: the row's keys went with its menu.
  await app.keyboard.press("escape");
  await app.keyboard.press("down");
  await app.wait(100);
  expect(app.menu()).toBe(null);
});

test("macOS: one row, no menu row (the system's menu bar)", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", ...asking() });
  await app.waitFor("id:title-bar");
  expect(app.shows("id:menu-row")).toBe(false);
});
