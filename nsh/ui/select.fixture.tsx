// The entry the selection tests open (ui/fields.select.test.tsx headless; shots/select.test.tsx in a real window): one of each number box with a readout of what it reports, a plain box
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
  const blank = signal("");
  const code = signal("000");
  const zeroWrites = signal<string[]>([]);
  const zeroReports = signal(0);
  const showReadonlyZeros = signal(false);
  const tin = signal("000123453");
  const kept = signal("1250");
  let someRef: InputRef | undefined;
  let zeroRef: InputRef | undefined;
  let rateRef: InputRef | undefined;
  let countRef: InputRef | undefined;
  return (
    <View class="gap-2 p-3">
      <View class="flex-row flex-wrap gap-x-3">
        <Text variant="cap">{`Some: ${some() || "empty"}`}</Text>
        <Text variant="cap">{`Zero: ${zero() || "empty"}`}</Text>
        <Text variant="cap">{`Rate: ${rate() || "empty"}`}</Text>
        <Text variant="cap">{`Count: ${count() || "empty"}`}</Text>
        <Text variant="cap">{`Blank: ${blank() || "empty"}`}</Text>
        <Text variant="cap">{`Code: ${code() || "empty"}`}</Text>
        <Text variant="cap">{`Zwrites: ${zeroWrites().join(",") || "none"}`}</Text>
        <Text variant="cap">{`Zreports: ${zeroReports()}`}</Text>
        <Text variant="cap">{`Tin: ${tin() || "empty"}`}</Text>
        <Text variant="cap">{`Kept: ${kept() || "empty"}`}</Text>
      </View>
      <View class="flex-row flex-wrap gap-2">
        <AppButton label="Focus some" size="sm" testID="s-go-some" onPress={() => someRef?.focus()} />
        <AppButton label="Focus zero" size="sm" testID="s-go-zero" onPress={() => zeroRef?.focus()} />
        <AppButton label="Focus zero same-turn 15" size="sm" testID="s-zero-same" onPress={() => {
          zeroRef?.focus();
          zeroRef?.insertText("15");
        }} />
        <AppButton label="Rate same-turn 15" size="sm" testID="s-rate-same" onPress={() => {
          rateRef?.focus();
          rateRef?.insertText("15");
        }} />
        <AppButton label="Count same-turn 15" size="sm" testID="s-count-same" onPress={() => {
          countRef?.focus();
          countRef?.insertText("15");
        }} />
        <AppButton label="Zero input then focus" size="sm" testID="s-zero-input-first" onPress={() => {
          zeroRef?.setSelection(0, 0);
          zeroRef?.insertText("15");
          setTimeout(() => zeroRef?.focus(), 20);
        }} />
        <AppButton label="Rate input then focus" size="sm" testID="s-rate-input-first" onPress={() => {
          rateRef?.setSelection(0, 0);
          rateRef?.insertText("15");
          setTimeout(() => rateRef?.focus(), 20);
        }} />
        <AppButton label="Count input then focus" size="sm" testID="s-count-input-first" onPress={() => {
          countRef?.setSelection(0, 0);
          countRef?.insertText("15");
          setTimeout(() => countRef?.focus(), 20);
        }} />
        <AppButton label="Zero to 1250" size="sm" testID="s-zero-1250" onPress={() => zero.set("1250")} />
        <AppButton label="Zero back to 0" size="sm" testID="s-zero-back-0" onPress={() => zero.set("0")} />
        <AppButton label="Zero to blank" size="sm" testID="s-zero-blank" onPress={() => zero.set("")} />
        <AppButton label="Show readonly zeros" size="sm" testID="s-show-readonly-zeros" onPress={() => showReadonlyZeros.set(true)} />
      </View>
      <TextField label="Free" value={free()} onChange={(v) => free.set(v)} testID="s-free" />
      <MoneyInput label="Some" value={some()} onChange={(v) => some.set(v)} inputRef={(r) => (someRef = r)} testID="s-some" />
      <MoneyInput
        label="Zero"
        value={zero()}
        onChange={(v) => {
          zeroReports.update((n) => n + 1);
          zero.set(v);
        }}
        inputRef={(r) => (zeroRef = r)}
        writeProbe={(write, start, end) => zeroWrites.update((a) => [...a, `${write}${start === undefined ? "" : `:${start}-${end}`}`])}
        testID="s-zero"
      />
      <PercentInput label="Rate" value={rate()} onChange={(v) => rate.set(v)} inputRef={(r) => (rateRef = r)} testID="s-rate" />
      <IntInput label="Count" value={count()} onChange={(v) => count.set(v)} inputRef={(r) => (countRef = r)} testID="s-count" />
      <MoneyInput label="Whole zero" whole prefix="" value="0" onChange={() => {}} testID="s-whole-zero" />
      <MoneyInput label="Blank" value={blank()} onChange={(v) => blank.set(v)} testID="s-blank" />
      <IntInput label="Code" value={code()} onChange={(v) => code.set(v)} zeroEmpty={false} testID="s-code" />
      <TinInput label="TIN" value={tin()} onChange={(v) => tin.set(v)} testID="s-tin" />
      <If when={showReadonlyZeros()}>
        <MoneyInput label="Disabled zero" value="0" onChange={() => {}} disabled testID="s-zero-disabled" />
        <MoneyInput label="Locked zero" value="0" onChange={() => {}} locked testID="s-zero-locked" />
      </If>
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
