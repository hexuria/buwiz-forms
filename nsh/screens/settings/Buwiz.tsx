// Settings > Account > Buwiz account (old AccountCard): the cloud account. Separate from the profile on this computer (the PIN
// that unlocks it, Identity.tsx). Filing keeps working when the server is unreachable. Signing in happens on the gate only
// (screens/auth/Gate.tsx): Sign in here shows it. Sign out of Buwiz asks once first, since signing back in costs an email and a
// code.
import { openUrl, signal, View } from "nsh";
import { AppButton, Card, deviceCopy, Dialog, Line, SettingRow, Text } from "../../ui";
import { cloud, openAccount, returnsWaitingCount, signIn, signOut } from "./cloud";
import { accountHint, deletionPageUrl, serverHint, showsServer } from "./logic";

/** What signing out of Buwiz does, said before it does it ("this device" on touch). */
export const SIGN_OUT_SENTENCE = "You will go back to the sign-in page. Your drafts and returns stay on this computer.";

/** The account: who is signed in (or Sign in), the server, a practice account, deletion. */
export function BuwizCard() {
  openAccount();
  const asking = signal(false);
  const status = () => cloud.status();
  const who = () => (status().signed_in ? status().email || status().user_id || "Signed in" : "Not signed in to Buwiz");
  const hint = () => deviceCopy(cloud.failure() ?? accountHint(status().signed_in ? who() : "Not signed in", status().detail || ""));
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
            label={cloud.busy() ? "Signing out…" : "Sign out of Buwiz"}
            variant="ghost"
            size="sm"
            disabled={cloud.busy()}
            onPress={() => asking.set(true)}
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
      <Dialog testID="settings-sign-out-confirm" width={420} open={asking()} onOpenChange={(open) => !open && asking.set(false)}>
        <View class="gap-3 p-5">
          <Text variant="title" testID="settings-sign-out-sentence">{deviceCopy(SIGN_OUT_SENTENCE)}</Text>
          {/* §A.5: the count, and no names, while returns wait to send. */}
          <If when={returnsWaitingCount() > 0}>
            <Text variant="meta" tone="fg2" testID="sign-out-waiting-count">
              {`${returnsWaitingCount()} returns are waiting to send. They send when you sign in again.`}
            </Text>
          </If>
          <AppButton
            testID="settings-sign-out-yes"
            variant="primary"
            block
            label="Sign out of Buwiz"
            onPress={() => {
              asking.set(false);
              void signOut();
            }}
          />
          <AppButton testID="settings-sign-out-cancel" block label="Cancel" onPress={() => asking.set(false)} />
        </View>
      </Dialog>
    </Card>
  );
}
