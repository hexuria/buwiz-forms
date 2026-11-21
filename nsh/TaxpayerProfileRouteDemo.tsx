import { host, mount, Scroll, View } from "nsh";
import "./theme/fonts";
import { bir } from "./bir";
import { createDemoTaxpayer, DEMO } from "./bir/testing";
import { TaxpayerPage } from "./screens/taxpayer/TaxpayerPage";
import { app, store } from "./shell";
import { Text, Toaster } from "./ui";

function Demo() {
  const params = host.params as { edit: string; fromForm?: boolean };
  const taxpayer = createDemoTaxpayer();
  bir.add_tax_year(DEMO.tin, 2026, false);
  store.bump();
  app.selectTaxpayer(taxpayer.tin_root, { quiet: true });
  app.selectYear(2026);
  app.go("form", { target: { mode: "return", tin: DEMO.tin, formCode: "2551Q", year: 2026, period: 3 } });
  app.go("profile", params);
  return (
    <View class="flex-1 bg-background">
      <If when={app.route().view === "profile"}>
        {/* The shell's main scroll (Shell.tsx): a section below the fold is scrolled to, not clipped. */}
        <Scroll testID="main-scroll" class="min-h-0 flex-1">
          <TaxpayerPage view="profile" params={params} />
        </Scroll>
      </If>
      <If when={app.route().view === "form"}><Text>Back on the return</Text></If>
      <Toaster />
    </View>
  );
}

mount(Demo);
