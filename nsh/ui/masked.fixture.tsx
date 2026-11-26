// The entry the masked-field tests open (fields.test.tsx): one of each masked box with a readout of the value it reports, a
// count of how often it reported, and a plain box to leave the others for. Tests only; nothing imports it. The readouts are
// first so a phone-sized window shows them all.
import { mount, signal, View } from "nsh";
import { IntInput, MoneyInput, PercentInput, TextField, TinInput } from "./fields";
import { Text } from "./text";

function App() {
  const amount = signal("");
  const reports = signal(0);
  const textWrites = signal(0);
  const caretWrites = signal(0);
  const caretOps = signal("");
  const writeOrder = signal("");
  const stored = signal("-250");
  const free = signal("");
  const whole = signal("1234.5");
  const odd = signal("1e21");
  const long = signal("1.255");
  const tin = signal("0001234530");
  const rate = signal("12.5");
  const code = signal("0123");
  const other = signal("");
  const host = signal("0.00");
  return (
    <View class="gap-2 p-3">
      <View class="flex-row flex-wrap gap-x-3">
        <Text variant="cap">{`Amount: ${amount() || "empty"}`}</Text>
        <Text variant="cap">{`Reports: ${reports()}`}</Text>
        <Text variant="cap">{`Text writes: ${textWrites()}`}</Text>
        <Text variant="cap">{`Caret writes: ${caretWrites()}`}</Text>
        <Text variant="cap">{`Caret ops:${caretOps()}`}</Text>
        <Text variant="cap">{`Write ops:${writeOrder()}`}</Text>
        <Text variant="cap">{`Stored: ${stored() || "empty"}`}</Text>
        <Text variant="cap">{`Free: ${free() || "empty"}`}</Text>
        <Text variant="cap">{`Whole: ${whole() || "empty"}`}</Text>
        <Text variant="cap">{`Odd: ${odd() || "empty"}`}</Text>
        <Text variant="cap">{`Long: ${long() || "empty"}`}</Text>
        <Text variant="cap">{`Tin: ${tin() || "empty"}`}</Text>
        <Text variant="cap">{`Rate: ${rate() || "empty"}`}</Text>
        <Text variant="cap">{`Code: ${code() || "empty"}`}</Text>
        <Text variant="cap">{`Host: ${host() || "empty"}`}</Text>
      </View>
      <MoneyInput
        label="Amount"
        value={amount()}
        onChange={(v) => {
          amount.set(v);
          reports.update((n) => n + 1);
        }}
        testID="m-amount"
        writeProbe={(write, start, end) => {
          if (write === "text") {
            textWrites.update((n) => n + 1);
            writeOrder.update((ops) => `${ops}T`);
          }
          else {
            caretWrites.update((n) => n + 1);
            caretOps.update((ops) => `${ops} ${start}:${end}`);
            writeOrder.update((ops) => `${ops}C`);
          }
        }}
      />
      <MoneyInput label="Stored" value={stored()} onChange={(v) => stored.set(v)} testID="m-stored" />
      <MoneyInput label="Free" negative value={free()} onChange={(v) => free.set(v)} testID="m-free" />
      <MoneyInput label="Whole" whole value={whole()} onChange={(v) => whole.set(v)} testID="m-whole" />
      <MoneyInput label="Odd" value={odd()} onChange={(v) => odd.set(v)} testID="m-odd" />
      <MoneyInput label="Long" value={long()} onChange={(v) => long.set(v)} testID="m-long" />
      <TinInput label="TIN" value={tin()} onChange={(v) => tin.set(v)} testID="m-tin" />
      <PercentInput label="Rate" value={rate()} onChange={(v) => rate.set(v)} testID="m-rate" />
      <IntInput label="Code" maxDigits={4} value={code()} onChange={(v) => code.set(v)} testID="m-code" />
      <MoneyInput
        label="Host"
        value={host()}
        onChange={(v) => {
          // A form that cannot hold a blank amount answers "" with its stored zero, as the host does.
          host.set(v === "" ? "0.00" : v);
        }}
        testID="m-host"
      />
      <TextField label="Other" value={other()} onChange={(v) => other.set(v)} testID="m-other" />
    </View>
  );
}

mount(App);
