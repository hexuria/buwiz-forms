// A test fixture, not a screen: the form page for the demo taxpayer (FormPage.test.tsx and shots/form.test.tsx open it).
// No Toaster: the kit's toast layer covers the pill (nsh/AGENTS.md pitfall 15); the page's toasts go nowhere here.
import { mount, View } from "nsh";
import "../../theme/fonts";
import { createDemoTaxpayer, DEMO } from "../../bir/testing";
import { FormPage } from "./FormPage";

function Preview() {
  createDemoTaxpayer();
  return (
    <View class="flex-1 bg-background">
      <FormPage tin={DEMO.tin} formCode="1702RT" year={2025} period={0} mode="return" />
    </View>
  );
}

mount(Preview);
