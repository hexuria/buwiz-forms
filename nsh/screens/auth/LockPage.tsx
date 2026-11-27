// The owner-PIN lock page as a component. Every decision and action is the injected LockInterface; nothing here is
// the app's real lock.
import { cleanup, effect, signal, View } from "nsh";
import { PinCells, Text } from "../../ui";
import { ErrorLine, Labelled, OpenPage, PageButton, PageHeading, SoftLine, TextLink } from "./parts";
import type { LockInterface } from "./lockInterface";

const BIOMETRIC_LABEL = {
  face_id: "Unlock with Face ID",
  touch_id: "Unlock with Touch ID",
  windows_hello: "Unlock with Windows Hello",
} as const;

function clock(secs: number): string {
  return `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, "0")}`;
}

export function LockPage(props: { face: LockInterface }) {
  const state = () => props.face.lockState();
  const wrong = signal<number | null>(null);
  const lost = signal(false);
  const wait = signal(0);
  const offeredWait = signal(0);
  effect(() => {
    const offered = state().wait_secs;
    if (offered > 0 && offered !== offeredWait()) {
      offeredWait.set(offered);
      wait.set(offered);
    }
    if (state().lost) lost.set(true);
  });
  const timer = setInterval(() => {
    if (wait() > 0) wait.set(wait() - 1);
  }, 1000);
  cleanup(() => clearInterval(timer));

  const verify = (pin: string) => {
    void Promise.resolve(props.face.verify(pin)).then((result) => {
      if (result.kind === "wrong") {
        wrong.set(result.tries_left);
      } else if (result.kind === "wait") {
        wrong.set(null);
        wait.set(result.secs);
      } else if (result.kind === "lost") {
        lost.set(true);
      }
    });
  };
  const biometric = () => state().biometric;
  return (
    <OpenPage testID="owner-lock-page">
      <PageHeading testID="owner-lock-heading">Unlock eBIRForms</PageHeading>
      <SoftLine testID="owner-lock-account">{state().account_masked}</SoftLine>
      <If when={lost()} else={
        <>
          <If when={wait() > 0} else={
            <Labelled label="PIN">
              <PinCells testID="owner-lock-pin" label="PIN" length={4} masked value="" onChange={() => undefined} onComplete={verify} error={wrong() !== null} />
            </Labelled>
          }>
            <Text variant="emph" tone="danger" class="text-center" testID="owner-lock-wait">Try again in {clock(wait())}</Text>
          </If>
          <If when={wrong() !== null}>
            <ErrorLine message={`Wrong PIN. ${wrong()} tries left.`} testID="owner-lock-error" />
          </If>
          <If when={biometric() !== "none"}>
            <PageButton testID="lock-biometric" label={BIOMETRIC_LABEL[biometric() as keyof typeof BIOMETRIC_LABEL]} icon="lock" onPress={props.face.biometric} />
          </If>
        </>
      }>
        <Text variant="body" tone="danger" class="text-center" testID="owner-lock-lost">This device lost its lock store. Contact support.</Text>
      </If>
      <View class="items-center gap-1">
        <TextLink testID="lock-forgot" label="Forgot PIN?" onPress={props.face.forgot} />
        <TextLink testID="lock-sign-out" label="Not you? Sign out" onPress={props.face.signOut} />
        <TextLink testID="lock-client-sign-in" label="Client sign-in" onPress={props.face.clientSignIn} />
      </View>
    </OpenPage>
  );
}
