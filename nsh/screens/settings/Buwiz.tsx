// Settings > Account > Buwiz (old AccountCard): the cloud account. Separate from the PIN that unlocks this computer. Filing keeps
// working when the server is unreachable.
import { AppButton, Card, deviceCopy, Line, SettingRow } from "../../ui";
import { cloud, openAccount, signIn, signOut } from "./cloud";
import { serverHint } from "./logic";

export function BuwizCard() {
  openAccount();
  const status = () => cloud.status();
  const who = () => (status().signed_in ? status().email || status().user_id || "Signed in" : "Not signed in");
  const hint = () => deviceCopy(cloud.busy() ? status().detail || "Waiting…" : cloud.failure() || status().detail || "");
  return (
    <Card testID="settings-account">
      <SettingRow testID="settings-account-server" title="Server" hint={serverHint(status().server_url_source)}>
        <Line testID="settings-account-server-value" variant="meta" tone="fg2" class="max-w-52">{status().server_url || "Not configured"}</Line>
      </SettingRow>
      <SettingRow testID="settings-account-who" title={who()} hint={hint()}>
        <If
          when={status().signed_in}
          else={
            <AppButton
              testID="settings-account-sign-in"
              label={cloud.busy() ? "Waiting…" : "Sign in"}
              variant="primary"
              size="sm"
              disabled={cloud.busy()}
              onPress={() => void signIn()}
            />
          }
        >
          <AppButton
            testID="settings-account-sign-out"
            label={cloud.busy() ? "Signing out…" : "Sign out"}
            variant="ghost"
            size="sm"
            disabled={cloud.busy()}
            onPress={() => void signOut()}
          />
        </If>
      </SettingRow>
    </Card>
  );
}
