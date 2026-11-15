// A test fixture, not a screen: the form page for the demo taxpayer (FormPage.test.tsx and shots/form.test.tsx open it).
// The frame's surface (bg-card) and its Toaster at bottom 96: toasts sit above the pill, which stays pressable (nsh/AGENTS.md pitfall 15).
import { mount, View } from "nsh";
import "../../theme/fonts";
import { createDemoTaxpayer, DEMO } from "../../bir/testing";
import { Toaster } from "../../ui";
import { FormPage } from "./FormPage";

function Preview() {
  createDemoTaxpayer();
  return (
    <View class="flex-1 bg-card">
      <FormPage tin={DEMO.tin} formCode="1702RT" year={2025} period={0} mode="return" />
      <Toaster bottom={96} />
    </View>
  );
}

mount(Preview);
