// A test fixture of the form page (B1), not a screen: shots/form.test.tsx opens it. It lives at the nsh root because a
// real-window run serves assets (fonts, icons) only from the entry's folder down (nsh/AGENTS.md pitfalls).
import { mount, View } from "nsh";
import "./theme/fonts";
import { createDemoTaxpayer, DEMO } from "./bir/testing";
import { FormPage } from "./screens/form/FormPage";

function Preview() {
  createDemoTaxpayer();
  return (
    <View class="flex-1 bg-background">
      <FormPage tin={DEMO.tin} formCode="2551Q" year={2025} period={1} mode="return" />
    </View>
  );
}

mount(Preview);
