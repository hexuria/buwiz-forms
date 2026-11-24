// The application menu as data (the retired gpui shell's src/app_menu.rs): which menus and items exist, which are enabled, and
// their accelerators, for each state the app can be in. Pure: no nsh runtime import, so a plain test reads every state.
//
// The old menu came in two shapes, and so does this one:
//   * the filing menus (the app is signed in and on its main window): everything below;
//   * the locked menus (first-run setup, the sign-in screens, the account gate): only About and Quit, the editing items
//     (so a PIN box can paste), and on Windows and Linux a Help menu with About. Filing commands are absent, as before.
// New here: with no taxpayer the items that need one are present but disabled (New Return…, Toggle Tax Profile).
// Added for the eBIRForms app menu design: File's "New Tax Profile…" (above New Return…, no shortcut: Ctrl/Cmd+N stays New
// Return…), a last "Profiles" menu while signed in (the taxpayers as a switcher, the current one checked, then New Tax
// Profile…), and on Windows and Linux an "eBIRForms" app menu first, as on macOS. There Hide and Quit carry the user's keys (their
// copies in File carry none) and Settings… none (File or Edit keeps Ctrl+,), so an accelerator is never bound twice. The in-window
// menu row (shell/MenuRow.tsx) and the narrow ☰ list draw this same data.
//
// Dropped on purpose: macOS's "Services" submenu (NSH has no role for it; AppKit adds its own to the app menu) and Edit's
// "Delete" (no NSH role). "Enter Full Screen" on macOS is AppKit's own item. A shortcut the old app bound a second way
// (ctrl on macOS, cmd elsewhere) is bound once, as "mod".
//
// Close, quit and hide (the owner's approved rules, 2026-11-18; they replace the old CLOSE_POLICY, which followed docs/platforms):
//   * Quit: Cmd+Q on macOS, Ctrl+Q on Windows and Linux really quit (Apple review: Cmd+Q quits out of the box, and a Quit item is
//     always in the app menu and the tray). Super+Q is off by default on Linux; the recorder can bind it.
//   * Hide: Cmd+H / Ctrl+H hide to the tray (macOS: the menu-bar icon) and never ask.
//   * Close: macOS's red button and Cmd+W hide the window and take the Dock icon away (Show brings it back). Windows' and Linux's X,
//     Alt+F4 and Close Window ask "Hide to tray or Quit?" unless "Don't ask again" remembered a choice (Settings > Keyboard changes it).
//   * Any quit while returns are still sending warns first: Quit anyway / Wait / Hide to tray.
//   * The quit and hide keys are the user's (Settings > Keyboard, ShortcutRecorder): the menus show the keys they chose.
// `closePolicy(os, prefs)` is that data; actions.ts carries it out.
import type { MenuItem } from "nsh/desktop";
import { KEYSTROKE_WORDS, MODIFIER_ORDER } from "../../lib/settings_model";

/** The platforms the menu has a shape for. */
export type Os = "macos" | "windows" | "linux";

/** What a menu item does: desktop/actions.ts carries each one out. */
export type MenuAction =
  | "about"
  | "settings"
  | "new-return"
  | "new-tax-profile"
  | "switch-taxpayer"
  | "toggle-sidebar"
  | "toggle-profile"
  | "toggle-notifications"
  | "toggle-theme"
  | "palette"
  | "sign-out"
  | "hide"
  | "close-window"
  | "quit"
  | "bring-all-to-front"
  | "check-updates"
  | "support"
  | "website"
  | "show";

