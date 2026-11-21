// Settings > Computer (old computerCards; admin only): the local files, how long logs are kept, the global show hotkey, who the
// computer is set up for, and the version (how closing the window behaves is Settings > Keyboard now). The hotkey is a shortcut recorder (the box is pressed, then the
// keys): it stores the chord through set_global_hotkey, which the desktop host follows (shell/desktop/Desktop.tsx), holds that host's
// hotkey off while it listens (so the chord pressed to record it does not hide the window), and says where the registration stands.
import { cleanup, platform, View } from "nsh";
import { errorText } from "../../lib/errors";
import { comboToKeystroke, hotkeyProblem, hotkeyRegistrable, keystrokeToCombo, modifierWords } from "../../lib/settings_model";
import { holdHotkey, hotkeyStatus, store, toast } from "../../shell";
import { Card, Dropdown, SettingRow, ShortcutRecorder, Text, TOUCH } from "../../ui";
import { retentionChoices } from "./logic";
import { Labeled, NoteRow } from "./parts";
import { SetupCard } from "./SetupCard";

function setRetention(days: number): void {
  try {
    store.write("set_log_retention", days);
  } catch (error) {
    toast(errorText(error), "triangle-alert");
  }
}

/** Stores the recorded keystroke ("" removes the shortcut). A refusal is said in the host's words and the box shows the stored chord again. */
function setHotkey(keystroke: string): void {
  const combo = keystroke === "" ? "" : keystrokeToCombo(keystroke);
  if (combo === null) return;
  try {
    store.write("set_global_hotkey", combo);
  } catch (error) {
    toast(errorText(error), "triangle-alert");
  }
}

export function ComputerSection(props: { rows: string[] }) {
  const has = (id: string) => props.rows.includes(id);
  const info = () => store.systemInfo();
  const files = () => has("data") || has("logs");
  /** The stored chord as the recorder speaks it; the default modifiers of a bare stored key are the host's (system_info().os). */
  const hotkey = () => comboToKeystroke(info().global_hotkey, info().os) ?? "";
  // The recorder holds the desktop host's hotkey off while it listens; the hold is let go however the recording ends, and when this goes.
  let release: (() => void) | null = null;
  const hold = (on: boolean) => {
    release?.();
    release = on ? holdHotkey() : null;
  };
  cleanup(() => hold(false));
  return (
    <>
      <If when={files()}>
        <Labeled label="Local files">
          <Card testID="settings-files">
            <If when={has("data")}>
              <NoteRow testID="settings-database" title="Database" note={info().database_path || "Not available"} />
              <NoteRow testID="settings-data-dir" title="Data folder" note={info().data_dir || "Not available"} />
            </If>
            <If when={has("logs")}>
              <NoteRow testID="settings-logs" title="Logs folder" note={info().logs_dir || "Not available"} />
              <SettingRow testID="settings-retention-row" title="Keep logs for" hint="Older daily log files are deleted">
                <Dropdown
                  testID="settings-retention"
                  label="Keep logs for"
                  width={120}
                  options={retentionChoices(info())}
                  value={String(info().log_retention_days || 7)}
                  onChange={(v) => setRetention(Number(v))}
                />
              </SettingRow>
            </If>
          </Card>
        </Labeled>
      </If>
      <If when={has("hotkey")}>
        <Labeled label="Shortcuts">
          <Card testID="settings-hotkey">
            <SettingRow testID="settings-hotkey-row" title="Global show hotkey" hint="Shows or hides eBIRForms from any app">
              <ShortcutRecorder
                testID="settings-hotkey-recorder"
                label="Global show hotkey"
                width={160}
                value={hotkey()}
                onChange={setHotkey}
                accepts={hotkeyRegistrable}
                unacceptable={modifierWords(platform.os)}
                error={hotkeyProblem(hotkeyStatus())}
                onRecordingChange={hold}
              />
            </SettingRow>
          </Card>
        </Labeled>
      </If>
      <If when={has("setup")}>
        <Labeled label={TOUCH ? "Device setup" : "Computer setup"}>
          <SetupCard />
        </Labeled>
      </If>
      <If when={has("version")}>
        <View class="w-full">
          <Text testID="settings-version" variant="meta" tone="fg3">{`eBIRForms ${info().app_version ?? ""}`.trim()}</Text>
        </View>
      </If>
    </>
  );
}
