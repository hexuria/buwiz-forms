// Tablets (the owner's pick, 2026-11-25): an iPad and an Android tablet take the phone layout at every width and in both
// orientations. A 13-inch iPad upright (1032 x 1376) and sideways (1376 x 1032) with its status bar (24) and home indicator (20)
// as the safe area, and a 10-inch Android tablet (800 x 1280), against a scratch database with the demo taxpayer (ShellDemo.tsx).
// Where things sit in points this harness does not report (viewStyle has no layout); the Simulator pictures of the change show it.
import { expect, open, test } from "nsh/test";

const ENTRY = "../ShellDemo.tsx";
const SAFE = { top: 24, right: 0, bottom: 20, left: 0 };
const TABLETS = [
  { name: "iPad upright", size: { width: 1032, height: 1376, platform: "ios" as const, safeArea: SAFE } },
  { name: "iPad sideways", size: { width: 1376, height: 1032, platform: "ios" as const, safeArea: SAFE } },
  { name: "Android tablet", size: { width: 800, height: 1280, platform: "android" as const } },
];
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });

for (const { name, size } of TABLETS) {
  test(`${name}: the phone layout: four tabs, a header with +, search and whose return it is, no sidebar or title bar`, async () => {
    const app = await open(ENTRY, { ...size, ...asking() });
    await app.waitFor("id:tab-bar");
    for (const gone of ["sidebar", "title-bar", "context-pill", "panel-profile"]) expect(app.shows(`id:${gone}`)).toBe(false);
    for (const label of ["Home", "Forms", "Filings", "Calendar"]) expect(app.texts()).toContain(label);
    expect(app.shows("id:nb-new")).toBe(true);
    expect(app.shows("id:nb-search")).toBe(true);
    // A bookkeeper always sees whose return is on screen: the name, and the TIN with its branch under it.
    expect(app.shows("id:nb-taxpayer")).toBe(true);
    expect(app.texts()).toContain("Juana Santos");
    expect(app.texts().some((t) => t.startsWith("000-123-453"))).toBe(true);
    // The name opens the tax profile as a sheet.
    await app.press("id:nb-taxpayer");
    await app.waitFor("id:sheet-extras");
    await app.back();
    await app.waitFor("id:sheet-extras", { state: "gone" });
    // Filings is the phone's list in both orientations, never the desktop board.
    await app.press("id:tab-filings");
    await app.waitFor("id:filings");
    expect(app.shows("id:filings-filters-open")).toBe(true);
    // The calendar is the phone's.
    await app.press("id:tab-calendar");
    await app.waitFor("id:calendar-narrow");
  }, 60_000);

  test(`${name}: Settings is a list whose rows push a page, with one Back in the header`, async () => {
    const app = await open(ENTRY, { ...size, ...asking({ start: "view=settings" }) });
    await app.waitFor("id:settings-nav-account");
    expect(app.shows("id:settings-rail")).toBe(false);
    await app.press("id:settings-nav-account");
    await app.waitFor("id:settings-security");
    expect(app.shows("id:nb-back")).toBe(true);
    expect(app.shows("id:settings-back")).toBe(false);
    await app.press("id:nb-back");
    await app.waitFor("id:settings-nav-account");
  }, 60_000);

  test(`${name}: a return is the phone form (no side list, the bottom pill), and search lists only tax profiles`, async () => {
    const app = await open(ENTRY, { ...size, ...asking({ start: "form=2551Q/2026/3" }) });
    await app.waitFor("id:fp-pill");
    expect(app.shows("id:fp-pager-clearance")).toBe(true);
    await app.press("id:nb-search");
    await app.waitFor("id:palette-input");
    expect(app.texts()).toContain("TAXPAYERS");
    for (const word of ["BRANCHES", "GO TO", "ACTIONS", "Toggle sidebar", "Change theme"]) expect(app.texts()).not.toContain(word);
    expect(app.shows("id:palette-item-1")).toBe(false);
  }, 60_000);
}
