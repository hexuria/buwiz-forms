// The Buwiz account gate (the old shell's gate.js): the whole window until the device flow approves this computer. Nothing
// that names a taxpayer is drawn here; the host refuses the data verbs on its own, this card is how a person gets past it.
import { View } from "nsh";
import { app } from "../../shell";
import { AppButton, Text } from "../../ui";
import { GATE_ABOUT, GATE_DEFAULT } from "./logic";
import { AuthCard, deviceCopy } from "./parts";
import { auth } from "./state";

export function AccountGate() {
  const detail = () => deviceCopy(app.gateError() || auth.gateDetail() || GATE_DEFAULT);
  return (
    <View testID="gate-root" class="min-h-0 flex-1">
      <AuthCard testID="gate-card" shadow={false}>
        <Text variant="page" class="text-center">Welcome to eBIRForms</Text>
        <Text variant="meta" tone={app.gateError() ? "danger" : "fg2"} class="text-center" testID="gate-detail">{detail()}</Text>
        <Text variant="meta" tone="fg3" class="text-center" testID="gate-about">{deviceCopy(GATE_ABOUT)}</Text>
        <AppButton
          testID="gate-sign-in"
          label={auth.gateBusy() ? "Waiting…" : "Sign in"}
          variant="primary"
          block
          disabled={auth.gateBusy()}
          onPress={() => void auth.startAccountFlow("sign-in")}
        />
        <AppButton
          testID="gate-create-account"
          label={auth.gateBusy() ? "Waiting…" : "Create account"}
          block
          disabled={auth.gateBusy()}
          onPress={() => void auth.startAccountFlow("create")}
        />
      </AuthCard>
    </View>
  );
}
