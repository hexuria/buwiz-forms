// The application menu for the state the app is in now: menu_model.ts's data over the frame's signals, wired to actions.ts.
// One source for every place the menu shows: the system's menu bar (Desktop.tsx's Menu.setApplicationMenu), the menu row the
// window draws on Windows and Linux (shell/MenuRow.tsx) and the narrow header's ☰ list. Read inside an effect or JSX.
import type { MenuItem } from "nsh/desktop";
import { untracked } from "nsh";
import { app } from "../app";
import { perform } from "./actions";
import { captured, capturing, closePrefs } from "./app_keys";
import { buildMenu, itemKeys, type MenuAction, type MenuState, type Os } from "./menu_model";
import { masBuild } from "./native";

/** What decides the menu now (reactive). */
export function menuStateNow(os: Os): MenuState {
  return {
    os,
    unlocked: app.chromeVisible(),
    hasTaxpayer: !app.isFresh(),
    formOpen: app.view() === "form",
    taxpayers: app.taxpayers().map((t) => ({ root: t.tin_root, name: t.name })),
    currentTaxpayer: app.taxpayer()?.tin_root ?? null,
    canAddTaxpayer: app.canAddTaxpayer(),
    canSwitchTaxpayer: app.isAdmin(),
    keys: closePrefs().keys,
    capturing: capturing(),
    masBuild: masBuild(),
  };
}

/**
 * Carries out a menu item. While a Quit or Hide key recorder listens, those two items hand the key they carry to it instead (the
 * platform's quit and hide keys reach the app only through the menu: nsh/keymap cannot bind them).
 */
function select(os: Os, action: MenuAction, root?: string): void {
  const heard = untracked(captured);
  if (heard && (action === "quit" || action === "hide")) {
    const key = itemKeys({ os, keys: untracked(closePrefs).keys, capturing: true })[action];
    if (key) heard(key);
    return;
  }
  perform(action, os, root);
}

/** The menus now (reactive): each item carries out its action through perform(). */
export function menuNow(os: Os): MenuItem[] {
  return buildMenu(menuStateNow(os), (action, root) => select(os, action, root));
}
