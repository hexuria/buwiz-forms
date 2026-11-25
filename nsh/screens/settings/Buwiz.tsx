// Settings > Account > Buwiz (old AccountCard): the cloud account. Separate from the PIN that unlocks this computer. Filing keeps
// working when the server is unreachable. Signing in is the shared flow (screens/auth/signInFlow.ts): while the person approves,
// the card is that flow's waiting card (the code with Copy, Open the page again, Cancel), drawn as on the gate.
import { openUrl, View } from "nsh";
import { AppButton, Card, deviceCopy, Line, SettingRow, Text } from "../../ui";
import { FlowCardBody, StartButton } from "../auth/signInCards";
import { flowCard, signInFlow, startMessage } from "../auth/signInFlow";
import { cloud, openAccount, signIn, signOut } from "./cloud";
import { accountHint, deletionRequestUrl, serverHint, showsServer } from "./logic";

export function BuwizCard() {
  openAccount();
  const status = () => cloud.status();
  const card = () => (status().signed_in ? null : flowCard(signInFlow.state()));
  return (
    <If when={card()} else={<AccountCard />}>
      <Card testID="settings-account">
        <View testID="settings-account-wait" class="w-full gap-3 py-3.5">
          <Text variant="control" testID="settings-account-wait-title">{card()!.title}</Text>
          <FlowCardBody card={card()!} prefix="settings-account" />
        </View>
      </Card>
    </If>
  );
}

/** The account: who is signed in (or Sign in), the server, a practice account, deletion. */
function AccountCard() {
  const status = () => cloud.status();
  const who = () => (status().signed_in ? status().email || status().user_id || "Signed in" : "Not signed in");
  const hint = () => {
    const said = status().signed_in ? null : startMessage(signInFlow.state());
    return deviceCopy(said?.text ?? cloud.failure() ?? accountHint(who(), status().detail || ""));
  };
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
          else={<StartButton entry="sign-in" testID="settings-account-sign-in" label="Sign in" primary onStart={() => void signIn()} />}
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
        <SettingRow testID="settings-account-delete" title="Delete account" hint="Writes an email to support asking to delete this Buwiz account">
          <AppButton
            testID="settings-account-delete-button"
            label="Request deletion"
            variant="ghost"
            size="sm"
            onPress={() => void openUrl(deletionRequestUrl(status().email))}
          />
        </SettingRow>
      </If>
    </Card>
  );
}
