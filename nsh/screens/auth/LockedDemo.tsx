// A test fixture, not a screen: the sign-in/setup screen alone (LockedScreen, no frame around it) over a database whose access
// mode is already set and has no taxpayer yet, while first launch's "Just me" is half done (app.setup().pending === "solo").
// The two Test buttons take the screen away and mount it again (what a refresh in the middle of the setup does to it).
import { host, mount, signal, Text, View } from "nsh";
import { Button } from "nsh/base";
import "../../theme/fonts";
import { bir } from "../../bir";
import { app, store } from "../../shell";
import { LockedScreen } from "./Locked";

interface Params {
  /** The access mode is set before "Just me" was half done (default true): the help desk's, the one a mode can be set in with no taxpayer. */
  modeSet?: boolean;
}

const shown = signal(true);

function Demo() {
  const p = (host.params || {}) as Params;
  if (p.modeSet !== false) bir.setup_helpdesk("1234");
  store.bump();
  app.setSetup("solo", "choose");
  return (
    <View class="flex-1 bg-background">
      <View class="flex-1">
        <If when={shown()}>
          <LockedScreen />
        </If>
      </View>
      <View class="absolute top-0 left-0 flex-row gap-2">
        <Button testID="fx-hide" onPress={() => shown.set(false)}>
          <Text>Test: hide</Text>
        </Button>
        <Button testID="fx-show" onPress={() => shown.set(true)}>
          <Text>Test: show</Text>
        </Button>
        <Text testID="fx-mode">{`mode: ${store.authState().mode ?? "none"} | pending: ${app.setup().pending ?? "none"} | taxpayers: ${store.taxpayers().length}`}</Text>
      </View>
    </View>
  );
}

mount(Demo);
