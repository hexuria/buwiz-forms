// The recorder's pure rules: the 768 chords, and finding the ones the platform lets an app bind with few calls.
import { expect, test } from "nsh/test";
import { allChords, bindAvailable, CHORD_KEYS, CHORD_MODIFIERS, splitChord } from "./chords";

test("every subset of ctrl, alt, shift and cmd with each of the 48 keys, modifiers in one order", () => {
  const chords = allChords();
  expect(CHORD_KEYS.length).toBe(48);
  expect(chords.length).toBe(16 * 48);
  expect(new Set(chords).size).toBe(chords.length);
  expect(chords).toContain("a");
  expect(chords).toContain("ctrl+alt+e");
  expect(chords).toContain("ctrl+alt+shift+cmd+f12");
  expect(chords).toContain("cmd+0");
  // Modifiers always come in the order ctrl, alt, shift, cmd, so a recorded keystroke is one spelling.
  for (const chord of chords) {
    const { mods, key } = splitChord(chord);
    expect(CHORD_KEYS).toContain(key);
    expect(mods).toEqual(CHORD_MODIFIERS.filter((m) => mods.includes(m)));
  }
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
