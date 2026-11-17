// A test fixture of the field controls' gallery (B2), not a screen: controls/*.test.tsx and shots/controls.test.tsx open it. It
// lives at the nsh root because a real-window run serves assets (fonts, icons) only from the entry's folder down.
import { mount } from "nsh";
import "./theme/fonts";
import { ControlsGallery } from "./screens/form/controls/ControlsGallery";
import { Toaster } from "./ui";
import { View } from "nsh";

function Preview() {
  return (
    <View class="flex-1 bg-background">
      <ControlsGallery />
      <Toaster bottom={96} />
    </View>
  );
}

mount(Preview);
