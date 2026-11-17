// The global hotkey: followed from the stored setting, replaced when it changes, off when it is empty; and the host's combo rules
// (src/desktop.rs, which bir-app-api's hotkey_combo feeds) reached through the door, so the Settings page and the listener agree.
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { HOTKEY_KEYS, HOTKEY_NSH_KEYS } from "../../lib/settings_model";
import { createHotkey, type HotkeyDeps } from "./hotkey";
import { holdHotkey, hotkeyHeld, hotkeyStatus, setHotkeyStatus } from "./hotkey_state";
import { hotkeyShortcut } from "./native";

/** A fake GlobalShortcut: the system takes every chord but the ones the test says another app holds. */
function world(held: string[] = []) {
  const log: string[] = [];
  const registered = new Map<string, () => void>();
  const deps: HotkeyDeps = {
    shortcutFor: (combo) => (combo === "control+notakey" ? null : combo.toLowerCase().replace("control", "ctrl")),
    register: async (shortcut, onPress) => {
      log.push("register " + shortcut);
      if (held.includes(shortcut)) return false;
      registered.set(shortcut, onPress);
      return true;
    },
    unregister: (shortcut) => {
      log.push("unregister " + shortcut);
      registered.delete(shortcut);
    },
    onPress: () => log.push("pressed"),
  };
  return { deps, log, registered };
}

test("a stored combo is registered, and pressing it runs the toggle", async () => {
  const w = world();
  const hotkey = createHotkey(w.deps);
  expect(await hotkey.sync("control+alt+E")).toBe("registered");
  expect(w.log).toEqual(["register ctrl+alt+e"]);
  w.registered.get("ctrl+alt+e")?.();
  expect(w.log[w.log.length - 1]).toBe("pressed");
  expect(hotkey.status()).toBe("registered");
});

test("the same combo again changes nothing; another one replaces it; an empty one turns it off", async () => {
  const w = world();
  const hotkey = createHotkey(w.deps);
  await hotkey.sync("control+alt+E");
  await hotkey.sync("control+alt+E");
  expect(w.log).toEqual(["register ctrl+alt+e"]);
  expect(await hotkey.sync("control+alt+B")).toBe("registered");
  expect(w.log.slice(1)).toEqual(["unregister ctrl+alt+e", "register ctrl+alt+b"]);
  expect(await hotkey.sync("")).toBe("off");
  expect(await hotkey.sync(null)).toBe("off");
  expect(await hotkey.sync("  ")).toBe("off");
  expect(w.log.slice(3)).toEqual(["unregister ctrl+alt+b"]);
  expect([...w.registered.keys()]).toEqual([]);
});

test("a chord another app holds is refused, said so, and not asked again until the setting changes", async () => {
  const w = world(["ctrl+alt+e"]);
  const notes: string[] = [];
  const hotkey = createHotkey({ ...w.deps, log: (level, message) => notes.push(level + ": " + message) });
  expect(await hotkey.sync("control+alt+E")).toBe("refused");
  expect(await hotkey.sync("control+alt+E")).toBe("refused");
  expect(w.log).toEqual(["register ctrl+alt+e"]);
  expect(notes).toEqual(["warn: could not register global hotkey 'control+alt+E'"]);
  expect(await hotkey.sync("control+alt+H")).toBe("registered");
});

test("a combo the host cannot register is invalid, and a newer setting replaces a chord the system has not answered yet", async () => {
  const w = world();
  const hotkey = createHotkey(w.deps);
  expect(await hotkey.sync("control+notakey")).toBe("invalid");
  expect(w.log).toEqual([]);
  // Two changes in a row (the setting changed twice before the first register answered): the last one wins, nothing is left behind.
  const first = hotkey.sync("control+alt+E");
  const second = hotkey.sync("control+alt+B");
  await first;
  expect(await second).toBe("registered");
  expect([...w.registered.keys()]).toEqual(["ctrl+alt+b"]);
  hotkey.stop();
  expect([...w.registered.keys()]).toEqual([]);
  expect(hotkey.status()).toBe("off");
});

test("the host's combo rules reach TSX: what Settings stores, GlobalShortcut can take", () => {
  expect(hotkeyShortcut("control+alt+E")).toBe("ctrl+alt+e");
  expect(hotkeyShortcut("super+shift+F12")).toBe("shift+cmd+f12");
  expect(hotkeyShortcut("control+notakey")).toBeNull();
  expect(hotkeyShortcut("shift+E")).toBeNull();
  expect(hotkeyShortcut("")).toBeNull();
  // The chords the page offers (bir.system_info().hotkey_presets) all map.
  const info = bir.system_info();
  expect(info.hotkey_presets.length > 0).toBe(true);
  for (const preset of info.hotkey_presets) expect(hotkeyShortcut(preset.value)).not.toBeNull();
});

