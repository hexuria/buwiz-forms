// The global show/hide hotkey: registered from the stored setting, registered again when the setting changes, off when it is empty
// (old tray.rs HotkeyWatch). Pure over its dependencies, so a plain test plays the OS refusing a chord.
//
// One call into the system at a time. A register can take a while (a desktop portal asks the person), and the setting can change
// meanwhile (a lock and an unlock are two changes): a second register or an unregister started beside the pending one can cross it
// and leave the chord held by the system but unknown to us. So there is one state wanted (the last combo given to sync) and one loop
// that brings the system to it, a step at a time: give back the chord held if it is not the wanted one, then register the wanted one.

/** Where the hotkey stands: off (none stored), pending (the system has not answered yet), registered (the system took it), refused (another app holds the chord), invalid (not a combo this computer can register). */
export type HotkeyStatus = "off" | "pending" | "registered" | "refused" | "invalid";

/** What the hotkey needs of the world. */
export interface HotkeyDeps {
  /** The keystroke for a stored combo, or null (src/desktop.rs hotkey_shortcut: the host's combo rules). */
  shortcutFor(combo: string): string | null;
  /** GlobalShortcut.register. */
  register(shortcut: string, onPress: () => void): Promise<boolean>;
  /** GlobalShortcut.unregister. */
  unregister(shortcut: string): void;
  /** The hotkey was pressed: show the window, or hide it. */
  onPress(): void;
  /** Where a note about a refusal goes. */
  log?(level: "info" | "warn", message: string): void;
  /** Told each time the status changes (including "pending", while the system has not answered). */
  onStatus?(status: HotkeyStatus): void;
}

/** What createHotkey gives its owner. */
export interface Hotkey {
  /** Follows the stored combo: "control+alt+E", or empty for none. The answer is the status once the system has settled on it; a call a newer one replaced answers the status then. */
  sync(combo: string | null | undefined): Promise<HotkeyStatus>;
  status(): HotkeyStatus;
  /** Gives the keystroke back (the app is closing). */
  stop(): void;
}

/** The hotkey of one app. */
export function createHotkey(deps: HotkeyDeps): Hotkey {
  const log = deps.log ?? (() => {});
  /** The combo wanted now ("" for none); the last one sync was given. */
  let wanted = "";
  /** The chord the system holds for us (registered and not given back), or null. */
  let held: string | null = null;
  let status: HotkeyStatus = "off";
  /** The loop that brings the system to `wanted`, while it runs (one register or unregister in flight at most, never two loops). */
  let running: Promise<void> | null = null;
  /** The sync calls waiting for an answer: each is told when the system is settled on what was wanted when it asked, or sooner when a newer combo replaced it. */
  let waiting: { combo: string; done: (status: HotkeyStatus) => void }[] = [];
  const change = (next: HotkeyStatus): void => {
    status = next;
    deps.onStatus?.(next);
  };
  /** Answers the calls that asked for a combo that is not the wanted one any more (their register is over, or it is not asked at all). */
  const answerReplaced = (): void => {
    const gone = waiting.filter((w) => w.combo !== wanted);
    waiting = waiting.filter((w) => w.combo === wanted);
    for (const w of gone) w.done(status);
  };
  const answerAll = (): void => {
    const all = waiting;
    waiting = [];
    for (const w of all) w.done(status);
  };

  /** Brings the system to `wanted`, re-reading it after every step: a change during a step is picked up by the next turn of the loop. */
  async function settle(): Promise<void> {
    for (;;) {
      const combo = wanted;
      const shortcut = combo === "" ? null : deps.shortcutFor(combo);
      // Give back a chord that is not the wanted one: it was registered for a combo the setting has left (or the app is off).
      if (held !== null && held !== shortcut) {
        deps.unregister(held);
        held = null;
      }
      if (combo === "") return change("off");
      if (shortcut === null) {
        log("warn", "invalid global hotkey combo '" + combo + "'");
        return change("invalid");
      }
      if (held === shortcut) return change("registered");
      change("pending");
      let took = false;
      try {
        took = await deps.register(shortcut, deps.onPress);
      } catch (error) {
        log("warn", "could not register global hotkey '" + combo + "': " + String(error));
      }
      // Whatever the system took is ours to give back, even when the setting moved on while it thought: track it before anything else.
      if (took) held = shortcut;
      if (combo !== wanted) {
        answerReplaced();
        continue;
      }
      if (!took) {
        log("warn", "could not register global hotkey '" + combo + "'");
        return change("refused");
      }
      log("info", "global show hotkey registered: " + combo);
      return change("registered");
    }
  }

  function start(): void {
    if (running) return;
    running = settle()
      .catch((error) => log("warn", "global hotkey: " + String(error)))
      .then(() => {
        running = null;
        answerAll();
      });
  }

  return {
    sync(combo) {
      const next = (combo ?? "").trim();
      // The same combo again changes nothing (a refused or invalid one is not asked again until the setting changes).
      if (next === wanted && !running) return Promise.resolve(status);
      return new Promise<HotkeyStatus>((done) => {
        wanted = next;
        waiting.push({ combo: next, done });
        // A register still in flight: the status says what is wanted now (off: the chord is given back when the answer comes,
        // which a headless run never gives).
        if (running) change(next === "" ? "off" : "pending");
        start();
      });
    },
    status: () => status,
    stop() {
      wanted = "";
      // The loop still in flight gives back what its register brings; nothing in flight: give back what is held now.
      if (held !== null && !running) {
        deps.unregister(held);
        held = null;
      }
      change("off");
      if (running) answerReplaced();
    },
  };
}
