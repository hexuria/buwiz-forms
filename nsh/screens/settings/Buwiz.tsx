// Settings > Account > Buwiz (old AccountCard): the cloud account. Separate from the PIN that unlocks this computer. Filing keeps
// working when the server is unreachable.
import { openUrl } from "nsh";
import { AppButton, Card, deviceCopy, Line, SettingRow } from "../../ui";
import { cloud, openAccount, signIn, signOut } from "./cloud";
import { accountHint, deletionPageUrl, serverHint, showsServer } from "./logic";

export function BuwizCard() {
  openAccount();
  const status = () => cloud.status();
  const who = () => (status().signed_in ? status().email || status().user_id || "Signed in" : "Not signed in");
  const hint = () => deviceCopy(cloud.busy() ? status().detail || "Waiting…" : cloud.failure() || accountHint(who(), status().detail || ""));
  return (
    <Card testID="settings-account">
      <If when={showsServer(status())}>
        <SettingRow testID="settings-account-server" title="Server" hint={serverHint(status().server_url_source)}>
          <Line testID="settings-account-server-value" variant="meta" tone="fg2" class="max-w-52">{status().server_url || "Not configured"}</Line>
        </SettingRow>
      </If>
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
      <If when={status().practice}>
        <SettingRow testID="settings-account-practice" title="Practice account" hint="Returns are simulated and never sent to the BIR" />
      </If>
      <If when={status().signed_in}>
        <SettingRow testID="settings-account-delete" title="Delete account" hint="Opens the Buwiz page where you delete this account. You confirm with a code sent to your email.">
          <AppButton
            testID="settings-account-delete-button"
            label="Open deletion page"
            variant="ghost"
            size="sm"
            onPress={() => void openUrl(deletionPageUrl(status().account_url))}
          />
        </SettingRow>
      </If>
    </Card>
  );
}
