// The Buwiz account gate (the old shell's gate.js): the whole window until this device is signed in. Nothing that names a
// taxpayer is drawn here; the host refuses the data verbs on its own, this card is how a person gets past it. It opens on the
// email page (one page for signing in and creating an account), then the code page (signInFlow.ts), then Home (or, on a first
// run, the first-run setup). It is the one place to sign in: Settings > Account's Sign in comes here.
import { effect, untracked, View } from "nsh";
import { Text } from "../../ui";
import { AuthCard } from "./parts";
import { CODE_TITLE, CodeCardBody, EMAIL_TITLE, EmailCardBody } from "./signInCards";
import { pageOf, signInFlow } from "./signInFlow";

export function AccountGate() {
  // Made anew, or back at its start: the request the host keeps opens the code page.
  effect(() => {
    if (signInFlow.state().kind === "idle") untracked(() => signInFlow.restore());
  });
  const page = () => pageOf(signInFlow.state());
  return (
    <View testID="gate-root" class="min-h-0 flex-1">
      <If
        when={page() === "code"}
        else={
          <AuthCard testID="gate-card" shadow={false}>
            <Text variant="page" class="text-center" testID="gate-title">{EMAIL_TITLE}</Text>
            <EmailCardBody prefix="gate" />
          </AuthCard>
        }
      >
        <AuthCard testID="gate-code-card" shadow={false}>
          <Text variant="page" class="text-center" testID="gate-code-title">{CODE_TITLE}</Text>
          <CodeCardBody prefix="gate" />
        </AuthCard>
      </If>
    </View>
  );
}
