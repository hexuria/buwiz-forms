// Settings > Keyboard against the real desktop host (DesktopDemo.tsx): the quit and hide keys are recorded with ShortcutRecorder, the menu
// shows the keys chosen, and a key another item holds is refused under the box. Cmd+Q and Cmd+H are keys nsh/keymap cannot bind: while a
// box listens the application menu's Quit and Hide items hand them to it, so Cmd+Q can become Hide (Quit on Option+Cmd+Q first). The close
// button's choice (Windows and Linux) is a Dropdown, stored with the keys.
import { type App, expect, open, test } from "nsh/test";
import { warmForms } from "../../bir/testing";

warmForms();

const ENTRY = "../../DesktopDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const QUIT = "id:settings-quit-key";
const HIDE = "id:settings-hide-key";
const asking = (params: Record<string, unknown> = {}) => ({ host: { params: { start: "section=keyboard", quit: "fake", ...params } } });

/** What Settings stored ("desktop.close"), {} before anything was. */
const stored = (app: App): { quit?: string; hide?: string; close?: string } => JSON.parse(app.storage()["desktop.close"] ?? "{}");
const hidden = (app: App) => app.window("main").state()?.visible === false;
const shown = (app: App) => app.window("main").state()?.visible === true;

/** Presses `keys` until `done()` holds (the application menu is installed again a few times as the frame opens: see desktop.test.tsx). */
async function pressUntil(app: App, keys: string, done: () => boolean, cap = 15_000): Promise<void> {
  const end = Date.now() + cap;
  while (!done()) {
    if (Date.now() > end) throw new Error(`${keys} had no effect in ${cap} ms`);
    await app.keyboard.press(keys);
    await app.waitUntil(done, { timeout: 1000 }).catch(() => undefined);
  }
}

test("macOS: Cmd+Q is made Hide and Option+Cmd+Q Quit with the recorders; the menu shows them and the keys do it", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:settings-keys", { timeout: 30000 });
  expect(app.texts()).toContain("Quit eBIRForms");
  expect(app.texts()).toContain("Hides to the tray. Never asks.");
  expect(app.texts()).toContain("⌘Q");
  expect(app.texts()).toContain("⌘H");
  // macOS has no close question: the row says what the red button does.
  expect(app.texts()).toContain("The red button and ⌘W hide eBIRForms to the menu bar. Quit is always in the eBIRForms menu and the menu-bar icon.");
  await app.waitFor("id:run:eBIRForms > Quit eBIRForms");

  // Cmd+Q for Hide while Quit has it: refused under the box, which keeps listening; the app does not quit.
  await app.press(HIDE);
  await app.waitFor("text:Type shortcut");
  await pressUntil(app, "mod+q", () => app.texts().includes("That shortcut is already Quit eBIRForms."));
  expect(app.texts()).toContain("Type shortcut");
  expect(app.texts()).toContain("quits=0");
  // Another item's key is refused too, in its words.
  await app.keyboard.press("mod+k");
  await app.waitFor("text:That shortcut is already Command Palette….");
  await app.keyboard.press("escape");
  await app.waitFor("text:Type shortcut", { state: "gone" });

  // Quit moves to Option+Cmd+Q, then Cmd+Q is free for Hide (it comes through the menu's capture).
  await app.press(QUIT);
  await app.waitFor("text:Type shortcut");
  await app.keyboard.press("alt+cmd+q");
  await app.waitFor("text:⌥⌘Q");
  await app.press(HIDE);
  await app.waitFor("text:Type shortcut");
  await pressUntil(app, "mod+q", () => stored(app).hide === "cmd+q");
  await app.waitFor("text:Type shortcut", { state: "gone" });
  expect(app.texts()).toContain("quits=0");
  expect(JSON.parse(app.storage()["desktop.close"] ?? "{}")).toEqual({ quit: "alt+cmd+q", hide: "cmd+q", close: "ask" });

  // The menu shows the new keys, and they do it: Cmd+Q hides, Option+Cmd+Q quits.
  await app.waitFor("id:run:eBIRForms > Hide eBIRForms");
  await pressUntil(app, "mod+q", () => hidden(app));
  expect(app.storage()["fixture.desktop"]).toContain('"quits":0');
  await app.host.send("run", { id: "tray:Show eBIRForms" });
  await app.waitUntil(() => shown(app));
  await pressUntil(app, "alt+cmd+q", () => app.texts().includes("quits=1"));
});

test("a key is removed with Delete while recording: the item stays in the menu with no shortcut", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:settings-keys", { timeout: 30000 });
  await app.press(HIDE);
  await app.waitFor("text:Type shortcut");
  await app.keyboard.press("delete");
  await app.waitUntil(() => stored(app).hide === "");
  await app.waitFor("text:Click to record");
  await app.waitFor("id:run:eBIRForms > Hide eBIRForms");
});

for (const platform of ["windows", "linux"] as const) {
  test(`${platform}: the close button's choice is Ask, Hide to tray or Quit, stored with the keys; Super+Q can be bound for Quit`, async () => {
    const app = await open(ENTRY, { width: 1440, height: 900, platform, ...asking() });
    await app.waitFor("id:settings-close", { timeout: 30000 });
    expect(app.texts()).toContain("Close button (✕)");
    expect(app.texts()).toContain("Ask");
    expect(app.texts()).toContain("Ctrl+Q");
    expect(app.texts()).toContain("Ctrl+H");
    await app.press("id:settings-close-choice-trigger");
    await app.waitFor("text:Hide to tray");
    await app.press("text:Hide to tray");
    await app.waitFor("text:close=hide");
    expect(stored(app)).toEqual({ quit: "mod+q", hide: "mod+h", close: "hide" });
    // The close button now hides without asking.
    await app.window("main").pressClose();
    await app.waitUntil(() => hidden(app));
    await app.host.send("run", { id: "tray:Show eBIRForms" });
    await app.waitUntil(() => shown(app));
    expect(app.shows("id:close-ask")).toBe(false);

    if (platform === "linux") {
      // Super+Q is off by default; the recorder binds it.
      await app.press(QUIT);
      await app.waitFor("text:Type shortcut");
      await app.keyboard.press("cmd+q");
      await app.waitUntil(() => stored(app).quit === "cmd+q");
    }
  });
}
