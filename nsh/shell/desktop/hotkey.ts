// The global show/hide hotkey: registered from the stored setting, registered again when the setting changes, off when it is empty
// (old tray.rs HotkeyWatch). Pure over its dependencies, so a plain test plays the OS refusing a chord.

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
  /** Follows the stored combo: "control+alt+E", or empty for none. A newer call replaces one the system has not answered yet. */
  sync(combo: string | null | undefined): Promise<HotkeyStatus>;
  status(): HotkeyStatus;
  /** Gives the keystroke back (the app is closing). */
  stop(): void;
}

/** The hotkey of one app. */
export function createHotkey(deps: HotkeyDeps): Hotkey {
  const log = deps.log ?? (() => {});
  let wanted = "";
  let current: string | null = null;
  let status: HotkeyStatus = "off";
  /** Each change of the combo is a generation: a register that answers after a newer one began is given back. */
  let generation = 0;
  let latest: string | null = null;
  const change = (next: HotkeyStatus): HotkeyStatus => {
    status = next;
    deps.onStatus?.(next);
    return next;
  };

  async function apply(combo: string): Promise<HotkeyStatus> {
    // The same combo again changes nothing (a refused or invalid one is not asked again until the setting changes).
    if (combo === wanted) return status;
    wanted = combo;
    const mine = ++generation;
    latest = null;
    if (current !== null) {
      deps.unregister(current);
      current = null;
    }
    if (combo === "") return change("off");
    const shortcut = deps.shortcutFor(combo);
    if (shortcut === null) {
      log("warn", "invalid global hotkey combo '" + combo + "'");
      return change("invalid");
    }
    let took = false;
    latest = shortcut;
    change("pending");
    try {
      took = await deps.register(shortcut, deps.onPress);
    } catch (error) {
      log("warn", "could not register global hotkey '" + combo + "': " + String(error));
    }
    if (mine !== generation) {
      // The setting moved on while the system thought about it (it may take a while: a desktop portal asks the person). Give it back,
      // unless the newer combo is the same chord, which this registration already is.
      if (took && latest !== shortcut) deps.unregister(shortcut);
      return status;
    }
    if (!took) {
      log("warn", "could not register global hotkey '" + combo + "'");
      return change("refused");
    }
    current = shortcut;
    log("info", "global show hotkey registered: " + combo);
    return change("registered");
  }

  return {
    sync(combo) {
      return apply((combo ?? "").trim());
    },
    status: () => status,
    stop() {
      if (current !== null) deps.unregister(current);
      current = null;
      wanted = "";
      generation++;
      change("off");
    },
  };
}