/** What decides the menu. */
export interface MenuState {
  os: Os;
  /** The filing chrome is up: not first-run setup, not the sign-in screens (app.chromeVisible()). */
  unlocked: boolean;
  /** At least one taxpayer exists in the database the session may see (not app.isFresh()). */
  hasTaxpayer: boolean;
  /** The form page is the view on screen. It changes no item: the page owns the text fields and the menu never takes their keys. */
  formOpen: boolean;
  /** The taxpayers the Profiles menu lists (store.taxpayers()); none: "No taxpayers yet". */
  taxpayers?: readonly MenuTaxpayer[];
  /** The tin_root of the taxpayer the frame is on (checked in Profiles), or null. */
  currentTaxpayer?: string | null;
  /** New Tax Profile… can be chosen (app.canAddTaxpayer(): the first taxpayer, or the admin in helpdesk mode). Default true. */
  canAddTaxpayer?: boolean;
  /** Another taxpayer can be switched to (app.isAdmin(), as the profile panel's switcher). Default true. */
  canSwitchTaxpayer?: boolean;
  /** The quit and hide keys the user chose (Settings > Keyboard). Default DEFAULT_KEYS. */
  keys?: AppKeys;
  /** A Quit or Hide key recorder is listening: the Quit and Hide items carry the platform's own quit and hide keys (CAPTURE_KEYS), which
   * nsh/keymap cannot bind, so the recorder hears them through the menu (menu.ts hands their onSelect to it). */
  capturing?: boolean;
  /** Mac App Store builds don't check for updates. */
  masBuild?: boolean;
  /** Neither does a Microsoft Store package: its store delivers updates. */
  storeManaged?: boolean;
}

/** One taxpayer in the Profiles menu. */
export interface MenuTaxpayer {
  root: string;
  name: string;
}

/** The app's name in the menus. */
export const APP_NAME = "eBIRForms";
/** Help > Contact Support. */
export const SUPPORT_URL = "mailto:support@buwiz.com?subject=eBIRForms%20Support%20Request";
/** Help > eBIRForms Website. */
export const WEBSITE_URL = "https://ebirforms.buwiz.com";
/** The About text's last line. */
export const COPYRIGHT = "© 2026 Goldcoders Corp";

// ---------------------------------------------------------------- close, quit and hide

/** The quit and hide keys, as nsh/keymap and menu shortcuts write them ("mod+q", "alt+cmd+q"); "" for none. */
export interface AppKeys {
  quit: string;
  hide: string;
}

/** Out of the box: Cmd+Q / Ctrl+Q quit, Cmd+H / Ctrl+H hide ("mod" is Command on macOS, Ctrl elsewhere). Super+Q is not bound on Linux. */
export const DEFAULT_KEYS: AppKeys = { quit: "mod+q", hide: "mod+h" };

/** What Windows' and Linux's close button does: ask each time, or the choice "Don't ask again" remembered. */
export type CloseChoice = "ask" | "hide" | "quit";

/** The close button's choices as Settings > Keyboard offers them. */
export const CLOSE_CHOICES: readonly { value: CloseChoice; label: string }[] = [
  { value: "ask", label: "Ask" },
  { value: "hide", label: "Hide to tray" },
  { value: "quit", label: "Quit" },
];

/** What the user chose: their keys and the close button's remembered choice. */
export interface ClosePrefs {
  keys: AppKeys;
  close: CloseChoice;
}

export const DEFAULT_CLOSE_PREFS: ClosePrefs = { keys: DEFAULT_KEYS, close: "ask" };

/** What a platform does, with the user's choices. */
export interface ClosePolicy {
  /** The window's close button, Alt+F4 and Close Window (Cmd+W, Ctrl+W): hide, ask "Hide to tray or Quit?", or quit. */
  close: "hide" | "ask" | "quit";
  /** Hiding takes the Dock icon away and showing brings it back (macOS). */
  hideTakesDockIcon: boolean;
  /** The quit item's shortcut, or null. */
  quitShortcut: string | null;
  /** The hide item's shortcut, or null. The hide key never asks. */
  hideShortcut: string | null;
}

/** The rules above for `os`: macOS's close always hides; Windows' and Linux's follows the remembered choice. */
export function closePolicy(os: Os, prefs: ClosePrefs = DEFAULT_CLOSE_PREFS): ClosePolicy {
  return {
    close: os === "macos" ? "hide" : prefs.close,
    hideTakesDockIcon: os === "macos",
    quitShortcut: prefs.keys.quit || null,
    hideShortcut: prefs.keys.hide || null,
  };
}

