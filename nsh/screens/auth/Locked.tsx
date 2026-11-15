// What stands in front of the frame (old main.js paint): the Buwiz account gate while the process is locked; first launch
// while the database has no access mode (and the help desk's recovery key until it was written down); the sign-in screen
// while appLocked. Shell.tsx's LockedFrame draws it under the bare title bar.
import { View } from "nsh";
import { app } from "../../shell";
import { AccountGate } from "./Gate";
import { Setup, SoloTaxpayer } from "./Setup";
import { SignIn } from "./SignIn";

/** True while first launch (or its recovery key) is in front. */
const setupShows = () => app.needsSetup() || app.setup().step === "helpdesk-key";

export function LockedScreen() {
  return (
    <View testID="locked-screen" class="min-h-0 flex-1">
      <If when={app.cloudLocked()}>
        <AccountGate />
      </If>
      <If when={!app.cloudLocked() && setupShows() && app.setup().pending === "solo"}>
        <SoloTaxpayer />
      </If>
      <If when={!app.cloudLocked() && setupShows() && app.setup().pending !== "solo"}>
        <Setup />
      </If>
      <If when={!app.cloudLocked() && !setupShows() && app.appLocked()}>
        <SignIn />
      </If>
    </View>
  );
}
