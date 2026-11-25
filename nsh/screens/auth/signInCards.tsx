// The pieces of the sign-in flow's two pages (signInFlow.ts), drawn the same on the account gate and in Settings > Account: the
// email page (the address and "Continue with email") and the code page (six boxes that check by themselves, Resend with its
// countdown, Change email, the line about the mail's button).
import { View } from "nsh";
import { AppButton, deviceCopy, PinCells, Spinner, Text, TextField, TOUCH } from "../../ui";
import { codeSaid, emailSaid, resendLabel, signInFlow, type SignInState } from "./signInFlow";

/** The email page's title, its sentence and its one button. */
export const EMAIL_TITLE = "Sign in to eBIRForms";
export const EMAIL_SENTENCE = "Enter your email and we will send you a 6-digit code to sign in, or to create your Buwiz account if you are new.";
export const EMAIL_BUTTON = "Continue with email";
/** The code page's title and its line about the mail's button. */
export const CODE_TITLE = "Enter the code";
export const MAIL_BUTTON_LINE = "The button in the email also signs you in on this computer.";

/** The email page under its title: the sentence, the address, what is wrong with it, "Continue with email" (its spinner while it sends). */
export function EmailCardBody(props: { prefix: string; centred?: boolean }) {
  const said = () => emailSaid(signInFlow.state());
  const sending = () => signInFlow.state().kind === "sending";
  return (
    <View class="w-full gap-4">
      <Text variant="meta" tone="fg2" class={props.centred ? "text-center" : ""} testID={`${props.prefix}-email-sentence`}>
        {EMAIL_SENTENCE}
      </Text>
      <View class="w-full gap-2">
        <TextField
          testID={`${props.prefix}-email`}
          label="Email"
          placeholder="you@example.com"
          keyboardType="email-address"
          autoComplete="email"
          autoCapitalize="none"
          returnKeyType="go"
          value={signInFlow.emailText()}
          onChange={signInFlow.setEmail}
          onSubmit={() => void signInFlow.sendCode()}
          error={said()?.danger === true}
        />
        <If when={said()}>
          <Text variant="meta" tone={said()!.danger ? "danger" : "fg2"} testID={`${props.prefix}-email-said`}>
            {deviceCopy(said()!.text)}
          </Text>
        </If>
      </View>
      <FlowButton testID={`${props.prefix}-continue`} label={EMAIL_BUTTON} primary block busy={sending()} onPress={() => void signInFlow.sendCode()} />
    </View>
  );
}

/** The code page under its title: where the code went, the boxes, what they said ("Checking…" while the request runs), Resend, Change email. */
export function CodeCardBody(props: { prefix: string; centred?: boolean }) {
  const page = () => signInFlow.state() as Extract<SignInState, { kind: "code" }>;
  const said = () => codeSaid(page(), signInFlow.now());
  const locked = () => signInFlow.now() < page().request.resendAt;
  return (
    <View class="w-full gap-4">
      <Text variant="meta" tone="fg2" class={props.centred ? "text-center" : ""} testID={`${props.prefix}-code-sentence`}>
        {`We sent a ${page().request.length}-digit code to ${page().request.email}.`}
      </Text>
      <If when={!page().tooMany}>
        <View class={props.centred ? "items-center" : ""}>
          <PinCells
            testID={`${props.prefix}-code-boxes`}
            label="Sign-in code"
            length={page().request.length}
            value={signInFlow.codeText()}
            onChange={signInFlow.typeCode}
            onComplete={(digits) => void signInFlow.checkCode(digits)}
            error={said()?.danger === true}
            disabled={page().checking}
          />
        </View>
      </If>
      <If when={page().checking}>
        <View class={`flex-row items-center gap-2 ${props.centred ? "justify-center" : ""}`} testID={`${props.prefix}-checking`}>
          <Spinner size={14} tone="fg2" label="Checking the code" />
          <Text variant="meta" tone="fg2">Checking…</Text>
        </View>
      </If>
      <If when={!page().checking && said()}>
        <Text variant="meta" tone={said()!.danger ? "danger" : "fg2"} class={props.centred ? "text-center" : ""} testID={`${props.prefix}-code-said`}>
          {deviceCopy(said()!.text)}
        </Text>
      </If>
      <If
        when={!page().tooMany}
        else={
          <FlowButton testID={`${props.prefix}-new-code`} label="Send a new code" primary block busy={page().resending} onPress={() => void signInFlow.newCode()} />
        }
      >
        <View class={TOUCH ? "w-full gap-2" : "w-full flex-row flex-wrap justify-end gap-2"}>
          <If when={!TOUCH}>
            <FlowButton testID={`${props.prefix}-change-email`} label="Change email" onPress={signInFlow.changeEmail} />
          </If>
          <AppButton
            testID={`${props.prefix}-resend`}
            label={resendLabel(page().request, signInFlow.now())}
            variant={locked() ? "secondary" : "primary"}
            block={TOUCH}
            disabled={locked()}
            leading={page().resending ? <Spinner size={14} tone="primaryFg" label="Sending a new code" /> : undefined}
            onPress={() => void signInFlow.resendCode()}
          />
          <If when={TOUCH}>
            <FlowButton testID={`${props.prefix}-change-email`} label="Change email" onPress={signInFlow.changeEmail} />
          </If>
        </View>
        <Text variant="meta" tone="fg3" class={props.centred ? "text-center" : ""} testID={`${props.prefix}-mail-line`}>
          {deviceCopy(MAIL_BUTTON_LINE)}
        </Text>
      </If>
    </View>
  );
}

/** A button that shows a spinner while its own request runs (never disabled for the wait on the person). */
export function FlowButton(props: { testID: string; label: string; primary?: boolean; busy?: boolean; block?: boolean; onPress: () => void }) {
  return (
    <AppButton
      testID={props.testID}
      label={props.label}
      variant={props.primary ? "primary" : "secondary"}
      block={props.block || TOUCH}
      leading={props.busy ? <Spinner size={14} tone={props.primary ? "primaryFg" : "fg2"} label={`${props.label}…`} /> : undefined}
      onPress={props.onPress}
    />
  );
}
