// A test fixture, not a screen: the frame over the demo taxpayer (Juana Santos) with a count of how many times the form library is
// built (bir.screen_forms_index), for shell/paletteIndex.test.tsx. The count shows as "builds: N". Cmd+Shift+9 drops the store's
// cached reads as any save does (store.bump), so a test can open the palette again over a cold store.
// It sits at the nsh root because a real-window run serves assets (fonts, icons) from the entry's folder down.
import { keymap, mount, signal, Text, View } from "nsh";
import "./theme/fonts";
import { bir } from "./bir";
import { createDemoTaxpayer } from "./bir/testing";
import { Shell } from "./shell/Shell";
import { store } from "./shell/store";

let builds = 0;
const shown = signal(0);
const real = bir.screen_forms_index;
// The store calls bir[name] at each read. The count is shown a tick later: a signal is not written inside a read.
(bir as { screen_forms_index: typeof real }).screen_forms_index = () => {
  builds += 1;
  setTimeout(() => shown.set(builds), 0);
  return real();
};

function Demo() {
  createDemoTaxpayer();
  keymap.bind({ "mod+shift+9": () => store.bump() });
  return (
    <View class="flex-1">
      <Shell />
      <View class="absolute bottom-0 left-0">
        <Text testID="probe-builds">{`builds: ${shown()}`}</Text>
      </View>
    </View>
  );
}

mount(Demo);
