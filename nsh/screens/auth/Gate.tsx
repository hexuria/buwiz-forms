// The Buwiz account gate (the old shell's gate.js): the whole window until the sign-in approves this computer. Nothing
// that names a taxpayer is drawn here; the host refuses the data verbs on its own, this card is how a person gets past it.
// Each state of the sign-in flow (signInFlow.ts) is its own card: the start card (Sign in, Create account; an expiry or a
// failure said inline), or a waiting card (the code with Copy, Open the page again, Cancel).
import { View } from "nsh";
import { app } from "../../shell";
import { Text } from "../../ui";
import { GATE_ABOUT, GATE_DEFAULT } from "./logic";
import { AuthCard, deviceCopy } from "./parts";
import { FlowCardBody, StartButton } from "./signInCards";
import { flowCard, signInFlow, startMessage } from "./signInFlow";

export function AccountGate() {
  const card = () => flowCard(signInFlow.state());
  return (
    <View testID="gate-root" class="min-h-0 flex-1">
      <If when={card()} else={<StartCard />}>
        <AuthCard testID="gate-wait-card" shadow={false}>
          <Text variant="page" class="text-center" testID="gate-wait-title">{card()!.title}</Text>
          <FlowCardBody card={card()!} prefix="gate" centred />
        </AuthCard>
      </If>
    </View>
  );
}

/** Signed out: the welcome, what Buwiz is, Sign in and Create account. An expiry or a failure is said under the title. */
function StartCard() {
  const message = () => {
    const said = startMessage(signInFlow.state());
    if (said) return said;
    return app.gateError() ? { text: app.gateError() as string, danger: true } : null;
  };
  return (
    <AuthCard testID="gate-card" shadow={false}>
      <Text variant="page" class="text-center">Welcome to eBIRForms</Text>
      <Text variant="meta" tone={message()?.danger ? "danger" : "fg2"} class="text-center" testID="gate-detail">
        {deviceCopy(message()?.text ?? GATE_DEFAULT)}
      </Text>
      <Text variant="meta" tone="fg3" class="text-center" testID="gate-about">{deviceCopy(GATE_ABOUT)}</Text>
      <View class="w-full gap-2">
        <StartButton entry="sign-in" testID="gate-sign-in" label="Sign in" primary block />
        <StartButton entry="create" testID="gate-create-account" label="Create account" block />
      </View>
    </AuthCard>
  );
}
