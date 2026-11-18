// The user's quit and hide keys and the close button's remembered choice (Settings > Keyboard), and the capture a Quit or Hide key
// recorder takes of the menu. A leaf (nothing of the app is imported), so Settings, the menu and the desktop host share it without a cycle.
//
// Kept in NSH Storage under "desktop.close" for the whole computer (not per database, as redesign.prefs): read when the desktop host
// starts, so the keys also hold on the sign-in screens. A change made before the read came back is not overwritten by it.
import { signal, Storage } from "nsh";
import { DEFAULT_CLOSE_PREFS, parseClosePrefs, type AppKeys, type CloseChoice, type ClosePrefs } from "./menu_model";

/** The Storage key. */
export const CLOSE_PREFS_KEY = "desktop.close";

const prefs = signal<ClosePrefs>(DEFAULT_CLOSE_PREFS);
let loading: Promise<void> | null = null;
let changed = false;

/** Reads the stored choices once (later calls get the same promise). */
export function loadClosePrefs(): Promise<void> {
  loading ??= Storage.getItem(CLOSE_PREFS_KEY)
    .then((raw) => {
      if (!changed) prefs.set(parseClosePrefs(raw));
    })
    .catch(() => {});
  return loading;
}

/** The choices now (reactive). */
export const closePrefs = (): ClosePrefs => prefs();

function save(next: ClosePrefs): void {
  changed = true;
  prefs.set(next);
  Storage.setItem(CLOSE_PREFS_KEY, JSON.stringify({ quit: next.keys.quit, hide: next.keys.hide, close: next.close })).catch(() => {});
}

/** Sets the quit or the hide key ("" for none). The caller has checked it is free (menu_model keyHolder). */
export function setAppKey(which: keyof AppKeys, keystroke: string): void {
  const now = prefs();
  save({ ...now, keys: { ...now.keys, [which]: keystroke } });
}

/** Sets what Windows' and Linux's close button does: ask, or hide or quit without asking. */
export function setCloseChoice(close: CloseChoice): void {
  save({ ...prefs(), close });
}

// ---------------------------------------------------------------- the capture

/** Who hears the Quit and Hide items while a recorder listens (an object: a signal holds it as a value). */
const capture = signal<{ heard: (keystroke: string) => void } | null>(null);

/** A Quit or Hide key recorder listens (reactive): the menu carries the platform's kept keys on those two items. */
export const capturing = (): boolean => capture() !== null;

/** Hands what the Quit and Hide items hear to `heard` (the keystroke they carry) until the release, instead of quitting or hiding. */
export function captureMenuKeys(heard: (keystroke: string) => void): () => void {
  const mine = { heard };
  capture.set(mine);
  return () => {
    if (capture() === mine) capture.set(null);
  };
}

/** The recorder listening now, or null. */
export function captured(): ((keystroke: string) => void) | null {
  return capture()?.heard ?? null;
}
