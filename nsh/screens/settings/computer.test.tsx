// Settings > Computer at narrow and wide desktop sizes: the local files, log retention and the global hotkey (a shortcut recorder: the box is
// pressed, then the keys; stored through the real host), how closing behaves, the version, and the "who uses this computer" change (an owner
// or a help desk) with its recovery key and the sign-out that follows. Against a scratch database with the demo taxpayer (SettingsDemo.tsx).
import { keymap } from "nsh";
import { expect, test } from "nsh/test";
import { totpCode } from "../auth/testTotp";
import { calls, expectTexts, goneText, openSettings, SIZES as VIEWPORTS, state } from "./testing";

const SIZES = VIEWPORTS.map(({ name, size }) => ({
  name: name === "phone" ? "narrow desktop" : name,
  size: { ...size, platform: "macos" as const },
}));

const BOX = "id:settings-hotkey-recorder";
const CLEAR = "id:settings-hotkey-recorder-clear";
const NEED_MODIFIER = "Add Control, Option or Command.";

const SECRET = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
const KEY = /^[A-Z0-9]{4}(-[A-Z0-9]{4}){3}$/;

for (const { name, size } of SIZES) {
  test(`${name}: the local files and the version are told; retention is stored by the host`, async () => {
    const app = await openSettings(size, { solo: true }, "computer");
    await app.waitFor("id:settings-files");
    expectTexts(app, [
      "Computer", "Local files", "Database", "Data folder", "Logs folder", "Keep logs for", "Older daily log files are deleted", "7 days",
      "Shortcuts", "Global show hotkey", "Shows or hides eBIRForms from any app", "None", "Click to record", "Computer setup", "eBIRForms 0.2.0",
    ]);
    // How closing works is Settings > Keyboard's now.
    expect(app.texts()).not.toContain("Close window");
    expect(app.texts().some((t) => t.includes("ebirforms-door") && t.endsWith("bir_data.db"))).toBe(true);
    // The logs folder inside the app's data folder, with the system's own separator (a backslash on Windows).
    expect(app.texts().some((t) => t.replace(/\\/g, "/").endsWith("dev.goldcoders.bir.nsh/logs"))).toBe(true);

    // The longest retention loses nothing newer than 90 days of the NSH app's own logs.
    await app.press("id:settings-retention-trigger");
    await app.waitFor("text:90 days");
    await app.press("text:90 days");
    await app.waitFor("text:90 days");
    await goneText(app, "7 days"); // the popup, with all its options, slides away
    expect(app.texts()).not.toContain("7 days");
  }, 90_000);

  test(`${name}: the hotkey is recorded with the keys: a press on the box, then the chord; the host is asked to store it and the box shows it`, async () => {
    const app = await openSettings(size, { solo: true, hotkey: "note" }, "computer");
    await app.waitFor("id:settings-hotkey");
    expect(app.texts()).toContain("None");
    expect(app.texts()).toContain("Click to record");
    expect(app.shows(CLEAR)).toBe(false);

    await app.press(BOX);
    await app.waitFor("text:Type shortcut");
    expect(app.texts()).toContain("Esc to cancel · Delete to remove");
    expect(app.texts()).not.toContain("None");
    await app.keyboard.press("ctrl+alt+b");
    await app.waitFor("text:⌃⌥B");
    await goneText(app, "Type shortcut");
    expect(calls(app)).toEqual(["set_global_hotkey control+alt+B"]);
    expect(app.shows(CLEAR)).toBe(true);
    expect(app.texts()).not.toContain("Click to record");

    // Another chord replaces it: Command is stored as super, in the order control, alt, shift, super.
    await app.press(BOX);
    await app.waitFor("text:Type shortcut");
    await app.keyboard.press("cmd+shift+f12");
    await app.waitFor("text:⇧⌘F12");
    expect(calls(app)).toEqual(["set_global_hotkey control+alt+B", "set_global_hotkey shift+super+F12"]);

    // The small x removes it: the host is asked to store nothing, and the box says None again.
    await app.press(CLEAR);
    await app.waitFor("text:None");
    expect(calls(app)).toEqual(["set_global_hotkey control+alt+B", "set_global_hotkey shift+super+F12", "set_global_hotkey"]);
    expect(app.shows(CLEAR)).toBe(false);
  }, 60_000);

  test(`${name}: the space bar, the arrows, punctuation and F13 to F20 are recorded and stored through the real host, as their words`, async () => {
    // The host (set_global_hotkey, whose hooks are this app's) stores what the box offers: the box shows the chord read back from it.
    const app = await openSettings(size, { solo: true, hotkey: "note" }, "computer");
    await app.waitFor("id:settings-hotkey");
    const sent: string[] = [];
    for (const [chord, stored] of [
      ["ctrl+alt+space", "control+alt+SPACE"],
      ["alt+left", "alt+LEFT"],
      ["ctrl+alt+pagedown", "control+alt+PAGEDOWN"],
      ["shift+cmd+,", "shift+super+COMMA"],
      ["ctrl+alt+[", "control+alt+LEFTBRACKET"],
      ["ctrl+alt+plus", "control+alt+EQUAL"],
      ["ctrl+alt+minus", "control+alt+MINUS"],
      ["ctrl+alt+\\", "control+alt+BACKSLASH"],
      ["ctrl+alt+`", "control+alt+BACKTICK"],
      ["cmd+f13", "super+F13"],
    ] as const) {
      await app.press(BOX);
      await app.waitFor("text:Type shortcut");
      await app.keyboard.press(chord);
      await app.waitFor(`text:${keymap.label(chord)}`);
      await goneText(app, "Type shortcut");
      sent.push(`set_global_hotkey ${stored}`);
      expect(calls(app)).toEqual(sent);
    }
    // A new key alone, or with Shift alone, is answered like any bare key, and the host is not asked.
    await app.press(BOX);
    await app.waitFor("text:Type shortcut");
    for (const bare of ["space", "left", "shift+space", "f13", "shift+,"]) {
      await app.keyboard.press(bare);
      await app.waitFor(`text:${NEED_MODIFIER}`);
      expect(app.texts()).toContain("Type shortcut");
    }
    expect(calls(app)).toEqual(sent);
    await app.keyboard.press("escape");
    await goneText(app, "Type shortcut");
    expect(app.texts()).toContain(keymap.label("cmd+f13"));
  }, 90_000);

  test(`${name}: Escape cancels and stores nothing; Delete removes the chord; a key with no Control, Option or Command is answered and the box keeps listening`, async () => {
    const app = await openSettings(size, { solo: true, hotkey: "note" }, "computer");
    await app.waitFor("id:settings-hotkey");
    await app.press(BOX);
    await app.waitFor("text:Type shortcut");
    await app.keyboard.press("escape");
    await goneText(app, "Type shortcut");
    expect(app.texts()).toContain("None");
    expect(calls(app)).toEqual([]);

    // A bare key and Shift with a key are answered, and nothing is stored; the box is still waiting.
    await app.press(BOX);
    await app.waitFor("text:Type shortcut");
    for (const keys of ["b", "shift+b", "f5"]) {
      await app.keyboard.press(keys);
      await app.waitFor(`text:${NEED_MODIFIER}`);
      expect(app.texts()).toContain("Type shortcut");
    }
    expect(calls(app)).toEqual([]);

    // A key the platform keeps (Command+Q quits) is not a chord: the box still waits, and the app's own Command+Q is not taken.
    await app.keyboard.press("mod+q");
    expect(app.texts()).toContain("Type shortcut");
    expect(calls(app)).toEqual([]);

    // The next chord with a modifier is the shortcut.
    await app.keyboard.press("alt+h");
    await app.waitFor("text:⌥H");
    await goneText(app, "Type shortcut");
    expect(app.texts()).not.toContain(NEED_MODIFIER);
    expect(calls(app)).toEqual(["set_global_hotkey alt+H"]);

    // Recording is over, and nothing is left listening: a chord, Escape and Delete do nothing now.
    for (const keys of ["ctrl+alt+z", "escape", "delete", "backspace"]) await app.keyboard.press(keys);
    expect(calls(app)).toEqual(["set_global_hotkey alt+H"]);
    expect(app.texts()).toContain("⌥H");

    // Delete while recording removes the chord.
    await app.press(BOX);
    await app.waitFor("text:Type shortcut");
    await app.keyboard.press("delete");
    await app.waitFor("text:None");
    expect(calls(app)).toEqual(["set_global_hotkey alt+H", "set_global_hotkey"]);
    // Delete with nothing to remove asks the host nothing.
    await app.press(BOX);
    await app.waitFor("text:Type shortcut");
    await app.keyboard.press("backspace");
    await goneText(app, "Type shortcut");
    expect(calls(app)).toEqual(["set_global_hotkey alt+H", "set_global_hotkey"]);
  }, 90_000);

  test(`${name}: while the box listens the app's own shortcuts are chords, and after it ends they are the app's again`, async () => {
    const app = await openSettings(size, { solo: true, hotkey: "note" }, "computer");
    await app.waitFor("id:settings-hotkey");
    await app.press(BOX);
    await app.waitFor("text:Type shortcut");
    // Command+K opens the palette, but while the box listens it is the chord that was pressed.
    await app.keyboard.press("mod+k");
    await app.waitFor("text:⌘K");
    expect(calls(app)).toEqual(["set_global_hotkey super+K"]);
    expect(app.shows("id:palette-input")).toBe(false);
    // Recording is over and nothing of it is left bound: Command+K opens the palette again, and Escape (which the box also listened for) closes it.
    await app.keyboard.press("mod+k");
    await app.waitFor("id:palette-input");
    await app.keyboard.press("escape");
    await app.waitFor("id:palette-input", { state: "gone" });
    expect(calls(app)).toEqual(["set_global_hotkey super+K"]);
  }, 60_000);

  test(`${name}: a hotkey the host refuses is said in its words and the box goes back to the stored chord`, async () => {
    const app = await openSettings(size, { solo: true, hotkey: "refuse" }, "computer");
    await app.waitFor("id:settings-hotkey");
    await app.press(BOX);
    await app.waitFor("text:Type shortcut");
    await app.keyboard.press("ctrl+alt+b");
    await app.waitFor("text:That shortcut is not one this computer can register. Use a letter, digit, or F-key with modifiers, like super+shift+E.");
    expect(calls(app)).toEqual(["set_global_hotkey control+alt+B"]);
    await goneText(app, "Type shortcut");
    // Nothing was stored, so the box says None again and not the chord that was refused.
    expect(app.texts()).toContain("None");
    expect(app.texts()).not.toContain("⌃⌥B");
    expect(app.shows(CLEAR)).toBe(false);
  }, 60_000);

  test(`${name}: where the system stands is said under the box: a chord another app holds, a chord this computer cannot register`, async () => {
    for (const [status, words] of [
      ["refused", "Another app is using this shortcut. Pick another one."],
      ["invalid", "This computer cannot register that shortcut."],
    ] as const) {
      const app = await openSettings(size, { solo: true, status }, "computer");
      await app.waitFor("id:settings-hotkey");
      await app.waitFor(`text:${words}`);
      // While the box listens it shows its prompt, not the words; Escape brings them back.
      await app.press(BOX);
      await app.waitFor("text:Type shortcut");
      expect(app.texts()).not.toContain(words);
      await app.keyboard.press("escape");
      await app.waitFor(`text:${words}`);
    }
    // Where there is no desktop host (here) or the chord is registered, nothing extra shows.
    const quiet = await openSettings(size, { solo: true }, "computer");
    await quiet.waitFor("id:settings-hotkey");
    expect(quiet.texts()).not.toContain("Another app is using this shortcut. Pick another one.");
    expect(quiet.texts()).not.toContain("This computer cannot register that shortcut.");
  }, 60_000);

  test(`${name}: the owner changes: another taxpayer is chosen, the admin PIN confirms it, a new recovery key shows once, then the computer signs out`, async () => {
    const app = await openSettings(size, { solo: true, second: true, pin: "1234", signin: true }, "computer");
    await app.waitFor("id:setup-change-btn");
    expect(app.texts()).toContain("Just me or my business. Owner: Juana Santos, TIN 000-123-453");
    await app.press("id:setup-change-btn");
    await app.waitFor("id:setup-change-choose");
    expect(app.texts()).toContain("Hand the computer to another taxpayer, or use it to help other taxpayers.");
    await app.press("id:setup-change-owner");
    await app.waitFor("id:setup-change-owner-head");
    expectTexts(app, ["New owner", "Sample Trading Corp", "TIN 000-111-229"]);
    // The owner herself is not a candidate.
    expect(app.shows("id:setup-change-cand-000123453")).toBe(false);
    await app.press("id:setup-change-pick-000111229");
    await app.waitFor("id:setup-change-what");
    expect(app.texts()).toContain("Make Sample Trading Corp (TIN 000-111-229) the owner and admin");
    expect(app.texts()).toContain("Your current admin PIN");
    // The admin has no authenticator: no Use code.
    expect(app.shows("id:setup-change-toggle")).toBe(false);

    // Four digits, then a wrong PIN: the host's refusal is said, and the cells clear.
    await app.type("id:setup-change-pin-cells", "12");
    await app.press("id:setup-change-confirm-btn");
    await app.waitFor("text:Type all 4 digits of your current admin PIN.");
    await app.type("id:setup-change-pin-cells", "99");
    await app.press("id:setup-change-confirm-btn");
    await app.waitFor("text:/PIN/", { timeout: 5000 });
    expect(app.shows("id:setup-change-what")).toBe(true);
    expect(app.value("id:setup-change-pin-cells")).toBe("");

    await app.type("id:setup-change-pin-cells", "1234");
    await app.press("id:setup-change-confirm-btn");
    await app.waitFor("id:setup-change-key", { timeout: 15000 });
    expectTexts(app, ["New recovery key", "Write it down now. It is shown once and works once; the old key no longer works.", "Setup changed", "I wrote it down"]);
    expect(app.texts().some((t) => KEY.test(t))).toBe(true);
    // Only the key shows now: no section list, and the title is Computer.
    expect(app.shows("id:settings-nav-general")).toBe(false);
    expect(state(app)).toContain("admin=000111229");

    await app.press("id:setup-change-done");
    await app.waitFor("text:/locked=true/");
    expect(state(app)).toContain("session=open");
    expect(app.shows("id:settings")).toBe(false);
  }, 90_000);

  test(`${name}: leaving Settings while the new recovery key shows signs the computer out all the same`, async () => {
    const app = await openSettings(size, { solo: true, second: true, pin: "1234", signin: true }, "computer");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-owner");
    await app.press("id:setup-change-pick-000111229");
    await app.type("id:setup-change-pin-cells", "1234");
    await app.press("id:setup-change-confirm-btn");
    await app.waitFor("id:setup-change-done", { timeout: 15000 });
    await app.press(name === "narrow desktop" ? "id:tab-home" : "id:nav-home");
    await app.waitFor("text:/locked=true/");
    expect(state(app)).toContain("admin=000111229");
  }, 90_000);

  test(`${name}: Cancel closes the change at each step and forgets what was typed`, async () => {
    const app = await openSettings(size, { solo: true, second: true, pin: "1234", signin: true }, "computer");
    await app.waitFor("id:setup-change-btn");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-cancel");
    await app.waitFor("id:setup-change-btn");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-owner");
    await app.waitFor("id:setup-change-owner-head");
    await app.press("id:setup-change-cancel");
    await app.waitFor("id:setup-change-btn");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-helpdesk");
    await app.waitFor("id:setup-change-newpin");
    await app.type("id:setup-change-newpin-cells", "2468");
    await app.press("id:setup-change-cancel");
    await app.waitFor("id:setup-change-btn");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-helpdesk");
    await app.waitFor("id:setup-change-newpin");
    expect(app.value("id:setup-change-newpin-cells")).toBe("");
    await app.press("id:setup-change-cancel");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-owner");
    await app.press("id:setup-change-pick-000111229");
    await app.waitFor("id:setup-change-what");
    await app.press("id:setup-change-cancel");
    await app.waitFor("id:setup-change-btn");
    expect(state(app)).toContain("admin=000123453");
  }, 90_000);

  test(`${name}: a taxpayer with no PIN yet cannot be chosen as the owner`, async () => {
    const app = await openSettings(size, { solo: true, second: "nopin", pin: "1234", signin: true }, "computer");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-owner");
    await app.waitFor("id:setup-change-owner-head");
    expect(app.texts()).toContain("TIN 000-111-229. No PIN yet: they set one in Settings after signing in");
    expect(app.shows("id:setup-change-pick-000111229")).toBe(false);
  }, 60_000);

  test(`${name}: with no other taxpayer, Another owner says to add one first`, async () => {
    const app = await openSettings(size, { solo: true, pin: "1234", signin: true }, "computer");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-owner");
    await app.waitFor("id:setup-change-none");
    expectTexts(app, ["No other taxpayers", "Add the taxpayer first, then come back here."]);
  }, 60_000);

  test(`${name}: switching to a help desk takes a four-digit admin PIN, the current PIN, and ends in the help desk's own sign-in`, async () => {
    const app = await openSettings(size, { solo: true, pin: "1234", signin: true }, "computer");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-helpdesk");
    await app.waitFor("id:setup-change-newpin");
    expectTexts(app, ["Switch to a help desk", "New admin PIN", "4 digits, for TIN 000-000-000"]);
    await app.press("id:setup-change-newpin-next");
    await app.waitFor("text:Type all 4 digits of the new admin PIN.");
    await app.type("id:setup-change-newpin-cells", "2468");
    await app.press("id:setup-change-newpin-next");
    await app.waitFor("id:setup-change-what");
    expect(app.texts()).toContain("Switch to a help desk with the new admin PIN");
    await app.type("id:setup-change-pin-cells", "1234");
    await app.press("id:setup-change-confirm-btn");
    await app.waitFor("id:setup-change-key", { timeout: 15000 });
    expect(app.texts().some((t) => KEY.test(t))).toBe(true);
    expect(state(app)).toContain("mode=helpdesk");
    await app.press("id:setup-change-done");
    await app.waitFor("text:/locked=true/");
  }, 90_000);

  test(`${name}: on a help desk the choice is An owner, and the authenticator code can confirm it instead of the PIN`, async () => {
    const app = await openSettings(size, { helpdesk: "2468", second: true, totpSecret: SECRET, signin: true }, "computer");
    await app.waitFor("id:setup-change-btn");
    expect(app.texts()).toContain("A help desk. The admin signs in with TIN 000-000-000");
    await app.press("id:setup-change-btn");
    await app.waitFor("id:setup-change-choose");
    expect(app.texts()).toContain("Make one of the taxpayers the owner. They become the admin and sign in with their own PIN.");
    expect(app.shows("id:setup-change-helpdesk")).toBe(false);
    expect(app.texts()).toContain("An owner");
    await app.press("id:setup-change-owner");
    await app.waitFor("id:setup-change-owner-head");
    // Juana has no PIN, so only Sample Trading Corp can be chosen.
    expect(app.shows("id:setup-change-pick-000123453")).toBe(false);
    await app.press("id:setup-change-pick-000111229");
    await app.waitFor("id:setup-change-what");
    expect(app.shows("id:setup-change-pin-cells")).toBe(true);
    await app.press("id:setup-change-toggle");
    await app.waitFor("id:setup-change-code-cells");
    expect(app.texts()).toContain("Your admin authenticator code");
    await app.type("id:setup-change-code-cells", "12");
    await app.press("id:setup-change-confirm-btn");
    await app.waitFor("text:Type all 6 digits of the code.");
    await app.clear("id:setup-change-code-cells");
    await app.type("id:setup-change-code-cells", totpCode(SECRET));
    await app.press("id:setup-change-confirm-btn");
    await app.waitFor("id:setup-change-key", { timeout: 15000 });
    expect(state(app)).toContain("admin=000111229");
    expect(state(app)).toContain("mode=solo");
  }, 90_000);
}
