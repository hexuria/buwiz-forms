// Settings > Account > Buwiz (old AccountCard): the cloud account. Separate from the PIN that unlocks this computer. Filing keeps
// working when the server is unreachable. Signing in is the shared flow (screens/auth/signInFlow.ts): Sign in opens its email page
// here, then its code page, drawn as on the gate.
import { openUrl, View } from "nsh";
import { AppButton, Card, deviceCopy, Line, SettingRow, Text } from "../../ui";
import { CODE_TITLE, CodeCardBody, EMAIL_TITLE, EmailCardBody } from "../auth/signInCards";
import { pageOf, signInFlow } from "../auth/signInFlow";
import { cloud, openAccount, signIn, signOut } from "./cloud";
import { accountHint, deletionRequestUrl, serverHint, showsServer } from "./logic";

/** Whether the flow's state draws a page of its own here (not the account row). */
function inFlow(): boolean {
  const kind = signInFlow.state().kind;
  return kind === "email" || kind === "sending" || kind === "code";
}

export function BuwizCard() {
  openAccount();
  const status = () => cloud.status();
  const shown = () => (status().signed_in || !inFlow() ? null : pageOf(signInFlow.state()));
  return (
    <If when={shown()} else={<AccountCard />}>
      <Card testID="settings-account">
        <If when={shown() === "code"}>
          <View testID="settings-account-code" class="w-full gap-3 py-3.5">
            <Text variant="control" testID="settings-account-code-title">{CODE_TITLE}</Text>
            <CodeCardBody prefix="settings-account" />
          </View>
        </If>
        <If when={shown() === "email"}>
          <View testID="settings-account-email-page" class="w-full gap-3 py-3.5">
            <Text variant="control" testID="settings-account-email-title">{EMAIL_TITLE}</Text>
            <EmailCardBody prefix="settings-account" />
          </View>
        </If>
      </Card>
    </If>
  );
}

/** The account: who is signed in (or Sign in), the server, a practice account, deletion. */
function AccountCard() {
  const status = () => cloud.status();
  const who = () => (status().signed_in ? status().email || status().user_id || "Signed in" : "Not signed in");
  const hint = () => deviceCopy(cloud.failure() ?? accountHint(who(), status().detail || ""));
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
          else={<AppButton testID="settings-account-sign-in" label="Sign in" variant="primary" size="sm" onPress={signIn} />}
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
