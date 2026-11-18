// Settings > Keyboard: the quit and hide keys (shortcut recorders: the box is pressed, then the keys) and what Windows' and Linux's close
// button does (Ask, Hide to tray, Quit; "Don't ask again" in the close question sets it too). The menus show the keys chosen here
// (shell/desktop/menu_model.ts). A chord another item, the frame or the global show hotkey holds is refused under the box, so no key is
// ever bound twice. While a box listens the global hotkey is held off, and the application menu's Quit and Hide items hand it the
// platform's own quit and hide keys (Cmd+Q, Cmd+H, Ctrl+Q), which nsh/keymap cannot bind: so Cmd+Q can be made Hide, say.
import { cleanup, platform, signal } from "nsh";
import { comboToKeystroke, modifierWords } from "../../lib/settings_model";
import { menuOs } from "../../shell/chrome";
import {
  appKeyUsable, captureMenuKeys, CLOSE_CHOICES, closePrefs, holdHotkey, keyHolder, loadClosePrefs, setAppKey, setCloseChoice, store,
  type AppKeys, type CloseChoice,
} from "../../shell";
import { Card, Dropdown, SettingRow, ShortcutRecorder } from "../../ui";
import { Labeled, NoteRow } from "./parts";

/** The platform as the menus know it (shell/chrome.ts menuOs); iOS and Android have no such keys: they read as macOS and Linux, and
 * nothing is bound there. */
function os(): "macos" | "windows" | "linux" {
  return menuOs() ?? (platform.os === "ios" ? "macos" : "linux");
}

/** One recorder for the quit or the hide key. */
function KeyRow(props: { which: keyof AppKeys; title: string; hint: string }) {
  const problem = signal("");
  const value = () => closePrefs().keys[props.which];
  const globalHotkey = () => {
    const info = store.systemInfo();
    return comboToKeystroke(info.global_hotkey, info.os) ?? "";
  };
  /** Free and usable; otherwise the words the box shows while it keeps listening. */
  const accepts = (keystroke: string): boolean => {
    if (!appKeyUsable(keystroke)) {
      problem.set(modifierWords(platform.os));
      return false;
    }
    const holder = keyHolder(os(), props.which, keystroke, closePrefs().keys, globalHotkey());
    if (holder) {
      problem.set(`That shortcut is already ${holder}.`);
      return false;
    }
    problem.set("");
    return true;
  };
  // While the box listens: the global hotkey is let go (its chord could be the one pressed) and the menu's Quit and Hide hand their keys here.
  let release: (() => void) | null = null;
  const hold = (on: boolean) => {
    release?.();
    release = on ? holdHotkey() : null;
  };
  cleanup(() => hold(false));
  return (
    <SettingRow testID={`settings-${props.which}-row`} title={props.title} hint={props.hint}>
      <ShortcutRecorder
        testID={`settings-${props.which}-key`}
        label={props.title}
        width={160}
        value={value()}
        onChange={(keystroke) => setAppKey(props.which, keystroke)}
        accepts={accepts}
        unacceptable={problem()}
        onRecordingChange={hold}
        listen={captureMenuKeys}
      />
    </SettingRow>
  );
}

export function KeyboardSection(props: { rows: string[] }) {
  const has = (id: string) => props.rows.includes(id);
  void loadClosePrefs();
  return (
    <>
      <If when={has("quit") || has("hide")}>
        <Labeled label="Quit and hide">
          <Card testID="settings-keys">
            <If when={has("quit")}>
              <KeyRow which="quit" title="Quit eBIRForms" hint="Stops everything." />
            </If>
            <If when={has("hide")}>
              <KeyRow which="hide" title="Hide eBIRForms" hint="Hides to the tray. Never asks." />
            </If>
          </Card>
        </Labeled>
      </If>
      <If when={has("close")}>
        <Labeled label="Window">
          <Card testID="settings-close">
            <If
              when={os() !== "macos"}
              else={<NoteRow testID="settings-close-row" title="Close window" note="The red button and ⌘W hide eBIRForms to the menu bar. Quit is always in the eBIRForms menu and the menu-bar icon." />}
            >
              <SettingRow testID="settings-close-row" title="Close button (✕)" hint="What ✕ and Alt+F4 do. Your Hide key never asks.">
                <Dropdown
                  testID="settings-close-choice"
                  label="Close button"
                  width={160}
                  options={CLOSE_CHOICES.map((c) => ({ value: c.value, label: c.label }))}
                  value={closePrefs().close}
                  onChange={(v) => setCloseChoice(v as CloseChoice)}
                />
              </SettingRow>
            </If>
          </Card>
        </Labeled>
      </If>
    </>
  );
}
