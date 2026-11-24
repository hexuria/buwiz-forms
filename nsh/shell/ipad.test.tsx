// The frame on an iPad: a 13-inch iPad in portrait (1032 x 1376, iOS) with its status bar (24) and home indicator (20) as the safe
// area. The wide layout (sidebar, title bar, panels) is the desktop's, so it has to keep off both. Against a scratch database with the
// demo taxpayer (ShellDemo.tsx).
import { expect, open, test } from "nsh/test";

const ENTRY = "../ShellDemo.tsx";
const IPAD = { width: 1032, height: 1376, platform: "ios" as const, safeArea: { top: 24, right: 0, bottom: 20, left: 0 } };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });

test("iPad: the title bar starts under the status bar and the frame keeps off the home indicator", async () => {
  const app = await open(ENTRY, { ...IPAD, ...asking() });
  await app.waitFor("id:title-bar");
  expect(app.shows("id:sidebar")).toBe(true);
  expect(app.shows("id:tab-bar")).toBe(false);
  // 40 points of bar under the 24 of status bar: its buttons are never drawn over the clock.
  expect(app.viewStyle("id:title-bar")?.paddingTop).toBe(24);
  expect(app.viewStyle("id:title-bar")?.height).toBe(64);
  expect(app.viewStyle("id:frame-body")?.paddingBottom).toBe(20);
  // The floating panels start under the taller bar too: narrowed under 860, the sidebar floats.
  await app.setViewport(800, 1100);
  await app.press("id:tb-sidebar");
  await app.waitFor("id:sidebar-overlay");
  expect(app.viewStyle("id:sidebar-overlay")?.top).toBe(64);
});

test("desktop: the title bar keeps its 40 points (no safe area)", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", ...asking() });
  await app.waitFor("id:title-bar");
  expect(app.viewStyle("id:title-bar")?.height).toBe(40);
});

test("iPad: Settings beside the docked tax profile lists its sections first, and with room keeps the list beside the cards", async () => {
  // Sidebar and tax profile docked: the page is about 460 points, too narrow for the list and the cards side by side.
  const app = await open(ENTRY, { ...IPAD, ...asking({ start: "view=settings;panel=profile" }) });
  await app.waitFor("id:settings");
  await app.waitFor("id:panel-profile");
  expect(app.shows("id:settings-rail")).toBe(false);
  await app.waitFor("id:settings-nav-general");
  await app.press("id:settings-nav-general");
  await app.waitFor("id:settings-appearance");
  expect(app.shows("id:settings-back")).toBe(true);
  expect(app.texts()).toContain("Theme");
  await app.press("id:settings-back");
  await app.waitFor("id:settings-nav-general");

  // The tax profile closed: the page has room, the list stays beside the cards.
  const wide = await open(ENTRY, { ...IPAD, ...asking({ start: "view=settings" }) });
  await wide.waitFor("id:settings-rail");
  await wide.waitFor("id:settings-appearance");
  expect(wide.shows("id:settings-back")).toBe(false);
});
