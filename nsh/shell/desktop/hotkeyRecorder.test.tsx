// The shortcut recorder in Settings against the real desktop host (DesktopDemo.tsx, a fake GlobalShortcut): while the box listens the
// global hotkey is given back to the system, so the chord pressed to record it reaches the window instead of hiding it; the chord chosen is
// taken when it ends, the old one comes back when it is cancelled, and a chord another app holds is said under the box. The keys the platform
// keeps (Command+Q) still do what the app does with them once recording is over.
import { type App, expect, open, test } from "nsh/test";
import { warmForms } from "../../bir/testing";

warmForms();

const ENTRY = "../../DesktopDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const BOX = "id:settings-hotkey-recorder";
const TAKEN = "Another app is using this shortcut. Pick another one.";
const asking = (params: Record<string, unknown> = {}) => ({ host: { params: { solo: true, start: "section=computer", system: "fake", ...params } } });

/** The main window is hidden / shown (nothing on a hidden window can be read). */
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

test("recording gives the hotkey back to the system, cancelling takes it again, and a chord is taken when it ends", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ hotkey: "control+alt+shift+9" }) });
  await app.waitFor("id:settings-hotkey", { timeout: 30000 });
  await app.waitFor("text:chords=ctrl+alt+shift+9");
  expect(app.texts()).toContain("hotkey=registered");
  expect(app.texts()).toContain("⌃⌥⇧9");

  // The box listens: the system has the chord back (pressing it now would reach the window, not hide it).
  await app.press(BOX);
  await app.waitFor("text:Type shortcut");
  await app.waitFor("text:chords=");
  expect(app.texts()).toContain("hotkey=off");
  // Escape: nothing changed, and the old chord is taken again.
  await app.keyboard.press("escape");
  await app.waitFor("text:chords=ctrl+alt+shift+9");
  expect(app.texts()).toContain("hotkey=registered");
  expect(app.texts()).toContain("⌃⌥⇧9");

  // A new chord: given back while it is chosen, and the NEW one is taken, not the old one again.
  await app.press(BOX);
  await app.waitFor("text:chords=");
  await app.keyboard.press("ctrl+alt+b");
  await app.waitFor("text:chords=ctrl+alt+b");
  expect(app.texts()).toContain("hotkey=registered");
  expect(app.texts()).toContain("⌃⌥B");

  // Removing it (Delete while recording): nothing is held, and nothing comes back.
  await app.press(BOX);
  await app.waitFor("text:chords=");
  await app.keyboard.press("delete");
  await app.waitFor("text:None");
  expect(app.texts()).toContain("chords=");
  expect(app.texts()).toContain("hotkey=off");

  // The chord a person records is the system's again afterwards: pressing it (the fake's own press) toggles the window as before.
  await app.press(BOX);
  await app.keyboard.press("ctrl+alt+h");
  await app.waitFor("text:chords=ctrl+alt+h");
  await app.host.send("run", { id: "chord:ctrl+alt+h" });
  await app.waitUntil(() => hidden(app));
  await app.host.send("run", { id: "tray:Show eBIRForms" });
  await app.waitUntil(() => shown(app));
});

test("a chord another app holds is said under the box in plain words, and another chord takes the words away", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ refuse: "ctrl+alt+b" }) });
  await app.waitFor("id:settings-hotkey", { timeout: 30000 });
  expect(app.texts()).toContain("None");
  await app.press(BOX);
  await app.keyboard.press("ctrl+alt+b");
  await app.waitFor(`text:${TAKEN}`);
  expect(app.texts()).toContain("hotkey=refused");
  expect(app.texts()).toContain("⌃⌥B");
  // Recording again hides the words (the box has its prompt), and a chord the system takes clears them.
  await app.press(BOX);
  await app.waitFor("text:Type shortcut");
  expect(app.texts()).not.toContain(TAKEN);
  await app.keyboard.press("ctrl+alt+h");
  await app.waitFor("text:chords=ctrl+alt+h");
  await app.waitFor("text:hotkey=registered");
  expect(app.texts()).not.toContain(TAKEN);
  expect(app.texts()).toContain("⌃⌥H");
});

test("Command+Q does what the app does with it before and after a recording, and the recording leaves nothing bound", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ hotkey: "control+alt+shift+9" }) });
  await app.waitFor("id:settings-hotkey", { timeout: 30000 });
  await app.waitFor("text:chords=ctrl+alt+shift+9");
  await app.press(BOX);
  await app.waitFor("text:Type shortcut");
  await app.keyboard.press("escape");
  await app.waitFor("text:chords=ctrl+alt+shift+9");
  // The recording is over: keys it listened for do nothing, and Command+Q is the app's own again (it hides to the tray on macOS).
  await app.keyboard.press("ctrl+alt+z");
  expect(app.texts()).not.toContain("⌃⌥Z");
  expect(app.texts()).toContain("⌃⌥⇧9");
  await pressUntil(app, "mod+q", () => hidden(app));
  await app.host.send("run", { id: "tray:Show eBIRForms" });
  await app.waitUntil(() => shown(app));
});
