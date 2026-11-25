// The frame on an iPad: a 13-inch iPad in portrait (1032 x 1376, iOS) with its status bar (24) and home indicator (20) as the safe
// area. The wide layout (sidebar, title bar, panels) is the desktop's. Against a scratch database with the demo taxpayer (ShellDemo.tsx).
// Where the title bar sits under the status bar is a matter of points this harness does not report (viewStyle has no layout); the
// Simulator pictures of the change show it.
import { expect, open, test } from "nsh/test";

const ENTRY = "../ShellDemo.tsx";
const IPAD = { width: 1032, height: 1376, platform: "ios" as const, safeArea: { top: 24, right: 0, bottom: 20, left: 0 } };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });

test("iPad: Settings beside the docked tax profile lists its sections first, and with room keeps the list beside the cards", async () => {
  // Sidebar and tax profile docked: the page is about 460 points, too narrow for the list and the cards side by side (the cards had
  // about 150 points, and each row's words wrapped one letter a line beside the theme dropdown).
  const app = await open(ENTRY, { ...IPAD, ...asking({ start: "view=settings;panel=profile" }) });
  await app.waitFor("id:settings");
  await app.waitFor("id:panel-profile");
  expect(app.shows("id:tab-bar")).toBe(false);
  expect(app.shows("id:settings-rail")).toBe(false);
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
