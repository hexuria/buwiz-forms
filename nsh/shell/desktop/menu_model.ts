// The application menu as data (old crates/bir-shell/src/app_menu.rs): which menus and items exist, which are enabled, and
// their accelerators, for each state the app can be in. Pure: no nsh runtime import, so a plain test reads every state.
//
// The old menu came in two shapes, and so does this one:
//   * the filing menus (the app is signed in and on its main window): everything below;
//   * the locked menus (first-run setup, the sign-in screens, the account gate): only About and Quit, the editing items
//     (so a PIN box can paste), and on Windows and Linux a Help menu with About. Filing commands are absent, as before.
// New here: with no taxpayer the items that need one are present but disabled (New Return…, Toggle Tax Profile).
//
// Dropped on purpose: macOS's "Services" submenu (NSH has no role for it; AppKit adds its own to the app menu) and Edit's
// "Delete" (no NSH role). "Enter Full Screen" on macOS is AppKit's own item. A shortcut the old app bound a second way
// (ctrl on macOS, cmd elsewhere) is bound once, as "mod".
//
// Quit: docs/platforms/macos.md says the window's X and Cmd+Q hide the app to the menu bar and that Quit is in the tray menu
// ("quitKeyHides" below); docs/platforms/linux.md says X and Ctrl+Q quit; docs/platforms/windows.md says X and Alt+F4 hide and
// there is no Ctrl+Q. The old app_menu.rs bound Ctrl/Cmd+Q to a real quit everywhere: this follows the platform docs.
import type { MenuItem } from "nsh/desktop";

/** The platforms the menu has a shape for. */
export type Os = "macos" | "windows" | "linux";

/** What a menu item does: desktop/actions.ts carries each one out. */
export type MenuAction =
  | "about"
  | "settings"
  | "new-return"
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
  | "show"
  | "quit-now";

/** What decides the menu. */
export interface MenuState {
  os: Os;
  /** The filing chrome is up: not first-run setup, not the sign-in screens (app.chromeVisible()). */
  unlocked: boolean;
  /** At least one taxpayer exists in the database the session may see (not app.isFresh()). */
  hasTaxpayer: boolean;
  /** The form page is the view on screen. It changes no item: the page owns the text fields and the menu never takes their keys. */
  formOpen: boolean;
}

/** The app's name in the menus. */
export const APP_NAME = "eBIRForms";
/** Help > Contact Support. */
export const SUPPORT_URL = "mailto:support@goldcoders.dev?subject=eBIRForms%20Support%20Request";
/** Help > Goldcoders Website. */
export const WEBSITE_URL = "https://goldcoders.dev";
/** The About text's last line. */
export const COPYRIGHT = "© 2026 Goldcoders Corp";

/** What each platform does with the window's close button and the quit key (see the header). */
export const CLOSE_POLICY: Record<Os, { closeHides: boolean; quitKeyHides: boolean; quitShortcut: string | null }> = {
  macos: { closeHides: true, quitKeyHides: true, quitShortcut: "mod+q" },
  windows: { closeHides: true, quitKeyHides: false, quitShortcut: null },
  linux: { closeHides: false, quitKeyHides: false, quitShortcut: "mod+q" },
};

type Run = (action: MenuAction) => void;
type Entry = MenuItem | null;
const SEP: MenuItem = { type: "separator" };

