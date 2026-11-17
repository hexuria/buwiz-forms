// The shortcut recorder at phone and desktop size: it shows the chord as the platform writes it, records the next chord, cancels on Escape,
// clears on Delete, answers a chord that is not acceptable and stays recording, leaves the keys the platform keeps alone, and leaves
// nothing bound when it ends.
import { keymap } from "nsh";
import { type App, expect, test } from "nsh/test";
import { openKit, readout, SIZES } from "./harness";

const LINE = "Shortcut: ctrl+alt+e | Recording: no | Needs a modifier: none";
const state = (app: App) => readout(app, "Shortcut:");
const said = (app: App, text: string) => app.texts().includes(text);

for (const size of SIZES) {
  test(`${size.name}: idle, it shows the chord as the platform writes it, or None with its hint; an error shows under the box`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    expect(state(app)).toBe(LINE);
    // Apple platforms (macOS, and iOS as an iPad with a keyboard) write the symbols.
    expect(app.texts()).toContain("⌃⌥E");
    expect(app.texts()).toContain("⌃⌥B");
    expect(app.texts()).toContain("None");
    expect(app.texts()).toContain("Click to record");
    expect(app.texts()).toContain("Another app is using this shortcut. Pick another one.");
    expect(app.shows("id:kit-shortcut-clear")).toBe(true);
    expect(app.shows("id:kit-shortcut-bare-clear")).toBe(false);
    expect(app.shows("label:Show shortcut: ⌃⌥E")).toBe(true);
    expect(said(app, "Type shortcut")).toBe(false);
    // A disabled box cannot be pressed to record.
    await expect(app.press("id:kit-shortcut-locked")).rejects.toMatchObject({ code: "ERR_NSH_TEST_DISABLED" });
    expect(said(app, "Type shortcut")).toBe(false);
  });

  test(`${size.name}: a press records: the box says so, and the next chord is the new shortcut`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    await app.press("id:kit-shortcut");
    await app.waitFor("text:Type shortcut");
    expect(state(app)).toBe("Shortcut: ctrl+alt+e | Recording: yes | Needs a modifier: none");
    expect(app.texts()).toContain("Esc to cancel · Delete to remove");
    expect(app.shows("id:kit-shortcut-clear")).toBe(false);
    await app.keyboard.press("ctrl+alt+b");
    await app.waitFor("text:Shortcut: ctrl+alt+b | Recording: no | Needs a modifier: none");
    expect(app.texts()).toContain("⌃⌥B");
    expect(said(app, "Type shortcut")).toBe(false);
    expect(said(app, "Esc to cancel · Delete to remove")).toBe(false);
    // Command and Shift come in the one order ctrl, alt, shift, cmd, whichever way they are written.
    await app.press("id:kit-shortcut");
    await app.keyboard.press("cmd+shift+f12");
    await app.waitFor("text:Shortcut: shift+cmd+f12 | Recording: no | Needs a modifier: none");
    expect(app.texts()).toContain("⇧⌘F12");
  });

  test(`${size.name}: Escape cancels and keeps the shortcut; Delete and Backspace remove it; the remove button removes it`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    await app.press("id:kit-shortcut");
    await app.waitFor("text:Type shortcut");
    await app.keyboard.press("escape");
    await app.waitFor("text:Shortcut: ctrl+alt+e | Recording: no | Needs a modifier: none");
    expect(app.texts()).toContain("⌃⌥E");

    await app.press("id:kit-shortcut");
    await app.waitFor("text:Type shortcut");
    await app.keyboard.press("delete");
    await app.waitFor("text:Shortcut: none | Recording: no | Needs a modifier: none");
    expect(app.texts()).not.toContain("⌃⌥E");
    expect(app.shows("id:kit-shortcut-clear")).toBe(false);

    await app.press("id:kit-shortcut");
    await app.keyboard.press("ctrl+alt+h");
    await app.waitFor("text:Shortcut: ctrl+alt+h | Recording: no | Needs a modifier: none");
    await app.press("id:kit-shortcut");
    await app.waitFor("text:Type shortcut");
    await app.keyboard.press("backspace");
    await app.waitFor("text:Shortcut: none | Recording: no | Needs a modifier: none");

    await app.press("id:kit-shortcut");
    await app.keyboard.press("alt+k");
    await app.waitFor("text:Shortcut: alt+k | Recording: no | Needs a modifier: none");
    await app.press("id:kit-shortcut-clear");
    await app.waitFor("text:Shortcut: none | Recording: no | Needs a modifier: none");
  });

  test(`${size.name}: a chord the rule refuses is answered in words and the box keeps recording`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    await app.press("id:kit-shortcut-bare");
    await app.waitFor("text:Type shortcut");
    // The new keys too: a bare space bar, arrow, punctuation key or F13 is answered in words (and never taken as the shortcut).
    for (const bare of ["b", "shift+b", "f5", "shift+7", "space", "left", "shift+left", "minus", "[", "f13", "shift+,"]) {
      await app.keyboard.press(bare);
      await app.waitFor("text:Add Control, Option or Command.");
      expect(said(app, "Type shortcut")).toBe(true);
      expect(readout(app, "Shortcut:")).toContain("Needs a modifier: none");
    }
    await app.keyboard.press("ctrl+b");
    await app.waitFor("text:/Needs a modifier: ctrl\\+b$/");
    expect(said(app, "Type shortcut")).toBe(false);
    expect(said(app, "Add Control, Option or Command.")).toBe(false);
  });

  test(`${size.name}: the space bar, the arrows, page keys, punctuation and F13 to F20 are recorded too, one spelling each`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    // One of each kind: the space bar, an arrow, home/end/page keys, the minus and plus keys, the brackets, backslash, the quote keys, comma,
    // period, slash, semicolon, the backtick, and the function keys above F12; with Shift and Cmd where they come in the one order.
    const chords = [
      "ctrl+alt+space", "ctrl+alt+left", "alt+up", "shift+cmd+down", "ctrl+alt+right", "ctrl+home", "alt+end", "ctrl+alt+pageup", "alt+pagedown",
      "ctrl+alt+minus", "ctrl+alt+plus", "ctrl+alt+[", "ctrl+alt+]", "ctrl+alt+\\", "ctrl+alt+;", "ctrl+alt+'", "alt+,", "cmd+.", "ctrl+alt+/",
      "ctrl+alt+`", "ctrl+alt+f13", "cmd+f20",
    ];
    for (const chord of chords) {
      await app.press("id:kit-shortcut");
      await app.waitFor("text:Type shortcut");
      await app.keyboard.press(chord);
      await app.waitFor(`text:Shortcut: ${chord} | Recording: no | Needs a modifier: none`);
      expect(said(app, "Type shortcut")).toBe(false);
      expect(app.texts()).toContain(keymap.label(chord));
    }
    // Written another way, the same key comes back as the recorder spells it (the recorder binds one spelling of each).
    await app.press("id:kit-shortcut");
    await app.keyboard.press("ctrl+alt+arrowleft");
    await app.waitFor("text:Shortcut: ctrl+alt+left | Recording: no | Needs a modifier: none");
    await app.press("id:kit-shortcut");
    await app.keyboard.press("ctrl+alt+-");
    await app.waitFor("text:Shortcut: ctrl+alt+minus | Recording: no | Needs a modifier: none");
    await app.press("id:kit-shortcut");
    await app.keyboard.press("ctrl+alt+=");
    await app.waitFor("text:Shortcut: ctrl+alt+plus | Recording: no | Needs a modifier: none");
  });

  test(`${size.name}: a new key that needs a modifier is refused without one and taken with one`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    await app.press("id:kit-shortcut-bare");
    await app.waitFor("text:Type shortcut");
    for (const bare of ["space", "right", "pagedown", ";", "f20"]) {
      await app.keyboard.press(bare);
      await app.waitFor("text:Add Control, Option or Command.");
      expect(readout(app, "Shortcut:")).toContain("Needs a modifier: none");
    }
    await app.keyboard.press("alt+space");
    await app.waitFor("text:/Needs a modifier: alt\\+space$/");
    expect(said(app, "Type shortcut")).toBe(false);
  });

  test(`${size.name}: the keys the platform keeps are not taken, and a chord pressed after recording ended does nothing`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    await app.press("id:kit-shortcut");
    await app.waitFor("text:Type shortcut");
    // Command+Q is the system's (quit): it is not bound, so it is not a chord and the box still waits.
    await app.keyboard.press("mod+q");
    expect(said(app, "Type shortcut")).toBe(true);
    expect(state(app)).toBe("Shortcut: ctrl+alt+e | Recording: yes | Needs a modifier: none");
    await app.keyboard.press("escape");
    await app.waitFor("text:Shortcut: ctrl+alt+e | Recording: no | Needs a modifier: none");
    // Nothing is left to act: chords, Escape, Delete and Tab change nothing now (computer.test.tsx shows the app's own Cmd+K and Escape working again).
    for (const chord of ["ctrl+alt+z", "cmd+k", "delete", "backspace", "escape", "tab"]) await app.keyboard.press(chord);
    expect(state(app)).toBe(LINE);
    expect(said(app, "Type shortcut")).toBe(false);
    // And a second recording works the same (the keys are bound again).
    await app.press("id:kit-shortcut");
    await app.waitFor("text:Type shortcut");
    await app.keyboard.press("ctrl+alt+z");
    await app.waitFor("text:Shortcut: ctrl+alt+z | Recording: no | Needs a modifier: none");
  });

  test(`${size.name}: switching to another app ends the recording as Escape does: nothing stored, nothing left bound`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    // "inactive" is another app or window holding the keys; "background" is not visible at all. Either one ends it.
    for (const away of ["inactive", "background"] as const) {
      await app.press("id:kit-shortcut");
      await app.waitFor("text:Type shortcut");
      expect(state(app)).toBe("Shortcut: ctrl+alt+e | Recording: yes | Needs a modifier: none");
      await app.setLifecycle(away);
      await app.waitFor("text:Shortcut: ctrl+alt+e | Recording: no | Needs a modifier: none");
      expect(said(app, "Type shortcut")).toBe(false);
      expect(said(app, "Esc to cancel · Delete to remove")).toBe(false);
      expect(app.texts()).toContain("⌃⌥E");
      await app.setLifecycle("active");
      // The keys are not bound any more: the chord that was being waited for does nothing now.
      for (const chord of ["ctrl+alt+z", "delete", "escape"]) await app.keyboard.press(chord);
      expect(state(app)).toBe(LINE);
    }
    // Coming back does not start it again, and the next recording works as before.
    expect(said(app, "Type shortcut")).toBe(false);
    await app.press("id:kit-shortcut");
    await app.waitFor("text:Type shortcut");
    await app.keyboard.press("ctrl+alt+z");
    await app.waitFor("text:Shortcut: ctrl+alt+z | Recording: no | Needs a modifier: none");
  });

  test(`${size.name}: Tab, or a press anywhere else, ends the recording and keeps the shortcut`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    await app.press("id:kit-shortcut");
    await app.waitFor("text:Type shortcut");
    await app.keyboard.press("tab");
    await app.waitFor("text:Shortcut: ctrl+alt+e | Recording: no | Needs a modifier: none");

    await app.press("id:kit-shortcut");
    await app.waitFor("text:Type shortcut");
    // A press on another control ends it (and reaches that control: the Primary button counts it).
    await app.press("id:kit-primary");
    await app.waitFor("text:Shortcut: ctrl+alt+e | Recording: no | Needs a modifier: none");
    expect(said(app, "Type shortcut")).toBe(false);
    expect(app.texts()).toContain("Presses: 1");
    // Nothing is left bound after that either.
    await app.keyboard.press("ctrl+alt+z");
    expect(state(app)).toBe(LINE);
  });
}
