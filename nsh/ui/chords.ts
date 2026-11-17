// The shortcut recorder's pure rules: the chords it listens for, and how the ones the platform lets an app bind are found
// with few calls. No nsh import (the part itself, shortcut.tsx, holds the keymap).

/**
 * The keys beyond letters, digits and f1 to f12 that a recorded chord can end in, as nsh/keymap spells them (website/modules/nsh.md,
 * "Keystrokes"): the space bar, the four arrows, home, end, page up and down, the punctuation keys as the character they type without
 * Shift on a US keyboard (`[` `]` `\` `;` `'` `,` `.` `/` and the backtick), the minus key (`minus`), the one typing `=` (`plus`: keymap's
 * own name for it, so "ctrl+plus" and never a bare "+", which is the separator) and f13 to f20 (macOS keyboards end at f20, so f21 to f24,
 * which keymap reads, are not offered: the global shortcut cannot register them there). Each of them binds, and registers as a global
 * shortcut, in a real window on macOS (shots/f19.test.tsx); Windows and Linux read the same names.
 *
 * Not offered, because the recorder keeps them for itself: escape, tab, backspace and delete (cancel, move on, remove). Enter and Insert
 * are left out too (not asked for: a Mac keyboard has no Insert, and Enter is one of the two keys that start a recording).
 */
export const EXTRA_KEYS: readonly string[] = [
  "space",
  "up",
  "down",
  "left",
  "right",
  "home",
  "end",
  "pageup",
  "pagedown",
  "minus",
  "plus",
  "[",
  "]",
  "\\",
  ";",
  "'",
  ",",
  ".",
  "/",
  "`",
  ...Array.from({ length: 8 }, (_, i) => `f${i + 13}`),
];

/** The keys a recorded chord can end in, as nsh/keymap names them: a to z, 0 to 9, f1 to f12 (48), then `EXTRA_KEYS` (28). */
export const CHORD_KEYS: readonly string[] = [
  ..."abcdefghijklmnopqrstuvwxyz".split(""),
  ..."0123456789".split(""),
  ...Array.from({ length: 12 }, (_, i) => `f${i + 1}`),
  ...EXTRA_KEYS,
];

/** The modifiers in the order a chord is written (and so the order a recorded keystroke has them). */
export const CHORD_MODIFIERS: readonly string[] = ["ctrl", "alt", "shift", "cmd"];

/** The keys that end or clear a recording instead of making a chord. */
export const CANCEL_KEY = "escape";
export const CLEAR_KEYS: readonly string[] = ["backspace", "delete"];
/** The keys that move the focus on (and end a recording, which they do not take: the focus still moves). */
export const LEAVE_KEYS: readonly string[] = ["tab", "shift+tab"];

/** Every chord a person could mean: each subset of the four modifiers with each key (16 x 76 = 1216), as "ctrl+alt+e". */
export function allChords(): string[] {
  const out: string[] = [];
  for (let set = 0; set < 1 << CHORD_MODIFIERS.length; set++) {
    const mods = CHORD_MODIFIERS.filter((_, i) => (set & (1 << i)) !== 0);
    for (const key of CHORD_KEYS) out.push([...mods, key].join("+"));
  }
  return out;
}

/** The keystroke's modifiers (in the order given) and its key. */
export function splitChord(keystroke: string): { mods: string[]; key: string } {
  const parts = keystroke.split("+");
  return { mods: parts.slice(0, -1), key: parts[parts.length - 1] ?? "" };
}

/** What `bindAvailable` did: the keystrokes bound, the ones the platform keeps, how many binds it took, and the way to unbind all. */
export interface Bound {
  bound: string[];
  kept: string[];
  calls: number;
  unbind: () => void;
}

/**
 * Binds every keystroke of `keys` the platform allows with as few calls as it can. `bindGroup` binds a whole group in one call (keymap.bind
 * with a record sends the platform its table once, so one call for many keystrokes is cheap) and returns its unbind; it throws when any one
 * of the group is a keystroke the platform keeps (keymap.bind checks the whole record before it binds any, so a refused group binds
 * nothing). A refused group is halved until the keystrokes that are kept stand alone: for 5 kept of 1219, 57 calls (49 for the three kept elsewhere), not 1219.
 */
export function bindAvailable(keys: readonly string[], bindGroup: (group: string[]) => () => void): Bound {
  const unbinds: (() => void)[] = [];
  const bound: string[] = [];
  const kept: string[] = [];
  let calls = 0;
  const walk = (group: string[]): void => {
    if (!group.length) return;
    calls++;
    try {
      unbinds.push(bindGroup(group));
      bound.push(...group);
    } catch {
      if (group.length === 1) kept.push(group[0]);
      else {
        const half = group.length >> 1;
        walk(group.slice(0, half));
        walk(group.slice(half));
      }
    }
  };
  walk([...keys]);
  return {
    bound,
    kept,
    calls,
    unbind: () => {
      for (const off of unbinds.splice(0)) off();
    },
  };
}
