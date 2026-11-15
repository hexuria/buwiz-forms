// The entry the digit-cell tests open (cells.test.tsx): the TIN as nine cells in three groups, a branch code of five, and PIN cells
// the TIN hands its last digit to, each with a readout of what it reports. Tests only; nothing imports it.
import { mount, signal, View } from "nsh";
import "../theme/fonts";
import { DigitCells, PinCells } from "./fields";
import { Text } from "./text";

function App() {
  const tin = signal("");
  const code = signal("00001");
  const pin = signal("");
  const done = signal("none");
  const lastFocus = signal("none");
  let focusPin: (() => void) | undefined;
  return (
    <View class="gap-3 p-4">
      <Text testID="r-tin">{`Tin: ${tin() || "empty"}`}</Text>
      <Text testID="r-code">{`Code: ${code() || "empty"}`}</Text>
      <Text testID="r-pin">{`Pin: ${pin() || "empty"}`}</Text>
      <Text testID="r-done">{`Done: ${done()}`}</Text>
      <Text testID="r-focus">{`Focus: ${lastFocus()}`}</Text>
      <DigitCells
        testID="c-tin"
        label="TIN"
        length={9}
        groups={3}
        value={tin()}
        onChange={(v) => tin.set(v)}
        onComplete={(v) => {
          done.set(v);
          focusPin?.();
        }}
        onBlur={() => lastFocus.set("left tin")}
      />
      <DigitCells testID="c-code" label="Branch code" length={5} value={code()} onChange={(v) => code.set(v)} />
      <PinCells
        testID="c-pin"
        label="PIN"
        length={4}
        masked
        value={pin()}
        onChange={(v) => pin.set(v)}
        focusRef={(f) => (focusPin = f)}
        onFocus={() => lastFocus.set("pin")}
      />
    </View>
  );
}

mount(App);
