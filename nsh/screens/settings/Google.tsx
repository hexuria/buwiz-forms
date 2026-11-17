// Settings > Account > Google (old googleBlock): Gmail connect or reconnect, receipt tracking, Disconnect, and the line the last
// job left. The writes and the "one inbox job at a time" state are the taxpayer screens' (screens/taxpayer/actions.ts), so the
// Tax profile's confirmation inbox and this card never disagree.
import { View } from "nsh";
import { app, store } from "../../shell";
import { AppButton, Card, SettingRow, Switch, Text } from "../../ui";
import { connectInbox, disconnectInbox, setTracking, shared } from "../taxpayer/actions";
import { Labeled, NoteRow } from "./parts";

export function GoogleBlock() {
  const branch = () => app.branch();
  const state = () => store.inboxState(branch()?.tin);
  const connected = () => Boolean(state()?.oauth_connected);
  const who = () => state()?.oauth_inbox_email || state()?.inbox_email || "";
  return (
    <If
      when={branch()}
      else={
        <Labeled label="Google">
          <Card testID="settings-google">
            <NoteRow testID="settings-google-empty" title="No taxpayer yet" note="Add a taxpayer before connecting a Gmail account." />
          </Card>
        </Labeled>
      }
    >
      <If
        when={state()}
        else={
          <Labeled label="Google">
            <Card testID="settings-google">
              <NoteRow testID="settings-google-error" title="Google account" note="This profile's Gmail state could not be read." />
            </Card>
          </Labeled>
        }
      >
        <View class="w-full gap-4">
          <Labeled label="Google">
            <Card testID="settings-google">
              <SettingRow
                testID="settings-google-gmail"
                title="Gmail"
                hint={connected() ? `Connected as ${who()}` : `Not connected. Reads BIR confirmation mail for ${branch()?.name || "this profile"}.`}
              >
                <AppButton
                  testID="settings-google-connect"
                  label={connected() ? "Reconnect" : "Connect"}
                  size="sm"
                  icon="key-round"
                  disabled={shared.inboxBusy()}
                  onPress={() => void connectInbox(branch()?.tin ?? "")}
                />
              </SettingRow>
              <SettingRow testID="settings-google-tracking" title="Receipt tracking" hint="Watch this inbox for BIR confirmation mail">
                <Switch
                  testID="settings-google-tracking-switch"
                  label="Receipt tracking"
                  checked={Boolean(state()?.tracking_enabled)}
                  onChange={(v) => setTracking(branch()?.tin ?? "", v)}
                />
              </SettingRow>
            </Card>
          </Labeled>
          <If when={connected()}>
            <Card testID="settings-google-disconnect-card">
              <SettingRow testID="settings-google-disconnect-row" title="Disconnect Google" hint="Clears the grant for every profile that shares this inbox">
                <AppButton
                  testID="settings-google-disconnect"
                  label="Disconnect"
                  variant="danger"
                  size="sm"
                  onPress={() => disconnectInbox(branch()?.tin ?? "")}
                />
              </SettingRow>
            </Card>
          </If>
          <If when={shared.inboxMsg()}>
            <Text testID="settings-google-message" variant="meta" tone={shared.inboxMsg()?.ok ? "success" : "danger"}>{shared.inboxMsg()?.text ?? ""}</Text>
          </If>
        </View>
      </If>
    </If>
  );
}
