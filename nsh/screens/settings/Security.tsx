// Settings > Account > Security (old SecurityCard): My PIN (four cells, Set PIN or Change, Remove), My authenticator (Add, the key
// and six cells with Confirm and Cancel, Remove) and, for the admin, the Recovery key. One place for every lock; the Tax profile
// page has none. The lock screens that use these codes are D2's.
import { AppButton, Card, Code, PinCells, SettingRow } from "../../ui";
import { app } from "../../shell";
import { StackRow } from "./parts";
import { cancelTotp, clearPin, clearTotp, confirmTotp, newRecoveryKey, security, securityTarget, setPin, startTotp } from "./securityState";

export function SecurityCard() {
  const t = () => securityTarget();
  const helpdeskAdmin = () => t().admin && app.isHelpdesk();
  const pinLabel = () => (helpdeskAdmin() ? "Admin PIN" : "My PIN");
  const totpLabel = () => (helpdeskAdmin() ? "Admin authenticator" : "My authenticator");
  const pinHint = () => {
    if (!t().has_pin) return "Not set. Anyone at this computer can open this account";
    if (helpdeskAdmin()) return "Signs in TIN 000-000-000; opens locked-out taxpayers";
    return t().admin ? "Signs you in; opens locked-out taxpayers" : "Asked when you sign in with your TIN";
  };
  const totpHint = () => {
    const secret = security.totpSecret();
    if (secret) return `Add this key to your authenticator app, then type the code it shows: ${secret}`;
    return t().has_totp ? "A code from your authenticator app also signs you in" : "Use a 6-digit code from an authenticator app instead of the PIN";
  };
  const hasCode = () => t().has_pin || t().has_totp;
  const recoveryHint = () => {
    if (security.recoveryKey()) return "Write it down now. It is shown once and works once.";
    if (!hasCode()) return "Made when you set a PIN or authenticator";
    if (app.authState().has_recovery_key) return "Signs you in if you forget the PIN. Making a new one cancels the old one";
    return "None yet. Without one, only your Mac login can reset a forgotten PIN";
  };
  return (
    <Card testID="settings-security">
      <StackRow testID="settings-pin" title={pinLabel()} hint={pinHint()}>
        <PinCells testID="settings-pin-cells" length={4} masked label={pinLabel()} value={security.pin()} onChange={security.setPin} />
        <AppButton testID="settings-pin-save" label={t().has_pin ? "Change" : "Set PIN"} size="sm" onPress={setPin} />
        <If when={t().has_pin}>
          <AppButton testID="settings-pin-clear" label="Remove" variant="ghost" size="sm" onPress={clearPin} />
        </If>
      </StackRow>
      <StackRow testID="settings-totp" title={totpLabel()} hint={totpHint()}>
        <If
          when={security.totpSecret()}
          else={
            <AppButton
              testID="settings-totp-btn"
              label={t().has_totp ? "Remove" : "Add"}
              variant={t().has_totp ? "ghost" : "secondary"}
              size="sm"
              onPress={() => (t().has_totp ? clearTotp() : startTotp())}
            />
          }
        >
          <PinCells testID="settings-totp-cells" length={6} label={totpLabel()} value={security.totpCode()} onChange={security.setTotpCode} />
          <AppButton testID="settings-totp-confirm" label="Confirm" variant="primary" size="sm" onPress={confirmTotp} />
          <AppButton testID="settings-totp-cancel" label="Cancel" variant="ghost" size="sm" onPress={cancelTotp} />
        </If>
      </StackRow>
      <If when={t().admin}>
        <If when={hasCode() || security.recoveryKey()} else={<SettingRow testID="settings-recovery" title="Recovery key" hint={recoveryHint()} />}>
          <StackRow testID="settings-recovery" title="Recovery key" hint={recoveryHint()}>
            <If
              when={security.recoveryKey()}
              else={
                <AppButton
                  testID="settings-recovery-btn"
                  label={app.authState().has_recovery_key ? "Make a new key" : "Make a key"}
                  size="sm"
                  onPress={newRecoveryKey}
                />
              }
            >
              <Code testID="settings-recovery-key">{security.recoveryKey() ?? ""}</Code>
            </If>
          </StackRow>
        </If>
      </If>
    </Card>
  );
}
