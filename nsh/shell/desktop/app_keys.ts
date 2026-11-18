// The user's quit and hide keys and the close button's remembered choice (Settings > Keyboard), and the capture a Quit or Hide key
// recorder takes of the menu. A leaf (nothing of the app is imported), so Settings, the menu and the desktop host share it without a cycle.
//
// Kept in NSH Storage under "desktop.close" for the whole computer (not per database, as redesign.prefs): read when the desktop host
// starts, so the keys also hold on the sign-in screens. A change made before the read came back is kept as a change: once the read is
// in, it is applied over what was stored and only then written, so it never overwrites the stored fields it did not touch.
import { signal, Storage } from "nsh";
import { DEFAULT_CLOSE_PREFS, parseClosePrefs, type AppKeys, type CloseChoice, type ClosePrefs } from "./menu_model";

/** The Storage key. */
export const CLOSE_PREFS_KEY = "desktop.close";

/** What the choices are read from and written to: NSH Storage (a test hands in a slower one). */
export interface ClosePrefsStorage {
  getItem(key: string): Promise<string | null>;
  setItem(key: string, value: string): Promise<void>;
}

let storage: ClosePrefsStorage = Storage;
const prefs = signal<ClosePrefs>(DEFAULT_CLOSE_PREFS);
let loading: Promise<void> | null = null;
/** The changes made before the read came back (null once it has): applied over the stored choices, then written. */
let early: ((now: ClosePrefs) => ClosePrefs)[] | null = [];

/** Reads the stored choices once (later calls get the same promise). */
export function loadClosePrefs(): Promise<void> {
  loading ??= storage
    .getItem(CLOSE_PREFS_KEY)
    .catch(() => null)
    .then((raw) => {
      const changes = early ?? [];
      early = null;
      const next = changes.reduce((now, change) => change(now), parseClosePrefs(raw));
      prefs.set(next);
      if (changes.length) write(next);
    });
  return loading;
}

/** For a test: reads and writes go to `store` from now on, as if the app had just started (nothing read yet). */
export function useClosePrefsStorage(store: ClosePrefsStorage): void {
  storage = store;
  prefs.set(DEFAULT_CLOSE_PREFS);
  loading = null;
  early = [];
}

/** The choices now (reactive). */
export const closePrefs = (): ClosePrefs => prefs();

function write(next: ClosePrefs): void {
  storage.setItem(CLOSE_PREFS_KEY, JSON.stringify({ quit: next.keys.quit, hide: next.keys.hide, close: next.close })).catch(() => {});
}

/** Shows `change` at once; writes once the stored choices are in (before that, it waits for them and is applied over them). */
function save(change: (now: ClosePrefs) => ClosePrefs): void {
  prefs.set(change(prefs()));
  if (early) {
    early.push(change);
    void loadClosePrefs();
  } else {
    write(prefs());
  }
}

/** Sets the quit or the hide key ("" for none). The caller has checked it is free (menu_model keyHolder). */
export function setAppKey(which: keyof AppKeys, keystroke: string): void {
  save((now) => ({ ...now, keys: { ...now.keys, [which]: keystroke } }));
}

/** Sets what Windows' and Linux's close button does: ask, or hide or quit without asking. */
export function setCloseChoice(close: CloseChoice): void {
  save((now) => ({ ...now, close }));
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
