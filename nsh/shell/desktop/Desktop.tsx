// The desktop host: what makes the app a desktop app. One component, mounted once by the app root (App.tsx), drawing nothing but the
// About dialog. It owns
//   * the application menu (menu_model.ts), rebuilt when the state it depends on changes: the gate (signed in or not), whether there
//     is a taxpayer, the view, and the host's own refresh requests (src/desktop.rs: refresh_menu, set_shell_chrome_open);
//   * the tray icon and its menu, and hide-to-tray: the window's close button hides the window instead of closing it;
//   * the global show/hide hotkey, from the stored setting global_hotkey_key;
//   * a short tick that reads what Rust has for it: a second launch asking for the window, the updater commands the host functions
//     queued (the banner's Install now and Later, "Check for Updates…", the Download automatically switch);
//   * the updater driver (updates.ts).
// The window's size and position are kept by NSH itself (nsh.json window.restoreBounds). Off the desktops it does nothing.
import type { JSX } from "nsh";
import { cleanup, effect, platform, signal, untracked, View } from "nsh";
import { currentWindow, GlobalShortcut, isAvailable, Menu, type MenuItem, Tray } from "nsh/desktop";
import { store } from "../store";
import { app } from "../app";
import { errorText } from "../../lib/errors";
import { startPolls } from "../../lib/poll";
import { AppButton, Dialog, Text } from "../../ui";
import { signOut as signOutOfTheAccount } from "../../screens/settings/cloud";
import { aboutOpen, closePressed, perform, setSignOutHandler, showMain, toggleMain } from "./actions";
import { createHotkey } from "./hotkey";
import { hotkeyHeld, setHotkeyStatus } from "./hotkey_state";
import { menuNow as menuForState } from "./menu";
import { buildTrayMenu, COPYRIGHT, type Os } from "./menu_model";
import * as native from "./native";
import icon from "./tray.png";
import { updates } from "./updates";
import type { UpdaterApi } from "./updater";

/** The menu as it was last handed to the system (for the fixture that prints it, and for tests). */
const menuNow = signal<MenuItem[]>([]);
/** The tray's menu as it was last handed to the system. */
const trayNow = signal<MenuItem[]>([]);

/** The application menu now installed: read inside JSX or a closure. */
export const installedMenu = (): readonly MenuItem[] => menuNow();
/** The tray icon's menu now installed. */
export const installedTray = (): readonly MenuItem[] => trayNow();

/** The desktop platform this run is, or null where there are no desktop windows. */
export function desktopOs(): Os | null {
  if (!isAvailable || !platform.desktop) return null;
  return platform.os === "macos" || platform.os === "windows" || platform.os === "linux" ? platform.os : null;
}

/** How often the host reads what Rust has for it, in ms: a second launch comes forward and the banner's buttons act within this. */
const TICK_MS = 250;

function AboutDialog() {
  return (
    <Dialog testID="about" open={aboutOpen()} onOpenChange={(open) => aboutOpen.set(open)} width={320}>
      <View class="gap-1 p-5">
        <Text testID="about-title" variant="title">
          About eBIRForms
        </Text>
        <Text testID="about-version" tone="fg2">
          {"Version " + (store.systemInfo().app_version ?? "")}
        </Text>
        <Text testID="about-copyright" tone="fg2">
          {COPYRIGHT}
        </Text>
        <View class="flex-row justify-end pt-3">
          <AppButton testID="about-ok" label="OK" variant="primary" onPress={() => aboutOpen.set(false)} />
        </View>
      </View>
    </Dialog>
  );
}

/** What the hotkey takes of NSH's GlobalShortcut. */
export interface Shortcuts {
  register(shortcut: string, onPress: () => void): Promise<boolean>;
  unregister(shortcut: string): void;
}

/** Props of DesktopHost. */
export interface DesktopHostProps {
  /** A stand-in for nsh/updater (a test or a fixture): the real one is used without it. */
  updater?: UpdaterApi;
  /** A stand-in for GlobalShortcut (a headless test cannot see the system's): the real one is used without it. */
  shortcuts?: Shortcuts;
}

/** Mount once, under the app root. */
export function DesktopHost(props: DesktopHostProps): JSX.Element {
  const os = desktopOs();
  if (os === null) return <View />;
  if (props.updater) updates.use(props.updater);
  const win = currentWindow();

  // Closing: the window stays, the app hears it, and the platform's policy decides (hide to the tray, or quit through the frame).
  win.set({ closeOnPress: false });
  win.on("closePress", () => closePressed(os));

  // The application menu follows the state it depends on.
  const refreshes = signal(0);
  effect(() => {
    void refreshes();
    const items = menuForState(os);
    untracked(() => {
      Menu.setApplicationMenu(items);
      menuNow.set(items);
    });
  });
  cleanup(() => Menu.setApplicationMenu(null));

  // The menu's Sign Out is Settings › Account's: the host's session closes, the gate comes back, every screen's state is reset.
  setSignOutHandler(signOutOfTheAccount);
  cleanup(() => setSignOutHandler(null));

  // The tray: a click brings the window back, a secondary click opens the menu.
  const trayMenu = buildTrayMenu((action) => perform(action, os));
  trayNow.set(trayMenu);
  Tray.set({ icon, tooltip: "eBIRForms", onClick: () => showMain(), menu: trayMenu });
  cleanup(() => Tray.set(null));

  // The global show/hide hotkey: the stored combo, only while the filing chrome is up (the old listener let go on the sign-in screens).
  const hotkey = createHotkey({
    shortcutFor: native.hotkeyShortcut,
    register: (shortcut, onPress) => (props.shortcuts ?? GlobalShortcut).register(shortcut, onPress),
    unregister: (shortcut) => (props.shortcuts ?? GlobalShortcut).unregister(shortcut),
    onPress: toggleMain,
    log: (level, message) => store.logScreen(level, message),
    onStatus: setHotkeyStatus,
  });
  effect(() => {
    // The page may read only the theme setting; system_info carries the stored combo (and a store write drops it when Settings changes it).
    // While Settings' shortcut recorder listens (hotkey_state.ts holdHotkey) the combo is empty: the chord goes back to the system, so
    // pressing it to record it reaches the window instead of hiding it, and it is taken again (the new chord, or the old) when the hold ends.
    const combo = app.chromeVisible() && !hotkeyHeld() ? (store.systemInfo().global_hotkey ?? "") : "";
    untracked(() => {
      hotkey.sync(combo).catch((error) => store.logScreen("warn", "hotkey: " + errorText(error)));
    });
  });
  cleanup(() => hotkey.stop());

  // What Rust has for us: a second launch, the menu refresh counter, the updater's commands.
  updates.announce();
  let seenRevision = 0;
  cleanup(
    startPolls(
      [
        {
          name: "desktop",
          every: TICK_MS,
          first: TICK_MS,
          run: () => {
            const seen = native.poll();
            if (seen.activate) showMain();
            if (seen.menuRevision !== seenRevision) {
              seenRevision = seen.menuRevision;
              refreshes.update((n) => n + 1);
            }
            // Not awaited: a check or a download takes a while and the tick must go on (the driver runs one at a time).
            for (const command of seen.updateCommands) {
              updates.handle(command as Parameters<typeof updates.handle>[0]).catch((error) => store.logScreen("warn", "updater: " + errorText(error)));
            }
          },
        },
      ],
      (name, error) => store.logScreen("warn", "poll " + name + ": " + errorText(error)),
    ),
  );
  cleanup(() => updates.stop());

  return <AboutDialog />;
}

