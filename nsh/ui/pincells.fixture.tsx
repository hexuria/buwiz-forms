// The entry the PinCells tests open (pincells.test.tsx): a sign-in code of six cells, a masked PIN of four and a disabled one,
// each with a readout of what it reports. Tests only; nothing imports it.
import { mount, signal, View } from "nsh";
import "../theme/fonts";
import { PinCells } from "./fields";
import { Text } from "./text";

function App() {
  const code = signal("");
  const pin = signal("");
  const off = signal("");
  const done = signal("none");
  const focused = signal("none");
  return (
    <View class="gap-3 p-4">
      <Text testID="r-code">{`Code: ${code() || "empty"}`}</Text>
      <Text testID="r-pin">{`Pin: ${pin() || "empty"}`}</Text>
      <Text testID="r-off">{`Off: ${off() || "empty"}`}</Text>
      <Text testID="r-done">{`Done: ${done()}`}</Text>
      <Text testID="r-focus">{`Focus: ${focused()}`}</Text>
      <PinCells
        testID="p"
        label="Sign-in code"
        length={6}
        value={code()}
        onChange={(v) => code.set(v)}
        onComplete={(v) => done.set(v)}
        onFocus={() => focused.set("code")}
      />
      <PinCells testID="p-mask" label="PIN" masked length={4} value={pin()} onChange={(v) => pin.set(v)} />
      <PinCells testID="p-off" label="PIN" length={4} disabled value={off()} onChange={(v) => off.set(v)} onFocus={() => focused.set("off")} />
    </View>
  );
}

mount(App);
