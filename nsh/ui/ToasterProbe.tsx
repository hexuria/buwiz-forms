// A test fixture, not a screen: the kit's Toaster over a floating bar at the bottom of the window (the form page's
// pill), the way the frame mounts it. toaster.test.tsx opens it. Buttons at the top show a plain toast and an undo toast
// (60 s, so a slow real-window run still finds it).
import { mount, signal, View } from "nsh";
import { AppButton } from "./buttons";
import { toast, Toaster, undoToast } from "./layers";
import { Text } from "./text";

function ToasterProbe() {
  const pill = signal(0);
  const undone = signal(0);
  return (
    <View class="flex-1 bg-background">
      <View class="flex-row items-center gap-2 p-4">
        <AppButton label="Show toast" testID="probe-toast" onPress={() => toast("Draft saved")} />
        <AppButton label="Show undo" testID="probe-undo" onPress={() => undoToast("Return deleted", () => undone.set(undone() + 1), 60_000)} />
      </View>
      <Text variant="body">{`Pill pressed: ${pill()}`}</Text>
      <Text variant="body">{`Undone: ${undone()}`}</Text>
      <View class="absolute inset-x-0 bottom-0 flex-row items-center justify-center p-3">
        <AppButton label="Next" variant="primary" testID="probe-pill" onPress={() => pill.set(pill() + 1)} />
      </View>
      <Toaster bottom={96} />
    </View>
  );
}

mount(ToasterProbe);