test("the page and the host agree: whatever the host can map the page stores, and what the page refuses the host does not map", () => {
  const combos = [
    "control+alt+E", "control+alt+F12", "super+shift+B", "control+alt+notakey", "control+alt+F13", "control+alt+space", "alt+9", "control+alt+0",
    "control+alt+F21", "control+alt+-", "control+alt+TAB", "shift+SPACE", "control+alt+COMMA",
  ];
  for (const combo of combos) {
    let stored = true;
    try {
      bir.set_global_hotkey(combo);
    } catch {
      stored = false;
    }
    expect(stored).toBe(hotkeyShortcut(combo) !== null);
  }
  bir.set_global_hotkey("");
  expect(bir.system_info().global_hotkey).toBeNull();
});

test("F19: the keys only this app registers: the host maps and stores each stored word, and the page's table spells them the same", () => {
  expect(Object.keys(HOTKEY_NSH_KEYS).length).toBe(28);
  for (const [word, key] of Object.entries(HOTKEY_NSH_KEYS)) {
    const combo = `control+alt+${word}`;
    expect(hotkeyShortcut(combo)).toBe(`ctrl+alt+${key}`);
    // The same word in lower case, as a hand-written setting could have it.
    expect(hotkeyShortcut(combo.toLowerCase())).toBe(`ctrl+alt+${key}`);
    bir.set_global_hotkey(combo);
    expect(bir.system_info().global_hotkey).toBe(combo);
  }
  // The old keys keep their old spelling (the lower-case word).
  for (const word of HOTKEY_KEYS) expect(hotkeyShortcut(`alt+${word}`)).toBe(`alt+${word.toLowerCase()}`);
  // A refused combo leaves what was stored.
  bir.set_global_hotkey("control+alt+SPACE");
  for (const refused of ["control+alt+F21", "control+alt+TAB", "control+alt+ENTER", "control+alt+-", "control+alt+[", "shift+SPACE", "shift+F20"]) {
    expect(() => bir.set_global_hotkey(refused)).toThrow(/not one this computer can register/);
    expect(hotkeyShortcut(refused)).toBeNull();
  }
  expect(bir.system_info().global_hotkey).toBe("control+alt+SPACE");
  bir.set_global_hotkey("");
  expect(bir.system_info().global_hotkey).toBeNull();
});

test("a chord the system answers for late is given back when the setting moved on; the same chord again is kept", async () => {
  const answers: ((took: boolean) => void)[] = [];
  const held = new Set<string>();
  const hotkey = createHotkey({
    shortcutFor: (combo) => combo.toLowerCase(),
    register: (shortcut) =>
      new Promise<boolean>((done) => {
        answers.push((took) => {
          if (took) held.add(shortcut);
          done(took);
        });
      }),
    unregister: (shortcut) => {
      held.delete(shortcut);
    },
    onPress: () => {},
  });
  const states: string[] = [];
  const watching = createHotkey({ shortcutFor: (c) => c, register: async () => true, unregister: () => {}, onPress: () => {}, onStatus: (s) => states.push(s) });
  await watching.sync("a");
  await watching.sync("");
  expect(states).toEqual(["pending", "registered", "off"]);

  // E asked, then B asked, then the system answers E (late) and B.
  const first = hotkey.sync("E");
  const second = hotkey.sync("B");
  answers[0](true);
  await first;
  expect([...held]).toEqual([]); // E was given back at once
  answers[1](true);
  expect(await second).toBe("registered");
  expect([...held]).toEqual(["b"]);

  // Off, then the same chord again before the first answer: the one register in flight is the chord that is wanted, so it stays
  // (no second register is asked beside it: the system would refuse the chord it already gave us).
  await hotkey.sync("");
  expect([...held]).toEqual([]);
  const third = hotkey.sync("C");
  const fourth = hotkey.sync("");
  const fifth = hotkey.sync("C");
  expect(answers.length).toBe(3);
  answers[2](true);
  await Promise.all([third, fourth, fifth]);
  expect(answers.length).toBe(3);
  expect([...held]).toEqual(["c"]);
  expect(hotkey.status()).toBe("registered");
});

/**
 * A system as the real one behaves: a chord this app already holds is refused when asked again, and a call that starts while
 * another is still waiting for its answer is counted (`overlaps`).
 */
