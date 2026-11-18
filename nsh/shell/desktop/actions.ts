// What the menu, the tray, the window's close button and the hotkey DO (the menu's data is menu_model.ts, the rules closePolicy).
// Every action goes through the frame's own API (shell/app.ts): navigation, the layers, the theme; none reaches into a screen.
import { openUrl, platform, signal, untracked } from "nsh";
import type { CloseReason } from "nsh/desktop";
import { Window } from "nsh/desktop";
import { bir } from "../../bir";
import { errorText } from "../../lib/errors";
import { app } from "../app";
import { closePrefs, setCloseChoice } from "./app_keys";
import { closePolicy, SUPPORT_URL, WEBSITE_URL, type MenuAction, type Os } from "./menu_model";
import * as native from "./native";
import { updates } from "./updates";

/** The About dialog is open (desktop/Desktop.tsx draws it). */
export const aboutOpen = signal(false);

// ---------------------------------------------------------------- the window and the Dock icon

/** The main window, or null where there are no windows. */
export function mainWindow() {
  return Window.get("main");
}

const dock = signal(true);
/** macOS: the Dock icon is there (false while the app is hidden to the menu-bar icon). Always true elsewhere. */
export const dockIconShown = (): boolean => dock();

/** macOS only: the Dock icon (and the app's place in the app switcher) goes or comes back with the window. */
function setDockIcon(shown: boolean): void {
  if (platform.os !== "macos" || untracked(dock) === shown) return;
  dock.set(shown);
  native.setDockIcon(shown);
}

/** Brings the window back and gives it the keyboard (tray Show, the hotkey, a second launch); on macOS the Dock icon first. */
export function showMain(): void {
  const window = mainWindow();
  if (!window) return;
  setDockIcon(true);
  window.show();
  window.focus();
}

/** Hides the window: the app keeps running (and sending) in the tray. macOS: the Dock icon goes too, the menu-bar icon stays. Never asks. */
export function hideToTray(): void {
  const window = mainWindow();
  window?.hide();
  if (window && platform.os === "macos") setDockIcon(false);
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

/** How many returns the submission worker still has to send (Queued, every taxpayer's: the worker sends them all). 0 when it cannot tell. */
export function returnsSending(): number {
  try {
    return untracked(() => bir.list_returns("filed")).filter((r) => r.status === "Queued").length;
  } catch {
    return 0;
  }
}

let sendingCount: () => number = returnsSending;
/** A stand-in for the count (DesktopHost's `sending` prop, a test); null puts the real one back. */
export function useSendingCount(count: (() => number) | null): void {
  sendingCount = count ?? returnsSending;
}

/** Returns are still sending: how many, while the warning (Quit anyway / Wait / Hide to tray) is up; 0 when it is not. */
export const sendingAsk = signal(0);

/** Any quit (the menu, the quit key, the tray, the close button's Quit): warns first while returns are still sending. */
export function requestQuit(): void {
  const count = sendingCount();
  if (count <= 0) {
    void quitApp();
    return;
  }
  // The warning shows in the window: bring it up (and the Dock icon) if the quit came from the tray.
  showMain();
  sendingAsk.set(count);
}

/** The warning's answer: "quit" (Quit anyway), "wait" (stay open, keep sending), "hide" (hide to the tray, keep sending). */
export function answerSending(answer: "quit" | "wait" | "hide"): void {
  sendingAsk.set(0);
  if (answer === "quit") void quitApp();
  else if (answer === "hide") hideToTray();
}

// ---------------------------------------------------------------- the close button

/** "Hide to tray or Quit?" is up (Windows and Linux; desktop/Desktop.tsx draws it). */
export const closeAsk = signal(false);

/**
 * The window's close button, Alt+F4 and the menu's Close Window (Cmd+W, Ctrl+W): macOS hides (the Dock icon goes); Windows and Linux
 * ask, or do what "Don't ask again" remembered. A close the system asks for (another program, the session ending) quits without a word.
 */
export function closePressed(os: Os, reason: CloseReason = "closeButton"): void {
  if (reason === "system") {
    void quitApp();
    return;
  }
  const close = closePolicy(os, untracked(closePrefs)).close;
  if (close === "hide") hideToTray();
  else if (close === "quit") requestQuit();
  else closeAsk.set(true);
}

/** The close question's answer; `remember` is "Don't ask again" (the choice is kept and Settings > Keyboard can change it). Null: keep the window. */
export function answerClose(answer: "hide" | "quit" | null, remember: boolean): void {
  closeAsk.set(false);
  if (answer === null) return;
  if (remember) setCloseChoice(answer);
  if (answer === "hide") hideToTray();
  else requestQuit();
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
      hideToTray();
      return;
    case "close-window":
      closePressed(os, "menu");
      return;
    case "quit":
      requestQuit();
      return;
    case "show":
    case "bring-all-to-front":
      showMain();
      return;
  }
}
