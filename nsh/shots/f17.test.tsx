// F17, the shortcut recorder in Settings > Computer, in a real window: idle (None and its hint), recording (the accent border, "Type shortcut",
// the bubble under the box), a bare key answered in words, recorded (the chord and its remove button), and the host's words under the box when
// another app holds the chord. Light and dark, wide and narrow. The chord goes in through the harness's keyboard (app.keyboard.press), which
// the app's keymap bindings hear as a real key. `just nsh-shots f17` saves ~/BuwizNsh/evidence/f17/*.png. Headless it takes no pictures and passes.
import { expect, open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

const ENTRY = "../SettingsDemo.tsx";
const BOX = "id:settings-hotkey-recorder";
const asking = (params: Record<string, unknown> = {}) => ({ host: { params: { solo: true, start: "section=computer", ...params } } });

for (const look of LOOKS) {
  const size = { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme };

  test(`F17 recorder, ${look.name}`, async () => {
    const app = await open(ENTRY, { ...size, ...asking() });
    await app.waitFor("id:settings-hotkey", { timeout: 30000 });
    await app.wait(800);
    expect(app.texts()).toContain("None");
    await shoot(app, `f17-${look.name}-1-idle`);

    await app.press(BOX);
    await app.waitFor("text:Type shortcut");
    await app.wait(400);
    await shoot(app, `f17-${look.name}-2-recording`);

    // A bare key: answered, and still listening.
    await app.keyboard.press("b");
    await app.waitFor("text:Add Control, Option or Command.");
    await app.wait(300);
    expect(app.texts()).toContain("Type shortcut");
    await shoot(app, `f17-${look.name}-3-bare`);

    // The real chord, through the harness's keyboard.
    await app.keyboard.press("ctrl+alt+b");
    await app.waitFor("text:⌃⌥B");
    await app.wait(400);
    expect(app.texts()).not.toContain("Type shortcut");
    await shoot(app, `f17-${look.name}-4-recorded`);

    // Recording again, then Escape: the chord is kept.
    await app.press(BOX);
    await app.waitFor("text:Type shortcut");
    await app.keyboard.press("escape");
    await app.waitFor("text:⌃⌥B");
    expect(app.texts()).not.toContain("Type shortcut");
  }, 120_000);

  test(`F17 recorder with the host's words under the box, ${look.name}`, async () => {
    const app = await open(ENTRY, { ...size, ...asking({ status: "refused" }) });
    await app.waitFor("id:settings-hotkey", { timeout: 30000 });
    await app.waitFor("text:Another app is using this shortcut. Pick another one.");
    await app.wait(400);
    await shoot(app, `f17-${look.name}-5-refused`);
  }, 120_000);
}
