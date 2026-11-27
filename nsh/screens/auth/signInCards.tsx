// The three pages of the Buwiz sign-in (signInFlow.ts), drawn on the account gate as open pages (parts.tsx OpenPage, the owner's
// look B): the first page (the logo, one line, "Continue with email"), the email page (the address and Continue) and the code
// page (six boxes that check by themselves, one "Send a new code" line, the back control that is Change email).
//
// Nothing moves when a sentence comes or goes: the line under the field and the line under the boxes keep one line's room in
// every state (a sentence that wraps grows the page downward). Sizes: signInLook.ts.
import { type InputRef, Text as NText, View, windowSize } from "nsh";
import { Button } from "nsh/base";
import { app } from "../../shell";
import { deviceCopy, PinCells, Spinner, T_MIN_H, Text, TextField, TOUCH } from "../../ui";
import { BackControl, OpenPage, openTight, PageButton, PageHeading, SoftLine } from "./parts";
import { dismissWaitingNotice, returnsWaitingNotice } from "../settings/cloud";
import { codeSaid, emailSaid, resendLabel, type Said, signInFlow, type SignInState } from "./signInFlow";
import { codeBoxWidth, OPEN_SIDE, openColumnMax, SAID_SLOT } from "./signInLook";

/** The first page's line under the logo and its one button. */
export const TAGLINE = "Prepare, file and print your BIR returns.";
export const START_BUTTON = "Continue with email";
/** The email page's title, its sentence and its button. */
export const EMAIL_TITLE = "Enter your email";
export const EMAIL_SENTENCE = "Enter your email and we will send you a 6-digit code to sign in, or to create your Buwiz account if you are new.";
export const EMAIL_BUTTON = "Continue";
/** The code page's title. */
export const CODE_TITLE = "Check your email";

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
 * The first page: the logo, the line under it (and why the gate could not open, if it could not), and "Continue with email"
 * close under them, all one centred block (the owner's pick for the phone; layout C gives every size the same).
 */
export function StartPage(props: { prefix: string }) {
  return (
    <OpenPage
      testID={`${props.prefix}-start`}
      gap="gap-0"
      footer={<PageButton testID={`${props.prefix}-start-continue`} label={START_BUTTON} icon="mail" onPress={() => { dismissWaitingNotice(); signInFlow.openEmail(); }} />}
    >
      <SoftLine testID={`${props.prefix}-tagline`}>{TAGLINE}</SoftLine>
      {/* §A.5: after a sign-out that left returns waiting, the notice shows once, with no names. */}
      <If when={returnsWaitingNotice() > 0}>
        {(() => {
          return (
            <View class="mt-2">
              <Text variant="meta" tone="fg2" testID="returns-waiting-notice" class="text-center">
                {"Returns are waiting to send on this device. They send when their account signs in again."}
              </Text>
            </View>
          );
        })()}
      </If>
      <If when={app.gateError()}>
        <View class="mt-3">
          <SaidLine said={{ text: app.gateError() ?? "", danger: true }} testID={`${props.prefix}-start-said`} />
        </View>
      </If>
    </OpenPage>
  );
}

/**
 * The email page: back to the first page, the title, the sentence, the address, what is wrong with it (or why the gate could not
 * open), Continue (its spinner while it sends).
 */
export function EmailPage(props: { prefix: string }) {
  const said = (): Said | null => {
    const own = emailSaid(signInFlow.state());
    if (own) return own;
    const gate = app.gateError();
    return gate ? { text: gate, danger: true } : null;
  };
  const sending = () => signInFlow.state().kind === "sending";
  return (
    <OpenPage
      testID={`${props.prefix}-email-page`}
      gap="gap-3"
      back={<BackControl testID={`${props.prefix}-back`} label="Back" onPress={() => void signInFlow.back()} />}
    >
      <PageHeading testID={`${props.prefix}-title`}>{EMAIL_TITLE}</PageHeading>
      <SoftLine testID={`${props.prefix}-email-sentence`}>{EMAIL_SENTENCE}</SoftLine>
      <View class={`${openTight() ? "mt-1" : "mt-2"} w-full gap-2`}>
        <TextField
          testID={`${props.prefix}-email`}
          label="Email"
          large
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
      <PageButton testID={`${props.prefix}-continue`} label={EMAIL_BUTTON} busy={sending()} onPress={() => void signInFlow.sendCode()} />
    </OpenPage>
  );
}

/**
 * The code page: the back control (Change email: the email page with the address kept), the title, where the code went, the
 * boxes (spent and dimmed after too many wrong tries), the line under them ("Checking…" while the check runs, or what the check
 * said), then the one "Send a new code" line: plain text with its countdown while locked, a link once a new code can be sent.
 */
export function CodePage(props: { prefix: string }) {
  const page = () => signInFlow.state() as Extract<SignInState, { kind: "code" }>;
  const said = () => codeSaid(page(), signInFlow.now());
  const locked = () => !page().tooMany && signInFlow.now() < page().request.resendAt;
  const column = () => Math.min(openColumnMax(TOUCH), windowSize().width - 2 * OPEN_SIDE);
  // The boxes' focus, kept from their mount for "Send a new code", which opens them again on the same page.
  let focusBoxes = (): void => undefined;
  const takeFocus = (focus: () => void): void => {
    focusBoxes = focus;
    focusWhenOpen(focus);
  };
  return (
    <OpenPage
      testID={`${props.prefix}-code-page`}
      gap="gap-3"
      back={<BackControl testID={`${props.prefix}-change-email`} label="Change email" onPress={signInFlow.changeEmail} />}
    >
      <PageHeading testID={`${props.prefix}-code-title`}>{CODE_TITLE}</PageHeading>
      <View class="w-full items-center" testID={`${props.prefix}-code-sentence`}>
        <SoftLine>{`We sent a ${page().request.length}-digit code to`}</SoftLine>
        <NText testID={`${props.prefix}-code-address`} class="text-center font-semibold text-foreground" style={{ fontSize: 14 }}>
          {page().request.email}
        </NText>
      </View>
      <View class="mt-1 w-full items-center gap-2">
        <PinCells
          testID={`${props.prefix}-code-boxes`}
          label="Sign-in code"
          length={page().request.length}
          large={{ width: codeBoxWidth(column(), page().request.length) }}
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
      <Button
        testID={`${props.prefix}-resend`}
        accessibilityLabel={resendLabel(page().request, signInFlow.now(), page().tooMany)}
        disabled={locked()}
        onPress={() => void (page().tooMany ? signInFlow.newCode().then(() => focusWhenOpen(focusBoxes)) : signInFlow.resendCode())}
        class={`flex-row items-center justify-center gap-2 self-center rounded-(--radius-ctl) px-2 enabled:hover:opacity-80 focus-visible:ring-2 focus-visible:ring-accent-blue ${TOUCH ? T_MIN_H : "min-h-8"}`}
      >
        <If when={page().resending}>
          <Spinner size={14} tone="fg2" label="Sending a new code" />
        </If>
        <NText class={locked() ? "text-muted-foreground" : "font-medium text-accent-blue"} style={{ fontSize: 14 }}>
          {resendLabel(page().request, signInFlow.now(), page().tooMany)}
        </NText>
      </Button>
    </OpenPage>
  );
}
