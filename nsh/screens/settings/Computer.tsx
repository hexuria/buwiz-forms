// Settings > Computer (old computerCards; admin only): the local files, how long logs are kept, the global show hotkey, how closing
// the window behaves, who the computer is set up for, and the version. The hotkey's registration is the app menu agent's hook:
// this stores the chord through set_global_hotkey, which the tray already polls.
import { View } from "nsh";
import { errorText } from "../../lib/errors";
import { app, store, toast } from "../../shell";
import { Card, Dropdown, SettingRow, Text } from "../../ui";
import { hotkeyChoices, OFF, retentionChoices } from "./logic";
import { Labeled, NoteRow } from "./parts";
import { SetupCard } from "./SetupCard";

function setRetention(days: number): void {
  try {
    store.write("set_log_retention", days);
  } catch (error) {
    toast(errorText(error), "triangle-alert");
  }
}

function setHotkey(combo: string): void {
  try {
    store.write("set_global_hotkey", combo === OFF ? "" : combo);
  } catch (error) {
    toast(errorText(error), "triangle-alert");
  }
}

export function ComputerSection(props: { rows: string[] }) {
  const has = (id: string) => props.rows.includes(id);
  const info = () => store.systemInfo();
  const files = () => has("data") || has("logs");
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
              <Dropdown
                testID="settings-hotkey-select"
                label="Global show hotkey"
                width={160}
                options={hotkeyChoices(info().global_hotkey, info().hotkey_presets)}
                value={info().global_hotkey || OFF}
                onChange={setHotkey}
              />
            </SettingRow>
          </Card>
        </Labeled>
      </If>
      <If when={has("tray")}>
        <Labeled label="Window">
          <Card testID="settings-tray">
            <NoteRow
              testID="settings-tray-row"
              title="Close window"
              note="Closing hides eBIRForms to the menu bar when a tray icon exists. With no tray, closing quits. Quit is always in the application menu."
            />
          </Card>
        </Labeled>
      </If>
      <If when={has("setup")}>
        <Labeled label="Computer setup">
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