/** One item of ours: a label, what it does, its accelerator, whether it can be chosen. */
function item(run: Run, label: string, action: MenuAction, options: { shortcut?: string; disabled?: boolean } = {}): MenuItem {
  return { label, shortcut: options.shortcut, disabled: options.disabled === true ? true : undefined, onSelect: () => run(action) };
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

function quitItem(state: MenuState, run: Run): MenuItem {
  return item(run, "Quit " + APP_NAME, "quit", { shortcut: CLOSE_POLICY[state.os].quitShortcut ?? undefined });
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

function appMenu(state: MenuState, run: Run): MenuItem {
  const { unlocked } = state;
  return menu(APP_NAME, [
    item(run, "About " + APP_NAME, "about"),
    SEP,
    unlocked ? item(run, "Settings…", "settings", { shortcut: "mod+," }) : null,
    unlocked ? SEP : null,
    unlocked ? item(run, "Hide " + APP_NAME, "hide", { shortcut: "mod+h" }) : null,
    unlocked ? { role: "hideOthers" } : null,
    unlocked ? { role: "showAll" } : null,
    unlocked ? SEP : null,
    unlocked ? item(run, "Sign Out", "sign-out") : null,
    unlocked ? SEP : null,
    quitItem(state, run),
  ]);
}

function fileMenu(state: MenuState, run: Run): MenuItem {
  const { os, unlocked, hasTaxpayer } = state;
  if (!unlocked) return menu("File", [quitItem(state, run)]);
  const newReturn = item(run, "New Return…", "new-return", { shortcut: "mod+n", disabled: !hasTaxpayer });
  if (os === "macos") return menu("File", [newReturn, SEP, item(run, "Close Window", "close-window", { shortcut: "mod+w" })]);
  if (os === "windows") {
    return menu("File", [
      newReturn,
      SEP,
      item(run, "Settings…", "settings", { shortcut: "mod+," }),
      item(run, "Sign Out", "sign-out"),
      SEP,
      item(run, "Hide " + APP_NAME, "hide", { shortcut: "mod+h" }),
      SEP,
      quitItem(state, run),
    ]);
  }
  return menu("File", [
    newReturn,
    SEP,
    item(run, "Close Window", "close-window", { shortcut: "mod+w" }),
    item(run, "Hide " + APP_NAME, "hide", { shortcut: "mod+h" }),
    SEP,
    item(run, "Sign Out", "sign-out"),
    quitItem(state, run),
  ]);
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
    item(run, "Check for Updates…", "check-updates"),
    item(run, "Contact Support…", "support"),
    item(run, "Goldcoders Website", "website"),
  ]);
}

/**
 * The menus for a state, the way the old app_menu.rs listed them. macOS: the app menu, File (signed in only), Edit, and
 * View, Window and Help (signed in only). Windows and Linux: File, Edit, and View and Window (signed in only), Help.
 */
export function buildMenu(state: MenuState, run: Run): MenuItem[] {
  const { os, unlocked } = state;
  const menus: Entry[] = [];
  if (os === "macos") menus.push(appMenu(state, run));
  if (unlocked || os !== "macos") menus.push(fileMenu(state, run));
  menus.push(editMenu(unlocked && os === "linux" ? item(run, "Settings…", "settings", { shortcut: "mod+," }) : null));
  if (unlocked) {
    menus.push(viewMenu(state, run), windowMenu(state, run), helpMenu(state, run));
  } else if (os !== "macos") {
    menus.push(helpMenu(state, run));
  }
  return tidy(menus);
}

/**
 * The tray icon's menu (old tray.rs): Show, Hide, and Quit, the one way out on macOS and Windows (the window's X and the quit key
 * hide the app; the quit here awaits the frame's quit path). The same on every state: the tray outlives the sign-in screens.
 */
export function buildTrayMenu(run: Run): MenuItem[] {
  return [item(run, "Show " + APP_NAME, "show"), item(run, "Hide " + APP_NAME, "hide"), SEP, item(run, "Quit", "quit-now")];
}

/** One line of a flattened menu: where it is, what it says, its accelerator, whether it can be chosen. */
export interface MenuLine {
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
      lines.push({ path, shortcut: entry.shortcut ?? null, disabled: entry.disabled === true, select: entry.onSelect ?? null });
    }
  }
  return lines;
}

/** The text of a flattened menu, one item per line: "File > New Return…  mod+n  (disabled)". */
export function printMenu(items: readonly (MenuItem | null | undefined | false)[]): string[] {
  return flatten(items).map((l) => l.path + (l.shortcut ? "  " + l.shortcut : "") + (l.disabled ? "  (disabled)" : ""));
}
