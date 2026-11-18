// The desktop host: what makes the app a desktop app. One component, mounted once by the app root (App.tsx), drawing nothing but the
// About dialog and the close and quit questions. It owns
//   * the application menu (menu_model.ts), rebuilt when the state it depends on changes: the gate (signed in or not), whether there
//     is a taxpayer, the view, and the host's own refresh requests (src/desktop.rs: refresh_menu, set_shell_chrome_open);
//   * the tray icon and its menu (Show / Hide / Quit, Show or Hide disabled to match the window), and the close button: the window
//     stays and closePolicy decides (macOS hides, Windows and Linux ask "Hide to tray or Quit?" or do the remembered choice);
//   * the two questions it draws: that close question, and the warning before a quit while returns are still sending;
//   * the global show/hide hotkey, from the stored setting global_hotkey_key;
//   * a short tick that reads what Rust has for it: a second launch asking for the window, the updater commands the host functions
//     queued (the banner's Install now and Later, "Check for Updates…", the Download automatically switch);
//   * the updater driver (updates.ts).
// The window's size and position are kept by NSH itself (nsh.json window.restoreBounds). Off the desktops it does nothing.
import type { JSX } from "nsh";
import { cleanup, computed, effect, keymap, signal, untracked, View } from "nsh";
import { currentWindow, GlobalShortcut, Menu, type MenuItem, Tray } from "nsh/desktop";
import { menuOs } from "../chrome";
import { store } from "../store";
import { app } from "../app";
import { errorText } from "../../lib/errors";
import { startPolls } from "../../lib/poll";
import { AppButton, Checkbox, Dialog, Text } from "../../ui";
import { signOut as signOutOfTheAccount } from "../../screens/settings/cloud";
import {
  aboutOpen, answerClose, answerSending, closeAsk, closePressed, listenToQuitRequests, mainWindow, perform, sendingAsk, setSignOutHandler,
  showMain, toggleMain, trayShows, useSendingCount, windowShown,
} from "./actions";
import { loadClosePrefs } from "./app_keys";
import { createHotkey } from "./hotkey";
import { hotkeyHeld, setHotkeyStatus } from "./hotkey_state";
import { menuNow as menuForState } from "./menu";
import { APP_NAME, buildTrayMenu, COPYRIGHT, sendingWarning } from "./menu_model";
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

/** The desktop platform this run is, or null where there are no desktop windows: the chrome's menuOs (shell/chrome.ts), one definition. */
export const desktopOs = menuOs;

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

/** Binds `keys` while `open()` holds (a question of the host's is up), and lets them go when it closes or the host goes. */
function whileOpen(open: () => boolean, keys: Record<string, () => void>): void {
  let unbind: (() => void) | null = null;
  effect(() => {
    const up = open();
    untracked(() => {
      unbind?.();
      unbind = up ? keymap.bind(keys, { when: "always" }) : null;
    });
  });
  cleanup(() => unbind?.());
}

/**
 * "Close eBIRForms?" (Windows and Linux, the close button and Alt+F4): Quit or Hide to tray, and "Don't ask again". Enter hides, Esc keeps
 * the window. With no tray icon (Linux without a panel that hosts one: GNOME without the AppIndicator extension) a hidden window could not
 * come back: the question offers Quit and Cancel, Quit is the default (Enter), and it says why.
 */
