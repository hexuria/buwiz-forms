// Back and Forward in the mounted frame, after the history learned to carry each stop's taxpayer and to wait for the screens'
// beforeLeave hooks: the title-bar buttons and the keys still walk the pages. (The app state itself is tested in navigation.test.ts.)
import { expect, open, test } from "nsh/test";

const WIDE = { width: 1440, height: 900, platform: "macos" as const };

test("desktop: Back and Forward buttons and keys walk the pages, and moving somewhere new empties Forward", async () => {
  const app = await open("../ShellDemo.tsx", { ...WIDE, host: { params: {} } });
  await app.waitFor("id:sidebar");
  expect(app.enabled("id:tb-back")).toBe(false);
  expect(app.enabled("id:tb-forward")).toBe(false);

  await app.press("id:nav-library");
  await app.waitUntil(() => app.enabled("id:tb-back"));
  await app.press("id:nav-filings");
  await app.press("id:nav-calendar");
  expect(app.enabled("id:tb-forward")).toBe(false);

  await app.press("id:tb-back");
  await app.waitUntil(() => app.enabled("id:tb-forward"));
  await app.keyboard.press("mod+[");
  await app.keyboard.press("mod+]");
  await app.press("id:tb-forward");
  await app.waitUntil(() => !app.enabled("id:tb-forward"));

  await app.press("id:tb-back");
  await app.press("id:nav-home");
  await app.waitUntil(() => !app.enabled("id:tb-forward"));
  expect(app.enabled("id:tb-back")).toBe(true);
});
