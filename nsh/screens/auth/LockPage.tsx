// The owner-PIN lock page as a component. Every decision and action is the injected LockInterface; nothing here is
// the app's real lock.
import { cleanup, signal, View } from "nsh";
import { PinCells, Text } from "../../ui";
import { ErrorLine, Labelled, OpenPage, PageButton, PageHeading, SoftLine, TextLink } from "./parts";
import { maskAccount, type LockInterface } from "./lockInterface";

const BIOMETRIC_LABEL = {
  face_id: "Unlock with Face ID",
  touch_id: "Unlock with Touch ID",
  windows_hello: "Unlock with Windows Hello",
} as const;

function clock(secs: number): string {
  return `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, "0")}`;
}

export function LockPage(props: { face: LockInterface }) {
  const state = signal(props.face.lockState());
  const pin = signal("");
  const wrong = signal(false);
  const reread = () => state.set(props.face.lockState());
  const timer = setInterval(() => {
    if (state().wait_secs > 0) reread();
  }, 1000);
  cleanup(() => {
    clearInterval(timer);
    pin.set("");
  });

  const verify = (typed: string) => {
    void Promise.resolve(props.face.verify(typed)).then((result) => {
      pin.set("");
      if (result.kind === "wrong") {
        wrong.set(true);
      } else if (result.kind === "wait") {
        wrong.set(false);
        state.set({ ...state(), wait_secs: result.secs });
      } else if (result.kind === "lost") {
        state.set({ ...state(), store_lost: true, wait_secs: 0 });
      } else {
        wrong.set(false);
      }
      reread();
    });
  };
  const biometric = () => state().biometric;
  return (
    <OpenPage testID="owner-lock-page">
      <PageHeading testID="owner-lock-heading">Unlock eBIRForms</PageHeading>
      <SoftLine testID="owner-lock-account">{maskAccount(state().account_masked)}</SoftLine>
      <If when={state().store_lost} else={
        <>
          <If when={state().wait_secs > 0} else={
            <Labelled label="PIN">
              <PinCells testID="owner-lock-pin" label="PIN" length={4} masked value={pin()} onChange={(v) => { pin.set(v); wrong.set(false); }} onComplete={verify} error={wrong()} />
            </Labelled>
          }>
            <Text variant="emph" tone="danger" class="text-center" testID="owner-lock-wait">Try again in {clock(state().wait_secs)}</Text>
          </If>
          <If when={wrong()}>
            <ErrorLine message="Wrong PIN." testID="owner-lock-error" />
          </If>
          <If when={biometric() !== "none"}>
            <PageButton testID="lock-biometric" label={BIOMETRIC_LABEL[biometric() as keyof typeof BIOMETRIC_LABEL]} icon="lock" onPress={props.face.biometric} />
          </If>
        </>
      }>
        <Text variant="body" tone="danger" class="text-center" testID="owner-lock-lost">This device's lock can't be read. Sign out and sign in again, or write to support@buwiz.com.</Text>
      </If>
      <View class="items-center gap-1">
        <TextLink testID="lock-forgot" label="Forgot PIN?" onPress={props.face.forgot} />
        <TextLink testID="lock-sign-out" label="Not you? Sign out" onPress={props.face.signOut} />
        <TextLink testID="lock-client-sign-in" label="Client sign-in" onPress={props.face.clientSignIn} />
      </View>
    </OpenPage>
  );
}
