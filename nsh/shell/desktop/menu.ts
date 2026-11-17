// The application menu for the state the app is in now: menu_model.ts's data over the frame's signals, wired to actions.ts.
// One source for every place the menu shows: the system's menu bar (Desktop.tsx's Menu.setApplicationMenu), the menu row the
// window draws on Windows and Linux (shell/MenuRow.tsx) and the narrow header's ☰ list. Read inside an effect or JSX.
import type { MenuItem } from "nsh/desktop";
import { app } from "../app";
import { perform } from "./actions";
import { buildMenu, type MenuState, type Os } from "./menu_model";

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
  };
}

/** The menus now (reactive): each item carries out its action through perform(). */
export function menuNow(os: Os): MenuItem[] {
  return buildMenu(menuStateNow(os), (action, root) => perform(action, os, root));
}
