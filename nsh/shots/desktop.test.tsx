// The desktop host in real windows (nsh test --target macos): the close button hides the window and Show brings it back at the same
// place and size, the global hotkey is taken from the real system, and the update banner and the About dialog, light and dark.
// What a real run cannot see: the system's menu bar and tray (no test API reads or clicks them), so the menu's items are run through the
// fixture's strip, as in the headless test. Headless (plain nsh test) it takes no pictures and checks what it can.
// "just nsh-shots desktop" saves ~/BuwizNsh/evidence/desktop/*.png.
import { expect, open, test } from "nsh/test";
import { shoot } from "./shoot";

const ENTRY = "../DesktopDemo.tsx";
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });
const LOOKS = [
  { name: "light", colorScheme: "light" },
  { name: "dark", colorScheme: "dark" },
] as const;

for (const look of LOOKS) {
  test("the update banner and the About dialog, " + look.name, async () => {
    const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", colorScheme: look.colorScheme, ...asking({ poll: true, updater: "found" }) });
    await app.waitFor("id:sidebar");
    await app.press("id:run:Help > Check for Updates…");
    await app.waitFor("id:update-banner", { timeout: 15_000 });
    await app.wait(400);
    await shoot(app, "desktop-" + look.name + "-update-banner");

    await app.press("id:run:eBIRForms > About eBIRForms");
    await app.waitFor("id:about");
    await app.wait(400);
    await shoot(app, "desktop-" + look.name + "-about");
    await app.press("id:about-ok");
    await app.press("id:update-later");
    await app.waitFor("id:update-banner", { state: "gone" });
  }, 120_000);
}

test("the update banner over a form page keeps clear of the form's pill", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", colorScheme: "light", ...asking({ poll: true, updater: "found", start: "form=1601C/2025/3" }) });
  await app.waitFor("text:view=form");
  await app.press("id:run:Help > Check for Updates…");
  await app.waitFor("id:update-banner", { timeout: 15_000 });
  await app.wait(600);
  await shoot(app, "desktop-light-update-banner-form");
  expect(app.shows("id:update-install")).toBe(true);
}, 120_000);

test("the update banner on a narrow window", async () => {
  const app = await open(ENTRY, { width: 390, height: 844, platform: "macos", colorScheme: "light", ...asking({ poll: true, updater: "found" }) });
  await app.waitFor("id:shell");
  await app.press("id:run:Help > Check for Updates…");
  await app.waitFor("id:update-banner", { timeout: 15_000 });
  await app.wait(400);
  await shoot(app, "desktop-narrow-update-banner");
  expect(app.shows("id:update-install")).toBe(true);
  expect(app.shows("id:update-later")).toBe(true);
}, 120_000);

test("the close button hides the window, and Show brings it back where it was", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", ...asking() });
  await app.waitFor("id:sidebar");
  const before = app.window("main").state();
  expect(before?.visible).toBe(true);
  await app.waitFor("id:sidebar");

  await app.window("main").pressClose();
  await app.waitUntil(() => app.window("main").state()?.visible === false, { timeout: 10_000 });

  await app.host.send("run", { id: "tray:Show eBIRForms" });
  await app.waitUntil(() => app.window("main").state()?.visible === true, { timeout: 10_000 });
  const after = app.window("main").state();
  // Same place, same size: hiding is not closing. (A headless window comes back at the default viewport, so only a real window is held to it.)
  const real = await shoot(app, "desktop-shown-again");
  if (real) expect(after?.bounds).toEqual(before?.bounds);
  expect(app.texts()).toContain("view=home");
}, 120_000);

test("the global hotkey is taken from the real system and given back", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", ...asking({ hotkey: "control+alt+shift+9" }) });
  await app.waitFor("id:sidebar");
  // The system answers at once on macOS (no permission is asked); where another app holds the chord it says refused, and the
  // note says which: either way the status is an answer, never "pending" for good.
  const real = await shoot(app, "desktop-hotkey");
  // A headless run has no system to ask: the chord waits for its answer.
  await app.waitFor(real ? "text:/^hotkey=(registered|refused)$/" : "text:hotkey=pending", { timeout: 10_000 });
  await app.host.send("run", { id: "hotkey:" });
  await app.waitFor("text:hotkey=off", { timeout: 10_000 });
}, 120_000);
