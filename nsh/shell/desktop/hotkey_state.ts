// Where the global hotkey stands, and whether something holds it off. A leaf module (nothing of the app is imported), so Settings can read
// the status and hold the hotkey off without a cycle through the desktop host, which writes the status and follows the hold.
import { signal } from "nsh";
import type { HotkeyStatus } from "./hotkey";

/** Where the global hotkey stands (written by DesktopHost; "off" where there is no desktop host: a headless test, the phone). */
const status = signal<HotkeyStatus>("off");
/** How many holds are on: while one is, the host lets the system have the chord back. */
const holds = signal(0);

/** The global hotkey's status: off, pending, registered, refused, invalid. Read inside JSX or a closure. */
export const hotkeyStatus = (): HotkeyStatus => status();

/** DesktopHost tells the status as the hotkey's own state changes. */
export function setHotkeyStatus(next: HotkeyStatus): void {
  status.set(next);
}

/** Whether the hotkey is held off now. */
export const hotkeyHeld = (): boolean => holds() > 0;

/**
 * Holds the global hotkey off: DesktopHost reads it as an empty combo, so the chord is given back to the system until every hold is
 * released (a chord pressed meanwhile reaches the app instead of hiding its window). Returns the release, which does it once.
 * Settings holds it while its shortcut recorder listens.
 */
export function holdHotkey(): () => void {
  let released = false;
  holds.update((n) => n + 1);
  return () => {
    if (released) return;
    released = true;
    holds.update((n) => n - 1);
  };
}
