// The recorder's pure rules: the 1216 chords, and finding the ones the platform lets an app bind with few calls.
import { keymap } from "nsh";
import { expect, test } from "nsh/test";
import { allChords, bindAvailable, CANCEL_KEY, CHORD_KEYS, CHORD_MODIFIERS, CLEAR_KEYS, EXTRA_KEYS, LEAVE_KEYS, splitChord } from "./chords";

test("every subset of ctrl, alt, shift and cmd with each of the 76 keys, modifiers in one order", () => {
  const chords = allChords();
  expect(CHORD_KEYS.length).toBe(76);
  expect(chords.length).toBe(16 * 76);
  expect(new Set(chords).size).toBe(chords.length);
  expect(chords).toContain("a");
  expect(chords).toContain("ctrl+alt+e");
  expect(chords).toContain("ctrl+alt+shift+cmd+f12");
  expect(chords).toContain("cmd+0");
  expect(chords).toContain("alt+space");
  expect(chords).toContain("ctrl+alt+left");
  expect(chords).toContain("shift+cmd+,");
  expect(chords).toContain("ctrl+alt+shift+cmd+f20");
  // Modifiers always come in the order ctrl, alt, shift, cmd, so a recorded keystroke is one spelling.
  for (const chord of chords) {
    const { mods, key } = splitChord(chord);
    expect(CHORD_KEYS).toContain(key);
    expect(mods).toEqual(CHORD_MODIFIERS.filter((m) => mods.includes(m)));
  }
});

test("the keys beyond letters, digits and f1 to f12: space, arrows, home, end, page up and down, punctuation, f13 to f20", () => {
  expect(EXTRA_KEYS).toEqual([
    "space", "up", "down", "left", "right", "home", "end", "pageup", "pagedown",
    "minus", "plus", "[", "]", "\\", ";", "'", ",", ".", "/", "`",
    "f13", "f14", "f15", "f16", "f17", "f18", "f19", "f20",
  ]);
  expect(CHORD_KEYS.slice(-EXTRA_KEYS.length)).toEqual(EXTRA_KEYS);
  expect(new Set(CHORD_KEYS).size).toBe(CHORD_KEYS.length);
  // The recorder keeps these for itself, and "+" would be read as the separator.
  const keptForTheRecorder = [CANCEL_KEY, ...CLEAR_KEYS, ...LEAVE_KEYS, "tab", "+", "enter", "return", "insert"];
  for (const key of keptForTheRecorder) expect(CHORD_KEYS).not.toContain(key);
  for (const key of CHORD_KEYS) expect(key.includes("+")).toBe(false);
  // Each chord is one keymap reads: keymap.label throws a TypeError for a keystroke it cannot read (bindAvailable would take that for a key the platform keeps and leave it out without a word).
  for (const chord of allChords()) expect(() => keymap.label(chord)).not.toThrow();
});

test("splitChord", () => {
  expect(splitChord("ctrl+alt+e")).toEqual({ mods: ["ctrl", "alt"], key: "e" });
  expect(splitChord("f12")).toEqual({ mods: [], key: "f12" });
  expect(splitChord("")).toEqual({ mods: [], key: "" });
});

/** A fake keymap: a group with any kept keystroke is refused whole, and nothing of it is bound. */
function fake(kept: string[]) {
  const live = new Set<string>();
  const bindGroup = (group: string[]) => {
    const bad = group.find((k) => kept.includes(k));
    if (bad) throw new TypeError(`keymap.bind: "${bad}" cannot be bound`);
    for (const k of group) live.add(k);
    return () => {
      for (const k of group) live.delete(k);
    };
  };
  return { live, bindGroup };
}

test("the keystrokes the platform keeps are left out and every other one is bound, in few calls", () => {
  const keys = [...allChords(), "escape", "backspace", "delete"];
  // The recorder's own spelling puts modifiers in the fixed order (alt before cmd), which is how the platform's list reads here.
  const keptOnMac = ["cmd+q", "cmd+h", "alt+cmd+h", "cmd+m", "ctrl+cmd+f"];
  const world = fake(keptOnMac);
  const done = bindAvailable(keys, world.bindGroup);
  expect(done.kept.sort()).toEqual(keptOnMac.filter((k) => keys.includes(k)).sort());
  expect(done.bound.length + done.kept.length).toBe(keys.length);
  expect(world.live.size).toBe(done.bound.length);
  expect(done.calls < 120).toBe(true);
  done.unbind();
  expect(world.live.size).toBe(0);
  // Unbinding twice changes nothing.
  done.unbind();
  expect(world.live.size).toBe(0);
});

test("with the real keymap every chord binds but the few the platform keeps, whichever platform runs the test", () => {
  const keys = [...allChords(), CANCEL_KEY, ...CLEAR_KEYS];
  const reserved = ["cmd+q", "cmd+h", "alt+cmd+h", "cmd+m", "ctrl+cmd+f", "ctrl+q", "alt+f4", "f11"];
  const done = bindAvailable(keys, (group) => {
    const record: Record<string, () => void> = {};
    for (const key of group) record[key] = () => {};
    return keymap.bind(record, { when: "always" });
  });
  expect(done.kept.filter((key) => !reserved.includes(key))).toEqual([]);
  expect(done.bound.length + done.kept.length).toBe(keys.length);
  // Every new key binds with every modifier subset: none of them is a kept one.
  for (const key of EXTRA_KEYS) for (const chord of allChords().filter((c) => splitChord(c).key === key)) expect(done.bound).toContain(chord);
  expect(done.calls < 120).toBe(true);
  done.unbind();
});

test("nothing kept: one call; everything kept: nothing bound", () => {
  const keys = ["a", "b", "c"];
  const free = fake([]);
  expect(bindAvailable(keys, free.bindGroup).calls).toBe(1);
  expect(free.live.size).toBe(3);
  const shut = fake(keys);
  const done = bindAvailable(keys, shut.bindGroup);
  expect(done.bound).toEqual([]);
  expect(done.kept).toEqual(keys);
  expect(shut.live.size).toBe(0);
  expect(bindAvailable([], free.bindGroup).calls).toBe(0);
});