/** Reads what was stored (desktop/app_keys.ts): anything missing or unknown is the default. */
export function parseClosePrefs(raw: string | null | undefined): ClosePrefs {
  let value: { quit?: unknown; hide?: unknown; close?: unknown } = {};
  try {
    value = raw ? (JSON.parse(raw) ?? {}) : {};
  } catch {
    value = {};
  }
  const key = (v: unknown, fallback: string) => (typeof v === "string" ? v : fallback);
  const close = CLOSE_CHOICES.some((c) => c.value === value.close) ? (value.close as CloseChoice) : "ask";
  return { keys: { quit: key(value.quit, DEFAULT_KEYS.quit), hide: key(value.hide, DEFAULT_KEYS.hide) }, close };
}

/** The keys the platform keeps for quitting and hiding: nsh/keymap refuses them, so while a recorder listens the menu carries them. */
export const CAPTURE_KEYS: Record<Os, { quit: string; hide: string | null }> = {
  macos: { quit: "cmd+q", hide: "cmd+h" },
  windows: { quit: "ctrl+q", hide: null },
  linux: { quit: "ctrl+q", hide: null },
};

/** The shortcuts the Quit and Hide items carry in `state` (null: none). While a recorder listens, the platform's kept keys. */
export function itemKeys(state: Pick<MenuState, "os" | "keys" | "capturing">): { quit: string | null; hide: string | null } {
  const keys = state.keys ?? DEFAULT_KEYS;
  if (!state.capturing) return { quit: keys.quit || null, hide: keys.hide || null };
  const kept = CAPTURE_KEYS[state.os];
  const quit = kept.quit;
  const hide = kept.hide ?? (keys.hide && chord(state.os, keys.hide) !== chord(state.os, quit) ? keys.hide : null);
  return { quit, hide };
}

/** How many returns are still sending, in the warning's words; null when none is (the quit goes ahead). */
export function sendingWarning(count: number): { title: string; message: string } | null {
  if (count <= 0) return null;
  return {
    title: count === 1 ? "1 return is still sending" : `${count} returns are still sending`,
    message: "Quitting stops them. Hide keeps them going in the background.",
  };
}

// ---------------------------------------------------------------- the keys: one spelling, and who holds each

const KEY_NAMES: Readonly<Record<string, string>> = {
  "-": "minus",
  "=": "plus",
  esc: "escape",
  return: "enter",
  del: "delete",
  arrowup: "up",
  arrowdown: "down",
  arrowleft: "left",
  arrowright: "right",
};

/** A keystroke in one spelling for `os`: "mod" made concrete, aliases resolved, modifiers in the order ctrl, alt, shift, cmd ("" for none). */
export function chord(os: Os, keystroke: string): string {
  const parts = keystroke.toLowerCase().split("+").filter(Boolean);
  if (!parts.length) return "";
  const key = parts[parts.length - 1];
  // nsh/keymap's modifier words (lib/settings_model.ts KEYSTROKE_WORDS), "mod" made concrete for `os`.
  const mods = new Set<string>(parts.slice(0, -1).map((m) => (m === "mod" || m === "cmdorctrl" ? (os === "macos" ? "cmd" : "ctrl") : (KEYSTROKE_WORDS[m] ?? m))));
  return [...MODIFIER_ORDER.filter((m) => mods.has(m)), KEY_NAMES[key] ?? key].join("+");
}

/** Windows' and Linux's: the same edit roles and Alt+F4. */
const PC_ROLE_KEYS: Readonly<Record<string, string>> = {
  "ctrl+z": "Undo",
  "ctrl+y": "Redo",
  "ctrl+shift+z": "Redo",
  "ctrl+x": "Cut",
  "ctrl+c": "Copy",
  "ctrl+v": "Paste",
  "ctrl+a": "Select All",
  "alt+f4": "Close",
};

/** The keys the platform's own items keep (the edit roles, Minimize, Hide Others, full screen, Alt+F4), with what they do. */
const ROLE_KEYS: Record<Os, Readonly<Record<string, string>>> = {
  macos: {
    "cmd+z": "Undo",
    "shift+cmd+z": "Redo",
    "cmd+x": "Cut",
    "cmd+c": "Copy",
    "cmd+v": "Paste",
    "cmd+a": "Select All",
    "cmd+m": "Minimize",
    "alt+cmd+h": "Hide Others",
    "ctrl+cmd+f": "Enter Full Screen",
  },
  windows: PC_ROLE_KEYS,
  linux: PC_ROLE_KEYS,
};