function CloseDialog() {
  const remember = signal(false);
  const answer = (choice: "hide" | "quit" | null) => {
    const kept = remember();
    remember.set(false);
    answerClose(choice, kept);
  };
  // While the question is up: Enter is the default (Hide to tray, or Quit without a tray; a focused button still takes its own Enter),
  // Esc keeps the window (before the frame's own Escape, which knows nothing of this dialog).
  whileOpen(closeAsk, { enter: () => answer(trayShows() ? "hide" : "quit"), escape: () => answer(null) });
  return (
    <Dialog testID="close-ask" width={400} open={closeAsk()} onOpenChange={(open) => !open && answer(null)}>
      <View class="gap-3 p-5">
        <View class="gap-1">
          <Text variant="title">{"Close " + APP_NAME + "?"}</Text>
          <If
            when={trayShows()}
            else={
              <Text testID="close-ask-no-tray" tone="fg2">
                This desktop shows no tray icons, so a hidden window could not be brought back. Quit stops sending your queued returns and
                reminders.
              </Text>
            }
          >
            <Text tone="fg2">Hide to tray keeps sending your queued returns and reminders. Quit stops everything.</Text>
          </If>
        </View>
        <View class="flex-row items-center gap-2">
          <Checkbox testID="close-ask-remember" label="Don't ask again" checked={remember()} onChange={(on) => remember.set(on)} />
          <Text variant="meta">Don't ask again. You can change this in Settings.</Text>
        </View>
        <View class="flex-row justify-end gap-2 pt-1">
          <If
            when={trayShows()}
            else={
              <>
                <AppButton testID="close-ask-cancel" label="Cancel" onPress={() => answer(null)} />
                <AppButton testID="close-ask-quit" variant="primary" label="Quit" onPress={() => answer("quit")} />
              </>
            }
          >
            <AppButton testID="close-ask-quit" variant="danger" label="Quit" onPress={() => answer("quit")} />
            <AppButton testID="close-ask-hide" variant="primary" label="Hide to tray" onPress={() => answer("hide")} />
          </If>
        </View>
      </View>
    </Dialog>
  );
}

/** "2 returns are still sending" before any quit: Quit anyway, Wait (stay open), Hide to tray (keep sending in the background). Esc is Wait. */
function SendingDialog() {
  const words = () => sendingWarning(sendingAsk());
  whileOpen(() => sendingAsk() > 0, { escape: () => answerSending("wait") });
  return (
    <Dialog testID="sending-ask" width={420} open={sendingAsk() > 0} onOpenChange={(open) => !open && answerSending("wait")}>
      <View class="gap-3 p-5">
        <View class="gap-1">
          <Text testID="sending-ask-title" variant="title">{words()?.title ?? ""}</Text>
          <Text tone="fg2">{words()?.message ?? ""}</Text>
        </View>
        <View class="flex-row justify-end gap-2 pt-1">
          <AppButton testID="sending-ask-quit" variant="danger" label="Quit anyway" onPress={() => answerSending("quit")} />
          <AppButton testID="sending-ask-wait" label="Wait" onPress={() => answerSending("wait")} />
          <If when={trayShows()}>
            <AppButton testID="sending-ask-hide" variant="primary" label="Hide to tray" onPress={() => answerSending("hide")} />
          </If>
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
  /** A stand-in for how many returns are still sending (a test): the Queued returns in the database without it. */
  sending?: () => number;
}

/** Mount once, under the app root. */
export function DesktopHost(props: DesktopHostProps): JSX.Element {
  const os = desktopOs();
  if (os === null) return <View />;
  if (props.updater) updates.use(props.updater);
  const win = currentWindow();

  // The user's quit and hide keys and the close button's remembered choice (Settings > Keyboard), for the menu and the close button.
  void loadClosePrefs();
  if (props.sending) {
    useSendingCount(props.sending);
    cleanup(() => useSendingCount(null));
  }

  // Closing: the window stays, the app hears it, and closePolicy decides (hide, ask "Hide to tray or Quit?", or quit through the frame).
  win.set({ closeOnPress: false });
  win.on("closePress", (event) => closePressed(os, event.reason));
  // The platform's own quits (the Dock's Quit, a logout on macOS, Ctrl+Q) run the same check as the menu's Quit (actions.ts).
  cleanup(listenToQuitRequests());

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

  // The tray: a click brings the window back, a secondary click opens Show / Hide / Quit (Show or Hide disabled to match the window).
  // Only whether it is on screen: a move or a resize changes the window's state too, and must not set the tray again.
  const visible = computed(() => mainWindow()?.state()?.visible ?? true);
  effect(() => {
    const up = visible();
    untracked(() => {
      // macOS: every show brings the Dock icon back, NSH's own reopen (the Finder, Spotlight, Launchpad) included.
      if (up) windowShown();
      const trayMenu = buildTrayMenu((action) => perform(action, os), up);
      trayNow.set(trayMenu);
      Tray.set({ icon, tooltip: "eBIRForms", onClick: () => showMain(), menu: trayMenu });
    });
  });
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

  return (
    <>
      <AboutDialog />
      <CloseDialog />
      <SendingDialog />
    </>
  );
}

