// What the menu, the tray, the window's close button and the hotkey DO (the menu's data is menu_model.ts). Every action goes
// through the frame's own API (shell/app.ts): navigation, the layers, the theme; none reaches into a screen.
import { openUrl, signal } from "nsh";
import { Window } from "nsh/desktop";
import { bir } from "../../bir";
import { errorText } from "../../lib/errors";
import { app } from "../app";
import { CLOSE_POLICY, SUPPORT_URL, WEBSITE_URL, type MenuAction, type Os } from "./menu_model";
import { updates } from "./updates";

/** The About dialog is open (desktop/Desktop.tsx draws it). */
export const aboutOpen = signal(false);

// ---------------------------------------------------------------- the window

/** The main window, or null where there are no windows. */
export function mainWindow() {
  return Window.get("main");
}

/** Brings the window back and gives it the keyboard (tray Show, the hotkey, a second launch, the Dock icon). */
export function showMain(): void {
  const window = mainWindow();
  if (!window) return;
  window.show();
  window.focus();
}

/** Hides the window: the app keeps running in the tray. */
export function hideToTray(): void {
  mainWindow()?.hide();
}

/** Show when it is hidden or behind, hide when it is up (the global hotkey). */
export function toggleMain(): void {
  const state = mainWindow()?.state();
  if (state?.visible && state.focused) hideToTray();
  else showMain();
}

// ---------------------------------------------------------------- quitting

/**
 * Quits for real: the frame's quit path (app.quit) runs every screen's beforeLeave hook (an open form flushes its unsaved edits),
 * then the door shuts down, then the app ends. Resolves once the hooks are done; "exit" is for a test.
 */
export function quitApp(exit?: () => void): Promise<void> {
  return app.quit(exit);
}

/** The window's close button (and Alt+F4, and the menu's Close Window): hides where the platform's docs say so, else quits. */
export function closePressed(os: Os): void {
  if (CLOSE_POLICY[os].closeHides) hideToTray();
  else void quitApp();
}

// ---------------------------------------------------------------- sign out

let signOutHandler: (() => void | Promise<void>) | null = null;

/** The Buwiz account's sign-out screen (wave 4, sign-in) takes over what the menu's Sign Out does. */
export function setSignOutHandler(handler: (() => void | Promise<void>) | null): void {
  signOutHandler = handler;
}

/** The old app's menu Sign Out when the account page was not open: revoke the session, close it, and show the gate. */
async function signOutOfTheAccount(): Promise<void> {
  try {
    await bir.cloud_account_sign_out();
  } catch (error) {
    app.toast(errorText(error));
    return;
  }
  // The frame's own way: the host's session closes, the gate comes back, and every screen's state is put back for the next session.
  app.lockCloud();
}

// ---------------------------------------------------------------- the menu's actions

/** "Check for Updates…": the banner says what a check found; a toast says it when there is no banner (no update source configured). */
async function checkForUpdates(): Promise<void> {
  const note = await updates.checkNow();
  if (note) app.toast(note);
}

/** Carries out one menu, tray or key action. Filing commands do nothing while the chrome is hidden (the old dispatch_if_unlocked).
 * `root` is the taxpayer a "switch-taxpayer" item names. */
export function perform(action: MenuAction, os: Os, root?: string): void {
  const filing = app.chromeVisible();
  switch (action) {
    case "about":
      aboutOpen.set(true);
      return;
    case "settings":
      if (filing) app.go("settings");
      return;
    case "new-return":
      if (filing) app.startNewReturn();
      return;
    case "new-tax-profile":
      // The profile panel's "New taxpayer": the first taxpayer, or the admin in helpdesk mode.
      if (filing && app.canAddTaxpayer()) app.go("new_taxpayer");
      return;
    case "switch-taxpayer":
      // The profile panel's switcher: the admin moves between taxpayers; the one on screen stays.
      if (filing && root && root !== app.taxpayer()?.tin_root && app.isAdmin()) app.selectTaxpayer(root);
      return;
    case "toggle-sidebar":
      if (filing) app.toggleSidebar();
      return;
    case "toggle-profile":
      if (filing) app.togglePanel("profile");
      return;
    case "toggle-notifications":
      if (filing) app.togglePanel("notifs");
      return;
    case "toggle-theme":
      if (filing) app.cycleTheme();
      return;
    case "palette":
      if (filing) app.openPalette();
      return;
    case "sign-out":
      if (filing) void Promise.resolve(signOutHandler ? signOutHandler() : signOutOfTheAccount());
      return;
    case "check-updates":
      if (filing) void checkForUpdates();
      return;
    case "support":
      if (filing) void openUrl(SUPPORT_URL);
      return;
    case "website":
      if (filing) void openUrl(WEBSITE_URL);
      return;
    case "hide":
    case "close-window":
      hideToTray();
      return;
    case "quit":
      // Cmd+Q hides on macOS (the quit is in the tray); Linux's Ctrl+Q quits.
      if (CLOSE_POLICY[os].quitKeyHides) hideToTray();
      else void quitApp();
      return;
    case "quit-now":
      void quitApp();
      return;
    case "show":
    case "bring-all-to-front":
      showMain();
      return;
  }
}
