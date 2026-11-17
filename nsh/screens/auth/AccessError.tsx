// What stands in front of the frame when the host could not say whether the admin has a PIN or an authenticator (boot, or Lock app):
// the app stays locked, and this says why with the host's own sentence and a Retry. It never offers sign-in, first-run setup or
// "Open without signing in": none of them may rest on an answer nobody gave.
import { app } from "../../shell";
import { AppButton, Text } from "../../ui";
import { AuthCard, Heading } from "./parts";

export function AccessError() {
  return (
    <AuthCard testID="access-error-card">
      <Heading icon="lock" title="eBIRForms is locked" line="This computer's sign-in could not be checked, so nothing opens." testID="access-error-title" />
      <Text variant="meta" tone="danger" testID="access-error-detail">{app.accessError() ?? ""}</Text>
      <AppButton testID="access-error-retry" label="Try again" variant="primary" block onPress={() => app.retryAccess()} />
    </AuthCard>
  );
}
