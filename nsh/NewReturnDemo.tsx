// A test fixture, not a screen: the Start a return dialog on its own (no frame around it, an empty database), for shell/newReturn.test.tsx.
// The frame never opens it without a taxpayer, so this is how the header with no taxpayer is looked at (with one, shell/palette.test.tsx
// reads "Juana Santos ·" through the frame).
// It sits at the nsh root because a real-window run serves assets (fonts, icons) from the entry's folder down.
import { mount, View } from "nsh";
import "./theme/fonts";
import { NewReturn } from "./shell/NewReturn";

function Demo() {
  return (
    <View class="flex-1">
      <NewReturn />
    </View>
  );
}

mount(Demo);
