// The desktop shell: the application menu, the tray and hide-to-tray, the global hotkey, the updater, the one-copy guard's wake-up.
// The app root mounts <DesktopHost />; the pieces are pure and tested on their own (menu_model.ts, updater.ts, hotkey.ts, banner.ts).
export { hideToTray, quitApp, setSignOutHandler, showMain } from "./actions";
export { promptFor, promptKey, type Prompt } from "./banner";
export { DesktopHost, desktopOs, installedMenu, installedTray, type DesktopHostProps, type Shortcuts } from "./Desktop";
export { createHotkey, type Hotkey, type HotkeyDeps, type HotkeyStatus } from "./hotkey";
export { holdHotkey, hotkeyHeld, hotkeyStatus } from "./hotkey_state";
export { APP_NAME, buildMenu, buildTrayMenu, CLOSE_POLICY, COPYRIGHT, flatten, type MenuAction, type MenuLine, type MenuState, type Os, printMenu, SUPPORT_URL, WEBSITE_URL } from "./menu_model";
export { createUpdater, type UpdateReport, type Updater, type UpdaterApi } from "./updater";
export { updates } from "./updates";
