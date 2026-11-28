// A test fixture of the form page, not a screen: shots/f21.test.tsx opens it. The demo taxpayer's 1601C (Sep 2025) is queued in the
// scratch database and the app's own dry-run submission worker sends it (nothing reaches BIR); once the stored return says Submitted the
// page opens on it and says "Test send: not filed. Nothing reached the BIR.". At the nsh root: a real-window run serves assets only from the entry's folder.
import { mount, signal, Text, View } from "nsh";
import "./theme/fonts";
import { bir } from "./bir";
import { createDemoTaxpayer, DEMO } from "./bir/testing";
import { FormPage } from "./screens/form/FormPage";
import { Toaster } from "./ui";

const YEAR = 2025;
const MONTH = 9;
const sent = signal(false);
const gaveUp = signal(false);
/** How long the fixture waits for the worker (the worker sends a queued return at once). */
const WAIT_MS = 30_000;

function sendThroughTheWorker(): void {
  createDemoTaxpayer();
  if (bir.list_tax_years(DEMO.tin).length === 0) bir.add_tax_year(DEMO.tin, YEAR, false);
  bir.set_yearly_forms(DEMO.tin, YEAR, ["1601C"]);
  bir.set_year_facts(DEMO.tin, YEAR, { eopt_tier: "Micro" });
  bir.save_form_draft(DEMO.tin, "1601C", YEAR, MONTH, "Draft", { tax_14_total_compensation: 100000, tax_25_total_taxes_withheld: 10000 });
  bir.queue_form_draft(DEMO.tin, "1601C", YEAR, MONTH);
  const until = Date.now() + WAIT_MS;
  const look = () => {
    const stored = (bir.get_form_draft(DEMO.tin, "1601C", YEAR, MONTH) ?? {}) as { status?: string };
    if (stored.status === "Submitted") sent.set(true);
    else if (Date.now() > until) gaveUp.set(true);
    else setTimeout(look, 100);
  };
  look();
}

function Preview() {
  sendThroughTheWorker();
  return (
    <View class="flex-1 bg-card">
      <If when={sent()} else={<Text testID="fx-sending">{gaveUp() ? "The worker did not send the return." : "Sending…"}</Text>}>
        <FormPage tin={DEMO.tin} formCode="1601C" year={YEAR} period={MONTH} mode="return" />
      </If>
      <Toaster bottom={96} />
    </View>
  );
}

mount(Preview);