function strictSystem() {
  const holds = new Set<string>();
  const waiting: ((took: boolean) => void)[] = [];
  const calls: string[] = [];
  let inFlight = 0;
  let overlaps = 0;
  const deps: HotkeyDeps = {
    shortcutFor: (combo) => combo.toLowerCase(),
    register: (shortcut) => {
      calls.push("register " + shortcut);
      if (inFlight > 0) overlaps++;
      inFlight++;
      return new Promise<boolean>((done) => {
        waiting.push((took) => {
          inFlight--;
          const answer = took && !holds.has(shortcut);
          if (answer) holds.add(shortcut);
          done(answer);
        });
      });
    },
    unregister: (shortcut) => {
      calls.push("unregister " + shortcut);
      if (inFlight > 0) overlaps++;
      holds.delete(shortcut);
    },
    onPress: () => {},
  };
  return { deps, holds, waiting, calls, overlaps: () => overlaps };
}

test("F8-11: a lock and an unlock while the system is still answering leave one chord, tracked, and nothing held when it is off", async () => {
  const system = strictSystem();
  const hotkey = createHotkey(system.deps);
  // Unlocked: the chord is asked. Locked, unlocked again, both before the system answered.
  const first = hotkey.sync("C");
  const lock = hotkey.sync("");
  const unlock = hotkey.sync("C");
  expect(system.waiting.length).toBe(1);
  system.waiting[0](true);
  expect(await Promise.all([first, lock, unlock])).toEqual(["registered", "registered", "registered"]);
  expect([...system.holds]).toEqual(["c"]);
  // The next lock gives it back: it was tracked.
  expect(await hotkey.sync("")).toBe("off");
  expect([...system.holds]).toEqual([]);
  expect(system.overlaps()).toBe(0);
});

test("F8-11: a lock while the system is still answering gives the chord back when the answer comes; one call at a time throughout", async () => {
  const system = strictSystem();
  const hotkey = createHotkey(system.deps);
  const unlocked = hotkey.sync("C");
  const locked = hotkey.sync("");
  system.waiting[0](true);
  expect(await locked).toBe("off");
  await unlocked;
  expect([...system.holds]).toEqual([]);
  expect(hotkey.status()).toBe("off");
  expect(system.calls).toEqual(["register c", "unregister c"]);
  // Another chord while one is being asked: it waits its turn, and the first is given back before the second is asked.
  const e = hotkey.sync("E");
  const b = hotkey.sync("B");
  expect(system.waiting.length).toBe(2);
  system.waiting[1](true);
  await e;
  expect([...system.holds]).toEqual([]);
  expect(system.waiting.length).toBe(3);
  system.waiting[2](true);
  expect(await b).toBe("registered");
  expect([...system.holds]).toEqual(["b"]);
  expect(system.calls.slice(2)).toEqual(["register e", "unregister e", "register b"]);
  expect(system.overlaps()).toBe(0);
});

test("F8-11: stop while the system is answering still gives the chord back", async () => {
  const system = strictSystem();
  const hotkey = createHotkey(system.deps);
  const asked = hotkey.sync("C");
  hotkey.stop();
  system.waiting[0](true);
  await asked;
  expect([...system.holds]).toEqual([]);
  expect(hotkey.status()).toBe("off");
  hotkey.stop();
  expect(system.overlaps()).toBe(0);
});

test("F17: a hold is counted, let go once, and the status Settings reads is the one the host wrote", () => {
  expect(hotkeyHeld()).toBe(false);
  expect(hotkeyStatus()).toBe("off");
  const first = holdHotkey();
  const second = holdHotkey();
  expect(hotkeyHeld()).toBe(true);
  first();
  first(); // a second release of the same hold counts for nothing
  expect(hotkeyHeld()).toBe(true);
  second();
  expect(hotkeyHeld()).toBe(false);
  setHotkeyStatus("refused");
  expect(hotkeyStatus()).toBe("refused");
  setHotkeyStatus("off");
});

test("F17: the hotkey follows an empty combo while it is held, so the chord goes back to the system, and takes the combo again when the hold ends", async () => {
  // What DesktopHost's effect does: the combo given to the hotkey is "" while a hold is on.
  const w = world();
  const hotkey = createHotkey(w.deps);
  const stored = "control+alt+E";
  const follow = () => hotkey.sync(hotkeyHeld() ? "" : stored);
  expect(await follow()).toBe("registered");
  expect([...w.registered.keys()]).toEqual(["ctrl+alt+e"]);
  const release = holdHotkey();
  expect(await follow()).toBe("off");
  expect([...w.registered.keys()]).toEqual([]);
  expect(w.log).toEqual(["register ctrl+alt+e", "unregister ctrl+alt+e"]);
  release();
  expect(await follow()).toBe("registered");
  expect([...w.registered.keys()]).toEqual(["ctrl+alt+e"]);
  hotkey.stop();
});
