// F19, the shortcut recorder with the wider key set, in a real window. Two things:
//  - the pictures: Settings > Computer records ctrl+alt+space and a punctuation chord (ctrl+alt+/), light and dark, wide and narrow.
//    The chord goes in through the harness's keyboard (app.keyboard.press), which the app's keymap bindings hear as a real key.
//  - every key beyond letters, digits and F1 to F12 (ui/chords.ts EXTRA_KEYS: space, the arrows, home, end, page up and down, the
//    punctuation keys, F13 to F20) is recorded in the desktop host with the REAL GlobalShortcut, and the system takes it (the host's
//    status says registered, not refused or invalid). The chords use all four modifiers, which no other app holds. Only a real window
//    answers, so headless this part passes without asking anything.
// (F13 to F20 cannot be pressed in a real window by the harness, which has F1 to F12; they are stored through the host instead.)
// `just nsh-shots f19` saves ~/BuwizNsh/evidence/f19/*.png. Headless it takes no pictures and passes.
import { keymap } from "nsh";
import { expect, open, test } from "nsh/test";
import { EXTRA_KEYS } from "../ui/chords";
import { LOOKS, shoot } from "./shoot";

const BOX = "id:settings-hotkey-recorder";
const asking = (params: Record<string, unknown> = {}) => ({ host: { params: { solo: true, start: "section=computer", ...params } } });

for (const look of LOOKS) {
  const size = { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme };

  test(`F19 recorder: the space bar and a punctuation key, ${look.name}`, async () => {
    const app = await open("../SettingsDemo.tsx", { ...size, ...asking() });
    await app.waitFor("id:settings-hotkey", { timeout: 30000 });
    await app.wait(800);
    expect(app.texts()).toContain("None");

    await app.press(BOX);
    await app.waitFor("text:Type shortcut");
    await app.wait(400);
    await shoot(app, `f19-${look.name}-1-recording`);

    // A bare space bar is answered in words and the box goes on listening.
    await app.keyboard.press("space");
    await app.waitFor("text:Add Control, Option or Command.");
    await app.wait(300);
    expect(app.texts()).toContain("Type shortcut");
    await shoot(app, `f19-${look.name}-2-bare-space`);

    await app.keyboard.press("ctrl+alt+space");
    await app.waitFor(`text:${keymap.label("ctrl+alt+space")}`);
    await app.wait(400);
    expect(app.texts()).not.toContain("Type shortcut");
    await shoot(app, `f19-${look.name}-3-ctrl-alt-space`);

    await app.press(BOX);
    await app.waitFor("text:Type shortcut");
    await app.keyboard.press("ctrl+alt+/");
    await app.waitFor(`text:${keymap.label("ctrl+alt+/")}`);
    await app.wait(400);
    expect(app.texts()).not.toContain("Type shortcut");
    await shoot(app, `f19-${look.name}-4-ctrl-alt-slash`);
  }, 120_000);
}

test("F19 every key the recorder adds is taken by the system as a global shortcut (a real window)", async () => {
  const app = await open("../DesktopDemo.tsx", { width: 1440, height: 900, platform: "macos", host: { params: { solo: true, start: "section=computer" } } });
  await app.waitFor("id:settings-hotkey", { timeout: 30000 });
  await app.wait(800);
  // Headless there is no system to ask (the global shortcut never answers): the real-window run is the one that proves it.
  if (!(await shoot(app, "f19-registers-0-start"))) return;
  const status = () => app.texts().find((t) => t.startsWith("hotkey=")) ?? "hotkey=?";
  const failures: string[] = [];
  for (const key of EXTRA_KEYS) {
    const chord = `ctrl+alt+shift+cmd+${key}`;
    // The harness's real-window keyboard presses F1 to F12 only (nsh-platform-macos synth.rs), so F13 to F20 are stored the way the
    // recorder stores a chord (the door's set_global_hotkey, through the app's own store) and the desktop host registers them.
    const pressable = !/^f(1[3-9]|2\d)$/.test(key);
    if (pressable) {
      await app.press(BOX);
      await app.waitFor("text:Type shortcut");
      // The host's hotkey is given back to the system while the box listens: its status goes to off before the chord is pressed.
      await app.waitFor("text:hotkey=off");
      await app.keyboard.press(chord);
      await app.waitFor(`text:${keymap.label(chord)}`);
    } else {
      await app.host.send("run", { id: "hotkey:" });
      await app.waitFor("text:hotkey=off");
      await app.host.send("run", { id: `hotkey:control+alt+shift+super+${key.toUpperCase()}` });
      await app.waitFor(`text:${keymap.label(chord)}`);
    }
    await app.waitUntil(() => /^hotkey=(registered|refused|invalid)$/.test(status()), { timeout: 10_000 });
    if (status() !== "hotkey=registered") failures.push(`${key}: ${status()}`);
  }
  await shoot(app, "f19-registers-1-last");
  expect(failures).toEqual([]);
}, 300_000);
