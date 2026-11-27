// The Buwiz account gate (the old shell's gate.js): the whole window until this device is signed in. Nothing that names a
// taxpayer is drawn here; the host refuses the data verbs on its own, this page is how a person gets past it. It opens on the
// first page (the logo and "Continue with email"), then the email page (one page for signing in and creating an account),
// then the code page (signInFlow.ts), then Home (or, on a first run, the first-run setup). It is the one place to sign in:
// Settings > Account's Sign in comes here. Android's back button steps back a page, as the pages' back control does.
import { effect, platform, untracked, View } from "nsh";
import { BackHandler, isAvailable as mobileAvailable } from "nsh/mobile";
import { CodePage, EmailPage, StartPage } from "./signInCards";
import { pageOf, signInFlow } from "./signInFlow";
import "./welcome";

export function AccountGate() {
  // Made anew, or back at its start: the request the host keeps opens the code page.
  effect(() => {
    if (signInFlow.state().kind === "idle") untracked(() => signInFlow.restore());
  });
  const page = () => pageOf(signInFlow.state(), signInFlow.emailOpen());
  // The newest listener hears the press first (the frame's, Shell.tsx, is older). It is made in an effect and only while the
  // email or code page shows (nsh navigation.md: a listener made in an effect lasts until the effect runs again or its
  // component goes), so the first page, and every page after sign-in, keep the press as everywhere else.
  if (mobileAvailable && platform.os === "android") {
    effect(() => {
      if (page() !== "start") BackHandler.on("press", () => signInFlow.back());
    });
  }
  return (
    <View testID="gate-root" class="min-h-0 flex-1">
      <If when={page() === "start"}>
        <StartPage prefix="gate" />
      </If>
      <If when={page() === "email"}>
        <EmailPage prefix="gate" />
      </If>
      <If when={page() === "code"}>
        <CodePage prefix="gate" />
      </If>
    </View>
  );
}
