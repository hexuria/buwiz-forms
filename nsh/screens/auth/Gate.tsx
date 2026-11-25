// The Buwiz account gate (the old shell's gate.js): the whole window until this device is signed in. Nothing that names a
// taxpayer is drawn here; the host refuses the data verbs on its own, this card is how a person gets past it. It opens on the
// email page (one page for signing in and creating an account), then the code page (signInFlow.ts), then Home.
import { View } from "nsh";
import { app } from "../../shell";
import { Text } from "../../ui";
import { AuthCard, deviceCopy } from "./parts";
import { CODE_TITLE, CodeCardBody, EMAIL_TITLE, EmailCardBody } from "./signInCards";
import { emailSaid, pageOf, signInFlow } from "./signInFlow";

export function AccountGate() {
  signInFlow.restore();
  const page = () => pageOf(signInFlow.state());
  return (
    <View testID="gate-root" class="min-h-0 flex-1">
      <If
        when={page() === "code"}
        else={
          <AuthCard testID="gate-card" shadow={false}>
            <Text variant="page" class="text-center" testID="gate-title">{EMAIL_TITLE}</Text>
            <If when={app.gateError() && !emailSaid(signInFlow.state())}>
              <Text variant="meta" tone="danger" class="text-center" testID="gate-detail">{deviceCopy(app.gateError() as string)}</Text>
            </If>
            <EmailCardBody prefix="gate" centred />
          </AuthCard>
        }
      >
        <AuthCard testID="gate-code-card" shadow={false}>
          <Text variant="page" class="text-center" testID="gate-code-title">{CODE_TITLE}</Text>
          <CodeCardBody prefix="gate" centred />
        </AuthCard>
      </If>
    </View>
  );
}
