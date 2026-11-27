// Sign-in (the old shell's auth.js renderSignIn): at launch when the admin has a code, and after Lock app or Log out. One door:
// a TIN and that account's PIN or authenticator code; the admin's TIN signs in as the admin. "Forgot PIN?": a taxpayer is
// let in with the admin's code; the admin uses the one-time recovery key or, last, the Mac login. With no admin code there
// is nothing to protect: "Open without signing in".
import { cleanup, keymap, signal, View } from "nsh";
import { app } from "../../shell";
import { AppButton, DigitCells, PinCells, TextField } from "../../ui";
import { signInLine } from "./logic";
import { Actions, AuthCard, ErrorLine, Heading, Labelled, TextLink } from "./parts";
import { auth } from "./state";

/** The code boxes that can follow the TIN, by name: the screen shows one set at a time (the admin's two can show together). */
type CodeBox = "pin" | "totp" | "recovery" | "adminPin" | "adminTotp";

export function SignIn() {
  const forAdmin = () => auth.tin().length === 9 && auth.tin() === app.authState().admin_root;
  const line = () => signInLine({ forgot: auth.forgot(), forAdmin: forAdmin(), helpdesk: app.isHelpdesk() });
  const failed = () => Boolean(auth.error());
  // The TIN's ninth digit hands the keys to the code box that shows, and Backspace in an empty code box hands them back (like the
  // cells of one code: a digit forward at the end, a step back at the start).
  const focusOf: Partial<Record<CodeBox, () => void>> = {};
  let focusTin: (() => void) | undefined;
  const inBox = signal<CodeBox | null>(null);
  const valueOf = (box: CodeBox): string => ({ pin: auth.tpPin(), totp: auth.tpTotp(), recovery: auth.recovery(), adminPin: auth.adminPin(), adminTotp: auth.adminTotp() })[box];
  const into = (box: CodeBox) => () => inBox.set(box);
  const outOf = (box: CodeBox) => () => {
    if (inBox() === box) inBox.set(null);
  };
  const nextBox = (): CodeBox => {
    if (!auth.forgot()) return auth.method() === "pin" ? "pin" : "totp";
    if (forAdmin()) return "recovery";
    return app.authState().has_admin_pin ? "adminPin" : "adminTotp";
  };
  cleanup(
    keymap.bind(
      "backspace",
      () => {
        const box = inBox();
        if (box !== null && valueOf(box) === "") focusTin?.();
      },
      { when: "always", passThrough: true },
    ),
  );
  return (
    <View testID="sign-in" class="min-h-0 flex-1">
      <AuthCard testID="auth-card">
        <Heading title="Sign in" line={line()} testID="auth-title" />
        <Labelled label="TIN">
          <DigitCells
            testID="auth-tin"
            label="TIN"
            length={9}
            groups={3}
            value={auth.tin()}
            onChange={auth.setTin}
            onComplete={() => focusOf[nextBox()]?.()}
            focusRef={(f) => (focusTin = f)}
          />
        </Labelled>
        <If when={!auth.forgot() && auth.method() === "pin"}>
          <Labelled label="PIN">
            <PinCells testID="auth-tp-pin" label="PIN" length={4} masked value={auth.tpPin()} onChange={auth.setTpPin} onComplete={(v) => { auth.setTpPin(v); auth.submitSignIn(); }} error={failed()} focusRef={(f) => (focusOf.pin = f)} onFocus={into("pin")} onBlur={outOf("pin")} />
          </Labelled>
        </If>
        <If when={!auth.forgot() && auth.method() === "totp"}>
          <Labelled label="Authenticator code">
            <PinCells testID="auth-tp-totp" label="Authenticator code" length={6} value={auth.tpTotp()} onChange={auth.setTpTotp} onComplete={(v) => { auth.setTpTotp(v); auth.submitSignIn(); }} error={failed()} focusRef={(f) => (focusOf.totp = f)} onFocus={into("totp")} onBlur={outOf("totp")} />
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
              inputRef={(r) => (focusOf.recovery = () => r.focus())}
              onFocus={into("recovery")}
              onBlur={outOf("recovery")}
            />
          </Labelled>
        </If>
        <If when={auth.forgot() && !forAdmin() && app.authState().has_admin_pin}>
          <Labelled label="Admin PIN">
            <PinCells testID="auth-admin-pin" label="Admin PIN" length={4} masked value={auth.adminPin()} onChange={auth.setAdminPin} onComplete={(v) => { auth.setAdminPin(v); auth.submitOverride(); }} error={failed()} focusRef={(f) => (focusOf.adminPin = f)} onFocus={into("adminPin")} onBlur={outOf("adminPin")} />
          </Labelled>
        </If>
        <If when={auth.forgot() && !forAdmin() && app.authState().has_admin_totp}>
          <Labelled label="Admin authenticator code">
            <PinCells testID="auth-admin-totp" label="Admin authenticator code" length={6} value={auth.adminTotp()} onChange={auth.setAdminTotp} onComplete={(v) => { auth.setAdminTotp(v); auth.submitOverride(); }} error={failed()} focusRef={(f) => (focusOf.adminTotp = f)} onFocus={into("adminTotp")} onBlur={outOf("adminTotp")} />
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
