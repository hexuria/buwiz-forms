// The desktop shell: the application menu, the tray and hide-to-tray, the global hotkey, the updater, the one-copy guard's wake-up.
// The app root mounts <DesktopHost />; the pieces are pure and tested on their own (menu_model.ts, updater.ts, hotkey.ts, banner.ts).
export { dockIconShown, hideToTray, quitApp, requestQuit, returnsSending, setSignOutHandler, showMain } from "./actions";
export { captureMenuKeys, closePrefs, loadClosePrefs, setAppKey, setCloseChoice } from "./app_keys";
export { promptFor, promptKey, type Prompt } from "./banner";
export { DesktopHost, desktopOs, installedMenu, installedTray, type DesktopHostProps, type Shortcuts } from "./Desktop";
export { createHotkey, type Hotkey, type HotkeyDeps, type HotkeyStatus } from "./hotkey";
export { holdHotkey, hotkeyHeld, hotkeyStatus } from "./hotkey_state";
export {
  APP_NAME, appKeyUsable, type AppKeys, buildMenu, buildTrayMenu, CLOSE_CHOICES, type CloseChoice, closePolicy, type ClosePolicy, type ClosePrefs,
  COPYRIGHT, DEFAULT_KEYS, flatten, keyHolder, type MenuAction, type MenuLine, type MenuState, type Os, printMenu, SUPPORT_URL, WEBSITE_URL,
} from "./menu_model";
export { createUpdater, type UpdateReport, type Updater, type UpdaterApi } from "./updater";
export { updates } from "./updates";
