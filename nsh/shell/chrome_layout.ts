// Which rows the window's chrome has, per OS, desktop and width (the approved app-menu design). Pure: a plain test reads every case.
//
//   macOS                  the system's menu bar; the window is ONE row: traffic lights, then our toolbar.
//   KDE Plasma with a      the menu goes to Plasma's Global Menu (NSH Linux exports it over D-Bus); ONE row: our toolbar, the
//   Global Menu            window buttons at its end (GTK's, client-side).
//   Windows, GNOME, every  TWO rows: the menu row (Windows: a small app icon first; titles; drag space; the window buttons),
//   other Linux desktop    then our toolbar without window buttons.
//   600 to 860             the same rows; titles lose their chevrons, the taxpayer pill its TIN.
//   under 600              the narrow header. Windows/Linux with an in-window menu: a ☰ first that lists every menu; macOS
//                          and KDE's Global Menu keep their menus at the top of the screen.
//   iOS, Android, web      no menu.

import { BREAK, SIZE } from "../theme/tokens";

/** The OS as nsh's `platform.os` says it. */
export type PlatformOs = "ios" | "android" | "macos" | "windows" | "linux" | "web" | string;

/** What decides the chrome. */
export interface ChromeInput {
  os: PlatformOs;
  /** A desktop with windows (nsh/desktop isAvailable && platform.desktop). */
  desktop: boolean;
  /** Linux only: NSH exported the menu to KDE Plasma's Global Menu and tied the window to it (nsh/desktop Menu.inGlobalMenu()). */
  globalMenu: boolean;
  /** The window's width, in points. */
  width: number;
}

/** What the chrome draws. */
export interface ChromeLayout {
  /** Where the application menu shows: the system's bar (macOS), Plasma's Global Menu, a row in the window, or nowhere. */
  menu: "system" | "global" | "window" | "none";
  /** Under 600: the narrow header and the tab bar. */
  narrow: boolean;
  /** The menu row above the toolbar (row 1 of two). */
  menuRow: boolean;
  /** The narrow header starts with ☰, which opens every menu as one list. */
  hamburger: boolean;
  /** The menu row's titles carry a small chevron (wide only). */
  chevrons: boolean;
  /** The menu row starts with the app's icon (Windows). */
  appIcon: boolean;
  /** The toolbar row pads for the window's own buttons (they share its row). False when the menu row holds them. */
  toolbarHoldsWindowButtons: boolean;
  /** The taxpayer pill shows the TIN (wide only). */
  pillTin: boolean;
  /** The height to ask the platform for the hidden title bar: the window buttons are centred in it. */
  titleBarHeight: number;
}

/** The menu row's height (and the title bar's in the two-row layout): Windows 11's caption buttons' height. */
export const MENU_ROW = 32;
/** The toolbar's height: the one-row title bar and the narrow header. */
export const TOOLBAR = SIZE.titleBar;
/** Under it: the narrow layout, as app.layout.narrow(). */
export const PHONE = BREAK.phone;
/** Under it: tablet width (the sidebar and panel are overlays). */
export const TABLET = BREAK.overlays;

/** Where the menu shows on this platform. */
export function menuPlace(input: Pick<ChromeInput, "os" | "desktop" | "globalMenu">): ChromeLayout["menu"] {
  if (!input.desktop) return "none";
  if (input.os === "macos") return "system";
  if (input.os === "windows") return "window";
  if (input.os === "linux") return input.globalMenu ? "global" : "window";
  return "none";
}

/** The chrome for an OS, desktop and width. */
export function chromeLayout(input: ChromeInput): ChromeLayout {
  const menu = menuPlace(input);
  const narrow = input.width < PHONE;
  const inWindow = menu === "window";
  const menuRow = inWindow && !narrow;
  return {
    menu,
    narrow,
    menuRow,
    hamburger: inWindow && narrow,
    chevrons: input.width >= TABLET,
    appIcon: menuRow && input.os === "windows",
    toolbarHoldsWindowButtons: !menuRow,
    pillTin: input.width >= TABLET,
    titleBarHeight: menuRow ? MENU_ROW : TOOLBAR,
  };
}

/** The Alt+letter that opens a menu of the row: its title's first letter ("eBIRForms" is B, as E is Edit's). */
export function menuKey(title: string, taken: readonly string[]): string | null {
  const letters = title === "eBIRForms" ? ["b", ...title.toLowerCase()] : [...title.toLowerCase()];
  for (const c of letters) if (/[a-z]/.test(c) && !taken.includes(c)) return c;
  return null;
}