/** The frame's own keys that no menu item shows (shell/Shell.tsx bindKeys). */
export const FRAME_KEYS: Readonly<Record<string, string>> = { "mod+l": "Lock", "mod+[": "Back", "mod+]": "Forward" };

/**
 * Who already holds `keystroke` if it became the `which` key: another menu item (in the full signed-in menu, whatever the state now),
 * a platform item, one of the frame's keys, the other of quit and hide, or the global show hotkey. Null when it is free. A shortcut is
 * never bound twice.
 */
export function keyHolder(os: Os, which: keyof AppKeys, keystroke: string, keys: AppKeys = DEFAULT_KEYS, globalHotkey = ""): string | null {
  const want = chord(os, keystroke);
  if (!want) return null;
  const others: AppKeys = { ...keys, [which]: "" };
  const full: MenuState = { os, unlocked: true, hasTaxpayer: true, formOpen: false, keys: others };
  for (const line of flatten(buildMenu(full, () => {}))) {
    if (line.shortcut && chord(os, line.shortcut) === want) return line.path.split(" > ").pop() ?? line.path;
  }
  const role = ROLE_KEYS[os][want];
  if (role) return role;
  for (const [key, name] of Object.entries(FRAME_KEYS)) if (chord(os, key) === want) return name;
  if (globalHotkey && chord(os, globalHotkey) === want) return "the global show hotkey";
  return null;
}

/** Whether a recorded chord can be a quit or hide key at all: it needs Control, Option/Alt or Command/Super (a plain key would take typing). */
export function appKeyUsable(keystroke: string): boolean {
  return keystroke.split("+").slice(0, -1).some((m) => m === "ctrl" || m === "alt" || m === "cmd" || m === "mod");
}

/** Carries out an item; "switch-taxpayer" names the taxpayer's tin_root. */
type Run = (action: MenuAction, root?: string) => void;
type Entry = MenuItem | null;
const SEP: MenuItem = { type: "separator" };

/** One item of ours: a label, what it does, its accelerator, whether it can be chosen. */
function item(run: Run, label: string, action: MenuAction, options: { shortcut?: string; disabled?: boolean; checked?: boolean; root?: string } = {}): MenuItem {
  const out: MenuItem = { label, shortcut: options.shortcut, disabled: options.disabled === true ? true : undefined, onSelect: () => run(action, options.root) };
  if (options.checked !== undefined) out.checked = options.checked;
  return out;
}

/** Drops the entries that are not there, and separators that would lead, trail or double. */
function tidy(entries: Entry[]): MenuItem[] {
  const out: MenuItem[] = [];
  for (const e of entries) {
    if (e === null) continue;
    const separator = "type" in e && e.type === "separator";
    if (separator && (out.length === 0 || "type" in out[out.length - 1])) continue;
    out.push(e);
  }
  while (out.length && "type" in out[out.length - 1]) out.pop();
  return out;
}

function menu(label: string, entries: Entry[]): MenuItem {
  return { label, submenu: tidy(entries) };
}

/** Quit and Hide: in the app menu they carry the user's keys (rule 8); their copies in File carry none, so no key is bound twice. */
function quitItem(state: MenuState, run: Run, withShortcut: boolean): MenuItem {
  return item(run, "Quit " + APP_NAME, "quit", { shortcut: withShortcut ? (itemKeys(state).quit ?? undefined) : undefined });
}

function hideItem(state: MenuState, run: Run, withShortcut: boolean): MenuItem {
  return item(run, "Hide " + APP_NAME, "hide", { shortcut: withShortcut ? (itemKeys(state).hide ?? undefined) : undefined });
}

function editMenu(settingsAtEnd: Entry): MenuItem {
  return menu("Edit", [
    { role: "undo" },
    { role: "redo" },
    SEP,
    { role: "cut" },
    { role: "copy" },
    { role: "paste" },
    SEP,
    { role: "selectAll" },
    settingsAtEnd === null ? null : SEP,
    settingsAtEnd,
  ]);
}

