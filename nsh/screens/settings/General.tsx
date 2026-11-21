// Settings > General: Theme, Show recent returns, Compact form sections, Download updates automatically (old generalCards).
import { effect, signal, untracked } from "nsh";
import { bir } from "../../bir";
import { errorText } from "../../lib/errors";
import { THEME_OPTIONS, type ThemeOption } from "../../lib/settings_model";
import { app, store, toast } from "../../shell";
import { Card, Dropdown, SettingRow, Switch } from "../../ui";
import { Labeled } from "./parts";

/** What the person chose for automatic updates until the poll reports the host's own answer. */
const chosen = signal<boolean | null>(null);

/** Whether a newer release downloads by itself: the choice just made, else the host's last status, else what it says now. */
function autoDownload(): boolean {
  return chosen() ?? app.updateStatus()?.auto_download ?? store.read("update_auto_download", () => bir.update_status().auto_download, true);
}

function setAutoDownload(on: boolean): void {
  try {
    store.writeFor("update_auto_download", "set_auto_update_download", on);
    chosen.set(on);
  } catch (error) {
    toast(errorText(error), "triangle-alert");
  }
}

export function GeneralSection(props: { rows: string[] }) {
  const has = (id: string) => props.rows.includes(id);
  // The poll's word on the setting replaces the choice made here (the old app wrote its choice into the polled status).
  effect(() => {
    app.updateStatus();
    untracked(() => chosen.set(null));
  });
  return (
    <If when={props.rows.length}>
      <Labeled label="Appearance">
        <Card testID="settings-appearance">
          <If when={has("theme")}>
            <SettingRow testID="settings-theme-row" title="Theme" hint="System follows this device">
              <Dropdown
                testID="settings-theme"
                label="Theme"
                width={120}
                options={THEME_OPTIONS.map((o) => ({ value: o.value, label: o.label }))}
                value={app.themePref()}
                onChange={(v) => app.setThemePreference(v as ThemeOption["value"])}
              />
            </SettingRow>
          </If>
          <If when={has("recent")}>
            <SettingRow testID="settings-recent" title="Show recent returns" hint="The last four returns in the sidebar">
              <Switch testID="settings-recent-switch" label="Show recent returns" checked={app.prefs().showRecent} onChange={(v) => app.setPref("showRecent", v)} />
            </SettingRow>
          </If>
          <If when={has("compact")}>
            <SettingRow testID="settings-compact-form-nav" title="Compact form sections" hint="One line per part; point at a part for its name">
              <Switch
                testID="settings-compact-form-nav-switch"
                label="Compact form sections"
                checked={app.prefs().compactFormNav}
                onChange={(v) => app.setPref("compactFormNav", v)}
              />
            </SettingRow>
          </If>
          <If when={has("updates")}>
            <SettingRow testID="settings-auto-update" title="Download updates automatically" hint="A newer release downloads in the background. You choose when to install.">
              <Switch testID="settings-auto-update-switch" label="Download updates automatically" checked={autoDownload()} onChange={setAutoDownload} />
            </SettingRow>
          </If>
        </Card>
      </Labeled>
    </If>
  );
}
