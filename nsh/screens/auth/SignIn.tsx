// Sign-in (old app/shell/auth.js renderSignIn): at launch when the admin has a code, and after Lock app or Log out. One door:
// a TIN and that account's PIN or authenticator code; the admin's TIN signs in as the admin. "Forgot PIN?": a taxpayer is
// let in with the admin's code; the admin uses the one-time recovery key or, last, the Mac login. With no admin code there
// is nothing to protect: "Open without signing in".
import { View } from "nsh";
import { app } from "../../shell";
import { AppButton, PinCells, TextField, TinInput } from "../../ui";
import { signInLine } from "./logic";
import { Actions, AuthCard, ErrorLine, Heading, Labelled, TextLink } from "./parts";
import { auth } from "./state";

export function SignIn() {
  const forAdmin = () => auth.tin().length === 9 && auth.tin() === app.authState().admin_root;
  const line = () => signInLine({ forgot: auth.forgot(), forAdmin: forAdmin(), helpdesk: app.isHelpdesk() });
  const failed = () => Boolean(auth.error());
  return (
    <View testID="sign-in" class="min-h-0 flex-1">
      <AuthCard testID="auth-card">
        <Heading icon="lock" title="Sign in" line={line()} testID="auth-title" />
        <Labelled label="TIN">
          <TinInput testID="auth-tin" accessibilityLabel="TIN" placeholder="000-000-000" value={auth.tin()} onChange={auth.setTin} />
        </Labelled>
        <If when={!auth.forgot() && auth.method() === "pin"}>
          <Labelled label="PIN">
            <PinCells testID="auth-tp-pin" label="PIN" length={4} masked value={auth.tpPin()} onChange={auth.setTpPin} onComplete={(v) => { auth.setTpPin(v); auth.submitSignIn(); }} error={failed()} />
          </Labelled>
        </If>
        <If when={!auth.forgot() && auth.method() === "totp"}>
          <Labelled label="Authenticator code">
            <PinCells testID="auth-tp-totp" label="Authenticator code" length={6} value={auth.tpTotp()} onChange={auth.setTpTotp} onComplete={(v) => { auth.setTpTotp(v); auth.submitSignIn(); }} error={failed()} />
          </Labelled>
        </If>
        <If when={auth.forgot() && forAdmin()}>
          <Labelled label="Recovery key">
            <TextField
              testID="auth-recovery"
              accessibilityLabel="Recovery key"
              placeholder="XXXX-XXXX-XXXX-XXXX"
              autoCapitalize="characters"
              tabular
              value={auth.recovery()}
              onChange={auth.setRecovery}
              onSubmit={auth.submitRecoveryKey}
              error={failed()}
            />
          </Labelled>
        </If>
        <If when={auth.forgot() && !forAdmin() && app.authState().has_admin_pin}>
          <Labelled label="Admin PIN">
            <PinCells testID="auth-admin-pin" label="Admin PIN" length={4} masked value={auth.adminPin()} onChange={auth.setAdminPin} onComplete={(v) => { auth.setAdminPin(v); auth.submitOverride(); }} error={failed()} />
          </Labelled>
        </If>
        <If when={auth.forgot() && !forAdmin() && app.authState().has_admin_totp}>
          <Labelled label="Admin authenticator code">
            <PinCells testID="auth-admin-totp" label="Admin authenticator code" length={6} value={auth.adminTotp()} onChange={auth.setAdminTotp} onComplete={(v) => { auth.setAdminTotp(v); auth.submitOverride(); }} error={failed()} />
          </Labelled>
        </If>
        <ErrorLine message={auth.error()} />
        <Actions
          left={
            <View class="gap-1">
              <If when={!auth.forgot()}>
                <TextLink testID="auth-method" label={auth.method() === "totp" ? "Use the PIN instead" : "Use an authenticator code"} onPress={auth.toggleMethod} />
              </If>
              <TextLink testID="auth-forgot" label={auth.forgot() ? "I remember my PIN" : "Forgot PIN?"} onPress={auth.toggleForgot} />
            </View>
          }
          right={<Submit forAdmin={forAdmin()} />}
        />
        <If when={auth.forgot() && forAdmin() && app.authState().mac_login_available}>
          <View class="items-center">
            <AppButton
              testID="auth-mac-login"
              label={auth.busy() ? "Waiting for your Mac login…" : "Use my Mac login instead"}
              variant="ghost"
              size="sm"
              icon="laptop"
              disabled={auth.busy()}
              onPress={() => void auth.submitMacLogin()}
            />
          </View>
        </If>
        <If when={!app.adminHasCode()}>
          <View class="items-center">
            <AppButton testID="auth-open" label="Open without signing in" variant="ghost" size="sm" onPress={auth.openWithoutSignIn} />
          </View>
        </If>
      </AuthCard>
    </View>
  );
}

/** The card's main button: Sign in, Use key (the admin's recovery), or Open account (the admin opens a taxpayer). */
function Submit(props: { forAdmin: boolean }) {
  return (
    <>
      <If when={!auth.forgot()}>
        <AppButton testID="auth-submit" label="Sign in" variant="primary" onPress={auth.submitSignIn} />
      </If>
      <If when={auth.forgot() && props.forAdmin}>
        <AppButton testID="auth-recovery-submit" label="Use key" variant="primary" onPress={auth.submitRecoveryKey} />
      </If>
      <If when={auth.forgot() && !props.forAdmin}>
        <AppButton testID="auth-override-submit" label="Open account" variant="primary" onPress={auth.submitOverride} />
      </If>
    </>
  );
}