/** The app menu: About, Settings…, Hide, Sign Out, Quit, the same on every desktop. Hide and Quit carry the user's keys here, signed
 * in or locked (a locked app still hides and quits). macOS: AppKit's app menu, with Settings' shortcut and Hide Others / Show All.
 * Windows and Linux: the first menu of the row in the window; Settings' shortcut stays in File (Windows) or Edit (Linux). */
function appMenu(state: MenuState, run: Run): MenuItem {
  const { unlocked } = state;
  const mac = state.os === "macos";
  return menu(APP_NAME, [
    item(run, "About " + APP_NAME, "about"),
    SEP,
    unlocked ? item(run, "Settings…", "settings", { shortcut: mac ? "mod+," : undefined }) : null,
    unlocked ? SEP : null,
    hideItem(state, run, true),
    unlocked && mac ? { role: "hideOthers" } : null,
    unlocked && mac ? { role: "showAll" } : null,
    SEP,
    unlocked ? item(run, "Sign Out", "sign-out") : null,
    unlocked ? SEP : null,
    quitItem(state, run, true),
  ]);
}

function fileMenu(state: MenuState, run: Run): MenuItem {
  const { os, unlocked, hasTaxpayer } = state;
  if (!unlocked) return menu("File", [hideItem(state, run, false), quitItem(state, run, false)]);
  const newProfile = newTaxProfileItem(state, run);
  const newReturn = item(run, "New Return…", "new-return", { shortcut: "mod+n", disabled: !hasTaxpayer });
  if (os === "macos") return menu("File", [newProfile, newReturn, SEP, item(run, "Close Window", "close-window", { shortcut: "mod+w" })]);
  if (os === "windows") {
    return menu("File", [
      newProfile,
      newReturn,
      SEP,
      item(run, "Settings…", "settings", { shortcut: "mod+," }),
      item(run, "Sign Out", "sign-out"),
      SEP,
      hideItem(state, run, false),
      SEP,
      quitItem(state, run, false),
    ]);
  }
  return menu("File", [
    newProfile,
    newReturn,
    SEP,
    item(run, "Close Window", "close-window", { shortcut: "mod+w" }),
    hideItem(state, run, false),
    SEP,
    item(run, "Sign Out", "sign-out"),
    quitItem(state, run, false),
  ]);
}

/** New Tax Profile…: no shortcut (Ctrl/Cmd+N is New Return…). Disabled where the profile panel offers no New taxpayer. */
function newTaxProfileItem(state: MenuState, run: Run): MenuItem {
  return item(run, "New Tax Profile…", "new-tax-profile", { disabled: state.canAddTaxpayer === false });
}

/** Profiles (signed in only): the taxpayers as a switcher, the current one checked, then New Tax Profile…. */
function profilesMenu(state: MenuState, run: Run): MenuItem {
  const list = state.taxpayers ?? [];
  const current = state.currentTaxpayer ?? null;
  const locked = state.canSwitchTaxpayer === false;
  const rows: Entry[] = list.length
    ? list.map((t) =>
        item(run, t.name, "switch-taxpayer", { root: t.root, checked: t.root === current, disabled: locked && t.root !== current }),
      )
    : [{ label: "No taxpayers yet", disabled: true }];
  return menu("Profiles", [...rows, SEP, newTaxProfileItem(state, run)]);
}

function viewMenu(state: MenuState, run: Run): MenuItem {
  return menu("View", [
    item(run, "Toggle Sidebar", "toggle-sidebar", { shortcut: "mod+b" }),
    item(run, "Toggle Tax Profile", "toggle-profile", { shortcut: "mod+shift+b", disabled: !state.hasTaxpayer }),
    item(run, "Toggle Notifications", "toggle-notifications", { shortcut: "mod+shift+n" }),
    item(run, "Toggle Theme", "toggle-theme", { shortcut: "mod+t" }),
    SEP,
    item(run, "Command Palette…", "palette", { shortcut: "mod+k" }),
    // AppKit puts its own Enter Full Screen item in the View menu: a second one would show the command twice.
    state.os === "macos" ? null : SEP,
    state.os === "macos" ? null : { role: "toggleFullScreen", label: "Enter Full Screen", shortcut: "f11" },
  ]);
}

