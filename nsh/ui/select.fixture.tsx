// The entry the selection tests open (fields.select.test.tsx): one of each number box with a readout of what it reports, a plain box
// to Tab from, and buttons that put the keyboard in a box the way a form page's pill step does (the box's own focus()). The
// amounts start at zero or at 1,250.00 so a test can tell a click that replaced the text from one that kept its caret. Tests only.
import type { InputRef } from "nsh";
import { mount, signal, View } from "nsh";
import { AppButton } from "./buttons";
import { IntInput, MoneyInput, PercentInput, TextField, TinInput } from "./fields";
import { Text } from "./text";

function App() {
  const free = signal("");
  const some = signal("1250");
  const zero = signal("0");
  const rate = signal("0");
  const count = signal("0");
  const tin = signal("000123453");
  const kept = signal("1250");
  let someRef: InputRef | undefined;
  let zeroRef: InputRef | undefined;
  return (
    <View class="gap-2 p-3">
      <View class="flex-row flex-wrap gap-x-3">
        <Text variant="cap">{`Some: ${some() || "empty"}`}</Text>
        <Text variant="cap">{`Zero: ${zero() || "empty"}`}</Text>
        <Text variant="cap">{`Rate: ${rate() || "empty"}`}</Text>
        <Text variant="cap">{`Count: ${count() || "empty"}`}</Text>
        <Text variant="cap">{`Tin: ${tin() || "empty"}`}</Text>
        <Text variant="cap">{`Kept: ${kept() || "empty"}`}</Text>
      </View>
      <View class="flex-row gap-2">
        <AppButton label="Focus some" size="sm" testID="s-go-some" onPress={() => someRef?.focus()} />
        <AppButton label="Focus zero" size="sm" testID="s-go-zero" onPress={() => zeroRef?.focus()} />
      </View>
      <TextField label="Free" value={free()} onChange={(v) => free.set(v)} testID="s-free" />
      <MoneyInput label="Some" value={some()} onChange={(v) => some.set(v)} inputRef={(r) => (someRef = r)} testID="s-some" />
      <MoneyInput label="Zero" value={zero()} onChange={(v) => zero.set(v)} inputRef={(r) => (zeroRef = r)} testID="s-zero" />
      <PercentInput label="Rate" value={rate()} onChange={(v) => rate.set(v)} testID="s-rate" />
      <IntInput label="Count" value={count()} onChange={(v) => count.set(v)} testID="s-count" />
      <TinInput label="TIN" value={tin()} onChange={(v) => tin.set(v)} testID="s-tin" />
      {/* A box whose page cannot hold a blank amount: it takes the empty report, then puts the old amount back 350 ms later
          unless something was typed (the form page does, because the host refuses a blank amount). */}
      <MoneyInput
        label="Kept"
        value={kept()}
        onChange={(v) => {
          kept.set(v);
          if (v === "") setTimeout(() => kept() === "" && kept.set("1250"), 350);
        }}
        testID="s-kept"
      />
    </View>
  );
}

mount(App);
