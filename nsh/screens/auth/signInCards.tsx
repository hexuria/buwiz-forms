// The pieces of the sign-in flow's two pages (signInFlow.ts), drawn on the account gate: the email page (the address and
// "Continue with email") and the code page (six boxes that check by themselves, one "get a new code" control, Change email).
//
// Nothing moves when a sentence comes or goes: the line under the field and the line under the boxes keep one line's room in
// every state (a sentence that wraps grows the card downward only on a phone, where the card is anchored at its top). Sizes are
// the kit's (nsh/ui look.ts CTL, fields.tsx TextField and PinCells): 32 points with a pointer, 44 on touch (the boxes 48).
import { type InputRef, View } from "nsh";
import { app } from "../../shell";
import { AppButton, deviceCopy, PinCells, Spinner, Text, TextField, TOUCH } from "../../ui";
import { codeSaid, emailSaid, resendLabel, type Said, signInFlow, type SignInState } from "./signInFlow";
import { codeActionsClass, SAID_SLOT } from "./signInLook";

/** The email page's title, its sentence and its one button. */
export const EMAIL_TITLE = "Sign in to eBIRForms";
export const EMAIL_SENTENCE = "Enter your email and we will send you a 6-digit code to sign in, or to create your Buwiz account if you are new.";
export const EMAIL_BUTTON = "Continue with email";
/** The code page's title. */
export const CODE_TITLE = "Enter the code";

/** The sentence in a reserved slot: the state's, else nothing (the slot keeps its room). */
function SaidLine(props: { said: Said | null; testID: string }) {
  return (
    <If when={props.said}>
      <Text variant="meta" tone={props.said!.danger ? "danger" : "fg2"} class="text-center" testID={props.testID}>
        {deviceCopy(props.said!.text)}
      </Text>
    </If>
  );
}

/**
 * Back from Change email: the field takes the focus with the whole address selected, so typing replaces it instead of adding to
 * its end (a tap places the caret). After the mount, once the box is there to take the focus.
 */
function selectAfterChange(input: InputRef): void {
  if (!signInFlow.takeReselect()) return;
  setTimeout(() => {
    input.focus();
    input.setSelection(0, signInFlow.emailText().length);
  }, 0);
}

/**
 * The boxes take the focus, so the code is typed with no tap first: when the code page opens (the email field's letter keyboard
 * gives way to the number pad), and when "Send a new code" opens boxes that were spent (a page that opened spent, back from
 * Change email, gave them no focus to keep). After the mount, once the boxes are there to take it; not while they are locked
 * (a check running, too many wrong tries), and not once the page has gone.
 */
function focusWhenOpen(focus: () => void): void {
  setTimeout(() => {
    const now = signInFlow.state();
    if (now.kind === "code" && !now.checking && !now.tooMany) focus();
  }, 0);
}

/**
 * The email page under its title: the sentence, the address, what is wrong with it (or why the gate could not open), "Continue
 * with email" (its spinner while it sends).
 */
export function EmailCardBody(props: { prefix: string }) {
  const said = (): Said | null => {
    const own = emailSaid(signInFlow.state());
    if (own) return own;
    const gate = app.gateError();
    return gate ? { text: gate, danger: true } : null;
  };
  const sending = () => signInFlow.state().kind === "sending";
  return (
    <View class="w-full gap-4">
      <Text variant="meta" tone="fg2" class="text-center" testID={`${props.prefix}-email-sentence`}>
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
          inputRef={selectAfterChange}
          onSubmit={() => void signInFlow.sendCode()}
          error={said()?.danger === true}
        />
        <View class={SAID_SLOT} testID={`${props.prefix}-email-slot`}>
          <SaidLine said={said()} testID={`${props.prefix}-email-said`} />
        </View>
      </View>
      <FlowButton testID={`${props.prefix}-continue`} label={EMAIL_BUTTON} primary block busy={sending()} onPress={() => void signInFlow.sendCode()} />
    </View>
  );
}

/**
 * The code page under its title: where the code went, the boxes (spent and dimmed after too many wrong tries), the line under
 * them ("Checking…" while the check runs, or what the check said), then the one "get a new code" control and Change email.
 */
export function CodeCardBody(props: { prefix: string }) {
  const page = () => signInFlow.state() as Extract<SignInState, { kind: "code" }>;
  const said = () => codeSaid(page(), signInFlow.now());
  const locked = () => !page().tooMany && signInFlow.now() < page().request.resendAt;
  // The boxes' focus, kept from their mount for "Send a new code", which opens them again on the same page.
  let focusBoxes = (): void => undefined;
  const takeFocus = (focus: () => void): void => {
    focusBoxes = focus;
    focusWhenOpen(focus);
  };
  return (
    <View class="w-full gap-4">
      <Text variant="meta" tone="fg2" class="text-center" testID={`${props.prefix}-code-sentence`}>
        {`We sent a ${page().request.length}-digit code to ${page().request.email}.`}
      </Text>
      <View class="w-full items-center gap-2">
        <PinCells
          testID={`${props.prefix}-code-boxes`}
          label="Sign-in code"
          length={page().request.length}
          value={signInFlow.codeText()}
          onChange={signInFlow.typeCode}
          onComplete={(digits) => void signInFlow.checkCode(digits)}
          error={said()?.danger === true}
          disabled={page().checking || page().tooMany}
          focusRef={takeFocus}
        />
        <View class={`${SAID_SLOT} items-center`} testID={`${props.prefix}-code-slot`}>
          <If when={page().checking} else={<SaidLine said={said()} testID={`${props.prefix}-code-said`} />}>
            <View class="flex-row items-center justify-center gap-2" testID={`${props.prefix}-checking`}>
              <Spinner size={14} tone="fg2" label="Checking the code" />
              <Text variant="meta" tone="fg2">Checking…</Text>
            </View>
          </If>
        </View>
      </View>
      <View class={codeActionsClass(TOUCH)} testID={`${props.prefix}-code-actions`}>
        <AppButton
          testID={`${props.prefix}-resend`}
          label={resendLabel(page().request, signInFlow.now(), page().tooMany)}
          block={TOUCH}
          disabled={locked()}
          leading={page().resending ? <Spinner size={14} tone="fg2" label="Sending a new code" /> : undefined}
          onPress={() => void (page().tooMany ? signInFlow.newCode().then(() => focusWhenOpen(focusBoxes)) : signInFlow.resendCode())}
        />
        <FlowButton testID={`${props.prefix}-change-email`} label="Change email" onPress={signInFlow.changeEmail} />
      </View>
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
