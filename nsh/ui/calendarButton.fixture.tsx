// The entry the CalendarButton test opens (calendarButton.test.tsx): the button on its own, with a readout of the day it reports.
// Tests only; nothing imports it.
import { mount, signal, View } from "nsh";
import "../theme/fonts";
import { CalendarButton } from "./fields";
import { Text } from "./text";

function App() {
  const picked = signal("2026-11-10");
  const calls = signal(0);
  return (
    <View class="items-start gap-3 p-4">
      <Text testID="cb-picked">{`Picked: ${picked() || "empty"}`}</Text>
      <Text testID="cb-calls">{`Calls: ${calls()}`}</Text>
      <CalendarButton
        id="cb-trigger"
        value={picked()}
        onPick={(iso) => {
          picked.set(iso);
          calls.update((n) => n + 1);
        }}
        min="2026-11-03"
        max="2026-11-27"
      />
    </View>
  );
}

mount(App);
