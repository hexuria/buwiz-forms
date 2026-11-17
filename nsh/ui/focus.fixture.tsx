// The entry the focus-handle tests open (focus.test.tsx): each box and choice of the kit that gives its ref (inputRef, focusRef),
// and a button beside it that puts the keyboard there the way a form page's pill step does. Readouts first. Tests only.
import type { InputRef, ViewRef } from "nsh";
import { mount, signal, View } from "nsh";
import { AppButton } from "./buttons";
import { Checkbox, Dropdown } from "./choice";
import { DateField, IntInput, MoneyInput, PercentInput, TinInput } from "./fields";
import { Text } from "./text";

const OPTIONS = [
  { value: "a", label: "Alpha" },
  { value: "b", label: "Bravo" },
];

function App() {
  const money = signal("");
  const pct = signal("");
  const int = signal("");
  const tin = signal("");
  const date = signal("");
  const pick = signal<string | null>(null);
  const ticked = signal(false);
  let moneyRef: InputRef | undefined;
  let pctRef: InputRef | undefined;
  let intRef: InputRef | undefined;
  let tinRef: InputRef | undefined;
  let dateRef: InputRef | undefined;
  let dropRef: ViewRef | undefined;
  let boxRef: ViewRef | undefined;
  return (
    <View class="gap-2 p-3">
      <View class="flex-row flex-wrap gap-x-3">
        <Text variant="cap">{`Money: ${money() || "empty"}`}</Text>
        <Text variant="cap">{`Pct: ${pct() || "empty"}`}</Text>
        <Text variant="cap">{`Int: ${int() || "empty"}`}</Text>
        <Text variant="cap">{`Tin: ${tin() || "empty"}`}</Text>
        <Text variant="cap">{`Date: ${date() || "empty"}`}</Text>
        <Text variant="cap">{`Pick: ${pick() ?? "none"}`}</Text>
        <Text variant="cap">{`Ticked: ${ticked()}`}</Text>
      </View>
      <View class="flex-row flex-wrap gap-2">
        <AppButton label="Focus money" size="sm" testID="go-money" onPress={() => moneyRef?.focus()} />
        <AppButton label="Focus pct" size="sm" testID="go-pct" onPress={() => pctRef?.focus()} />
        <AppButton label="Focus int" size="sm" testID="go-int" onPress={() => intRef?.focus()} />
        <AppButton label="Focus tin" size="sm" testID="go-tin" onPress={() => tinRef?.focus()} />
        <AppButton label="Focus date" size="sm" testID="go-date" onPress={() => dateRef?.focus()} />
        <AppButton label="Focus dropdown" size="sm" testID="go-drop" onPress={() => dropRef?.focus()} />
        <AppButton label="Focus checkbox" size="sm" testID="go-box" onPress={() => boxRef?.focus()} />
      </View>
      <MoneyInput label="Money" value={money()} onChange={(v) => money.set(v)} inputRef={(r) => (moneyRef = r)} testID="f-money" />
      <PercentInput label="Pct" value={pct()} onChange={(v) => pct.set(v)} inputRef={(r) => (pctRef = r)} testID="f-pct" />
      <IntInput label="Int" value={int()} onChange={(v) => int.set(v)} inputRef={(r) => (intRef = r)} testID="f-int" />
      <TinInput label="TIN" value={tin()} onChange={(v) => tin.set(v)} inputRef={(r) => (tinRef = r)} testID="f-tin" />
      <DateField label="Date" value={date()} onChange={(v) => date.set(v)} inputRef={(r) => (dateRef = r)} testID="f-date" />
      <Dropdown label="Pick" options={OPTIONS} value={pick()} onChange={(v) => pick.set(v)} focusRef={(r) => (dropRef = r)} testID="f-drop" />
      <Checkbox label="Ticked" checked={ticked()} onChange={(on) => ticked.set(on)} focusRef={(r) => (boxRef = r)} testID="f-box" />
    </View>
  );
}

mount(App);
