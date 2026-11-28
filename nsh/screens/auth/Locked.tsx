// What stands in front of the frame (old main.js paint): the Buwiz account gate while the process is locked; first launch
// while the database has no access mode (and the help desk's recovery key until it was written down); the sign-in screen
// while appLocked. Shell.tsx's LockedFrame draws it under the bare title bar.
import { View } from "nsh";
import { app } from "../../shell";
import { AccessError } from "./AccessError";
import { AccountGate } from "./Gate";
import { Setup, SoloTaxpayer } from "./Setup";
import { SignIn } from "./SignIn";
import { WelcomeNote } from "./welcome";

/** True while first launch (or its recovery key, or the PIN page returned to from it) is in front. */
const setupShows = () =>
  app.needsSetup() || app.setup().step === "helpdesk-key" || app.setup().pending === "helpdesk";

/**
 * "Just me" is half done: first launch chose it and the first taxpayer is not typed yet. It stands in front whatever the
 * access mode says (a refresh or a restart in the middle can leave the mode set already), but never over the sign-in
 * screen: an app locked by an admin code asks for the code first.
 */
const soloPending = () => app.setup().pending === "solo" && (setupShows() || !app.appLocked());

export function LockedScreen() {
  return (
    <View testID="locked-screen" class="min-h-0 flex-1">
      {/* After the first sign-in, when a page before the app comes first (first-run setup, the PIN sign-in). */}
      <WelcomeNote centred when={() => !app.cloudLocked() && !app.chromeVisible()} />
      <If when={app.cloudLocked()}>
        <AccountGate />
      </If>
      <If when={!app.cloudLocked() && app.accessError() !== null}>
        <AccessError />
      </If>
      <If when={!app.cloudLocked() && app.accessError() === null && soloPending()}>
        <SoloTaxpayer />
      </If>
      <If when={!app.cloudLocked() && app.accessError() === null && setupShows() && app.setup().pending !== "solo"}>
        <Setup />
      </If>
      <If when={!app.cloudLocked() && app.accessError() === null && !setupShows() && app.appLocked()}>
        <SignIn />
      </If>
    </View>
  );
}
