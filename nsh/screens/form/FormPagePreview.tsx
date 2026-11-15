// A test fixture, not a screen: the form page for the demo taxpayer (FormPage.test.tsx and shots/form.test.tsx open it).
// The frame's surface (bg-card) and its Toaster at bottom 96: toasts sit above the pill, which stays pressable (nsh/AGENTS.md pitfall 15).
// Default: a 2551Q of 2025, quarter 1. `open(entry, { host: { params: { form: "1601C", period: 3 } } })` opens another form.
import { host, mount, View } from "nsh";
import "../../theme/fonts";
import { createDemoTaxpayer, DEMO } from "../../bir/testing";
import { Toaster } from "../../ui";
import { FormPage } from "./FormPage";

function Preview() {
  createDemoTaxpayer();
  const p = host.params as { form?: string; year?: number; period?: number };
  return (
    <View class="flex-1 bg-card">
      <FormPage tin={DEMO.tin} formCode={p.form ?? "2551Q"} year={p.year ?? 2025} period={p.period ?? 1} mode="return" />
      <Toaster bottom={96} />
    </View>
  );
}

mount(Preview);
