// The entry the masked-field tests open (fields.test.tsx): one of each masked box with a readout of the value it reports, a
// count of how often it reported, and a plain box to leave the others for. Tests only; nothing imports it. The readouts are
// first so a phone-sized window shows them all.
import type { InputRef } from "nsh";
import { mount, signal, View } from "nsh";
import { AppButton } from "./buttons";
import { IntInput, MoneyInput, PercentInput, TextField, TinInput } from "./fields";
import { Text } from "./text";

function App() {
  const amount = signal("");
  const grouped = signal("1234.56");
  const reports = signal(0);
  const textWrites = signal(0);
  const caretWrites = signal(0);
  const stored = signal("-250");
  const free = signal("");
  const whole = signal("1234.5");
  const wholeFive = signal("12345");
  const odd = signal("1e21");
  const long = signal("1.255");
  const tin = signal("0001234530");
  const rate = signal("12.5");
  const code = signal("0123");
  const zcode = signal("000");
  const other = signal("");
  const host = signal("0.00");
  const two = signal("2000");
  const invalid = signal("none");
  const invalidCount = signal(0);
  const cb = signal("2000");
  const cbBlurs = signal(0);
  const showCb = signal(true);
  let twoRef: InputRef | undefined;
  const armHeld = signal(false);
  const refill = signal("1250");
  const refillOnce = signal(true);
  const refillBefore = signal(false);
  return (
    <View class="gap-2 p-3">
      <View class="flex-row flex-wrap gap-x-3">
        <Text variant="cap">{`Amount: ${amount() || "empty"}`}</Text>
        <Text variant="cap">{`Grouped: ${grouped() || "empty"}`}</Text>
        <Text variant="cap">{`Reports: ${reports()}`}</Text>
        <Text variant="cap">{`Text writes: ${textWrites()}`}</Text>
        <Text variant="cap">{`Caret writes: ${caretWrites()}`}</Text>
        <Text variant="cap">{`Stored: ${stored() || "empty"}`}</Text>
        <Text variant="cap">{`Free: ${free() || "empty"}`}</Text>
        <Text variant="cap">{`Whole: ${whole() || "empty"}`}</Text>
        <Text variant="cap">{`WholeFive: ${wholeFive() || "empty"}`}</Text>
        <Text variant="cap">{`Odd: ${odd() || "empty"}`}</Text>
        <Text variant="cap">{`Long: ${long() || "empty"}`}</Text>
        <Text variant="cap">{`Tin: ${tin() || "empty"}`}</Text>
        <Text variant="cap">{`Rate: ${rate() || "empty"}`}</Text>
        <Text variant="cap">{`Code: ${code() || "empty"}`}</Text>
        <Text variant="cap">{`Zcode: ${zcode() || "empty"}`}</Text>
        <Text variant="cap">{`Host: ${host() || "empty"}`}</Text>
        <Text variant="cap">{`Refill: ${refill() || "empty"}`}</Text>
        <Text variant="cap">{`Two: ${two() || "empty"}`}</Text>
        <Text variant="cap">{`Invalid: ${invalid()}`}</Text>
        <Text variant="cap">{`Invalid count: ${invalidCount()}`}</Text>
        <Text variant="cap">{`Cb: ${cb() || "empty"}`}</Text>
        <Text variant="cap">{`Cb blurs: ${cbBlurs()}`}</Text>
        <Text variant="cap">{`Cb shown: ${showCb()}`}</Text>
      </View>
      <View class="flex-row flex-wrap gap-2">
      <TextField width={100} label="Other" value={other()} onChange={(v) => other.set(v)} testID="m-other" />
      <MoneyInput
        width={100}
        label="Amount"
        value={amount()}
        onChange={(v) => {
          amount.set(v);
          reports.update((n) => n + 1);
        }}
        testID="m-amount"
        writeProbe={(write) => (write === "text" ? textWrites.update((n) => n + 1) : caretWrites.update((n) => n + 1))}
      />
      <MoneyInput
        width={100}
        label="Grouped"
        value={grouped()}
        onChange={(v) => grouped.set(v)}
        testID="m-grouped"
        writeProbe={(write) => (write === "text" ? textWrites.update((n) => n + 1) : caretWrites.update((n) => n + 1))}
      />
      <MoneyInput width={100} label="Stored" value={stored()} onChange={(v) => stored.set(v)} testID="m-stored" />
      <MoneyInput width={100} label="Free" negative value={free()} onChange={(v) => free.set(v)} testID="m-free" />
      <MoneyInput width={100} label="Whole" whole value={whole()} onChange={(v) => whole.set(v)} testID="m-whole" />
      <MoneyInput width={100} label="WholeFive" whole value={wholeFive()} onChange={(v) => wholeFive.set(v)} testID="m-whole-five" />
      <MoneyInput width={100} label="Odd" value={odd()} onChange={(v) => odd.set(v)} testID="m-odd" />
      <MoneyInput width={100} label="Long" value={long()} onChange={(v) => long.set(v)} testID="m-long" />
      <TinInput width={100} label="TIN" value={tin()} onChange={(v) => tin.set(v)} testID="m-tin" />
      <PercentInput width={100} label="Rate" value={rate()} onChange={(v) => rate.set(v)} testID="m-rate" />
      <IntInput width={100} label="Code" maxDigits={4} value={code()} onChange={(v) => code.set(v)} testID="m-code" />
      <IntInput width={100} label="Zcode" maxDigits={4} zeroEmpty={false} value={zcode()} onChange={(v) => zcode.set(v)} testID="m-zcode" />
      <AppButton label="Set two outside" size="sm" testID="m-set-two" onPress={() => two.set("3000")} />
      <AppButton label="Blur two" size="sm" testID="m-blur-two" onPress={() => twoRef?.blur()} />
      <AppButton label="Insert x in two" size="sm" testID="m-insert-x-two" onPress={() => twoRef?.insertText("x")} />
      <AppButton label="Arm held outside" size="sm" testID="m-arm-held" onPress={() => armHeld.set(true)} />
      <AppButton label="Arm refill before empty" size="sm" testID="m-arm-refill-before" onPress={() => {
        refillOnce.set(false);
        refillBefore.set(true);
      }} />
      <AppButton label="Unmount cb" size="sm" testID="m-unmount-cb" onPress={() => showCb.set(false)} />
      <MoneyInput
        width={100}
        label="Two"
        value={two()}
        onChange={(v) => two.set(v === "" ? "0" : v)}
        inputRef={(r) => (twoRef = r)}
        onFocus={() => {
          if (armHeld()) {
            armHeld.set(false);
            two.set("3000");
          }
        }}
        testID="m-two"
      />
      <If when={showCb()}>
        <MoneyInput
          width={100}
          label="Cb"
          value={cb()}
          onChange={(v) => cb.set(v === "" ? "0" : v)}
          onBlur={() => cbBlurs.update((n) => n + 1)}
          onLeftUnreadable={(typed, kept) => {
            invalidCount.update((n) => n + 1);
            invalid.set(`${typed}|${kept}`);
          }}
          testID="m-cb"
        />
      </If>
      <MoneyInput
        width={100}
        label="Refill"
        value={refill()}
        onFocus={() => {
          if (!refillBefore()) return;
          refillBefore.set(false);
          refill.set("3000");
        }}
        onChange={(v) => {
          refill.set(v);
          if (v === "" && refillOnce()) {
            refillOnce.set(false);
            setTimeout(() => refill() === "" && refill.set("1250"), 50);
          }
        }}
        testID="m-refill"
      />
      <MoneyInput
        width={100}
        label="Host"
        value={host()}
        onChange={(v) => {
          // A form that cannot hold a blank amount answers "" with its stored zero, as the host does.
          host.set(v === "" ? "0.00" : v);
        }}
        testID="m-host"
      />
      </View>
    </View>
  );
}

mount(App);