function windowMenu(state: MenuState, run: Run): MenuItem {
  return menu("Window", [
    { role: "minimize" },
    { role: "zoom" },
    state.os === "macos" ? SEP : null,
    state.os === "macos" ? item(run, "Bring All to Front", "bring-all-to-front") : null,
  ]);
}

function helpMenu(state: MenuState, run: Run): MenuItem {
  const about = state.os !== "macos" ? item(run, "About " + APP_NAME, "about") : null;
  if (!state.unlocked) return menu("Help", [about]);
  return menu("Help", [
    about,
    about ? SEP : null,
    state.masBuild || state.storeManaged ? null : item(run, "Check for Updates…", "check-updates"),
    item(run, "Contact Support…", "support"),
    item(run, "eBIRForms Website", "website"),
  ]);
}

/**
 * The menus for a state. macOS: the app menu, File (signed in only), Edit, and View, Window, Help and Profiles (signed in
 * only). Windows and Linux: the app menu, File, Edit, View and Window (signed in only), Help, Profiles (signed in only).
 */
export function buildMenu(state: MenuState, run: Run): MenuItem[] {
  const { os, unlocked } = state;
  const menus: Entry[] = [];
  menus.push(appMenu(state, run));
  if (unlocked || os !== "macos") menus.push(fileMenu(state, run));
  menus.push(editMenu(unlocked && os === "linux" ? item(run, "Settings…", "settings", { shortcut: "mod+," }) : null));
  if (unlocked) {
    menus.push(viewMenu(state, run), windowMenu(state, run), helpMenu(state, run), profilesMenu(state, run));
  } else if (os !== "macos") {
    menus.push(helpMenu(state, run));
  }
  return tidy(menus);
}

/**
 * The tray icon's menu (old tray.rs): Show, Hide and Quit on every platform and in every state (the tray outlives the sign-in screens), the
 * real way out when the window is hidden. Show is disabled while the window is on screen, Hide while it is not. No shortcuts: the
 * application menu has them.
 */
export function buildTrayMenu(run: Run, windowVisible: boolean): MenuItem[] {
  return [
    item(run, "Show " + APP_NAME, "show", { disabled: windowVisible }),
    item(run, "Hide " + APP_NAME, "hide", { disabled: !windowVisible }),
    SEP,
    item(run, "Quit " + APP_NAME, "quit"),
  ];
}

/** One line of a flattened menu: where it is, what it says, its accelerator, whether it can be chosen. */
export interface MenuLine {
  /** Its check mark, when it has one. */
  checked?: boolean;
  /** "File > New Return…". A role's item with no label of its own is named by its role ("Edit > role:undo"). */
  path: string;
  shortcut: string | null;
  disabled: boolean;
  /** The item to call onSelect on; null for a role (the platform carries it out). */
  select: (() => void) | null;
}

/** Every item with its path, in menu order, separators left out. For tests and for the fixture that prints the menu. */
export function flatten(items: readonly (MenuItem | null | undefined | false)[], prefix = ""): MenuLine[] {
  const lines: MenuLine[] = [];
  for (const entry of items) {
    if (!entry || "type" in entry) continue;
    const name = entry.label ?? (entry.role ? "role:" + entry.role : "?");
    const path = prefix ? prefix + " > " + name : name;
    if (entry.submenu) {
      lines.push(...flatten(entry.submenu, path));
    } else {
      const line: MenuLine = { path, shortcut: entry.shortcut ?? null, disabled: entry.disabled === true, select: entry.onSelect ?? null };
      if (entry.checked !== undefined) line.checked = entry.checked;
      lines.push(line);
    }
  }
  return lines;
}

/** The text of a flattened menu, one item per line: "File > New Return…  mod+n  (disabled)". */
export function printMenu(items: readonly (MenuItem | null | undefined | false)[]): string[] {
  return flatten(items).map((l) => l.path + (l.shortcut ? "  " + l.shortcut : "") + (l.checked ? "  (checked)" : "") + (l.disabled ? "  (disabled)" : ""));
}
