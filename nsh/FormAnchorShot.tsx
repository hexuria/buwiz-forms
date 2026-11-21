// A test fixture, not a screen: the form page for the demo taxpayer (shots/ffrowline.test.tsx's scroll-anchor
// proof opens it). Like FormAnyShot, but the session is kept and a "Check" button sits outside the page's
// Scroll, so pressing it checks without the press scrolling the page back to the header. PNGs only; headless
// runs take none and still pass.
import { host, mount, View } from "nsh";
import "./theme/fonts";
import { createDemoTaxpayer, DEMO } from "./bir/testing";
import { AppButton } from "./ui";
import { FormPage, type FormTarget } from "./screens/form/FormPage";
import { Toaster } from "./ui";
import type { FormSession } from "./screens/form/session";

interface Params {
  formCode?: string;
  year?: number;
  period?: number;
  mode?: FormTarget["mode"];
}

let session: FormSession | null = null;

function Preview() {
  createDemoTaxpayer();
  const p = (host.params || {}) as Params;
  return (
    <View class="flex-1 bg-card">
      <FormPage
        tin={DEMO.tin}
        formCode={p.formCode ?? "0605"}
        year={p.year ?? 2023}
        period={p.period ?? 1}
        mode={p.mode ?? "return"}
        onSession={(s) => {
          session = s;
        }}
      />
      {/* Outside the form's Scroll: a press here never scrolls the page (shots/ffrowline anchor proof). */}
      <View class="absolute top-2 right-2" style={{ pointerEvents: "box-none" }}>
        <AppButton testID="fx-check" size="sm" label="Check" onPress={() => session?.check()} />
      </View>
      <Toaster bottom={96} />
    </View>
  );
}

mount(Preview);
