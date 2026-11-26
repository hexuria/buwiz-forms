// The CheckRow fixture: two selectable rows and a disabled one in a list, with a live readout of each
// row's state and of how many times each toggled (a double toggle shows as two).
import { mount, signal, View, type ViewRef } from "nsh";
import { AppButton } from "./buttons";
import { CheckRow } from "./choice";
import { ListGroup } from "./lists";
import { Text } from "./text";

const aOn = signal(false);
const bOn = signal(true);
const aToggles = signal(0);
let focusA: () => void = () => {};
const bToggles = signal(0);

mount(() => (
  <View class="gap-4 p-4">
    <ListGroup testID="cr-list">
      <CheckRow
        testID="cr-a"
        focusRef={(r) => {
          focusA = () => r.focus();
        }}
        checked={aOn()}
        onChange={(v) => {
          aOn.set(v);
          aToggles.set(aToggles() + 1);
        }}
        label="Alpha row"
      >
        <Text>Alpha</Text>
        <Text variant="meta" tone="fg3">its words</Text>
      </CheckRow>
      <CheckRow
        testID="cr-b"
        checked={bOn()}
        onChange={(v) => {
          bOn.set(v);
          bToggles.set(bToggles() + 1);
        }}
        label="Beta row"
      >
        <Text>Beta</Text>
      </CheckRow>
      <CheckRow testID="cr-off" checked={false} onChange={() => {}} label="Gamma row" disabled>
        <Text>Gamma</Text>
      </CheckRow>
    </ListGroup>
    <AppButton testID="cr-focus-a" label="Focus alpha" onPress={() => focusA()} />
    <Text testID="cr-readout">{`a:${aOn() ? "on" : "off"}/${aToggles()} b:${bOn() ? "on" : "off"}/${bToggles()}`}</Text>
  </View>
));
