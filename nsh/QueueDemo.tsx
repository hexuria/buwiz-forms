// A test fixture, not a screen: the frame over the demo taxpayer with a FAKE filed list, to show what the app does while the
// submission worker changes a return behind its back (shell/queue.test.tsx). bir.list_returns("filed") answers one return (the
// worker's), Queued until `sentAfter` ms after the start and Submitted from then on; bir.cancel_queued is refused as the host
// refuses a return the worker has taken. What a test asks for goes in open's host params:
//   sentAfter  ms after the start at which the fake worker sends the return (default 1200)
//   watch      true: the frame watches the Queued return (Shell watchQueued); false: nothing re-reads on its own
//   start      the dev route (default "view=filings")
// The readout shows how often the filed list was read ("filed reads: N") and the cancel-queue calls ("cancels: N").
// It sits at the nsh root because a real-window run serves assets (fonts, icons) from the entry's folder down.
import { host, mount, signal, Text, View } from "nsh";
import "./theme/fonts";
import { bir, type ReturnRow } from "./bir";
import { createDemoTaxpayer, DEMO } from "./bir/testing";
import { Shell } from "./shell/Shell";

interface Params {
  sentAfter?: number;
  watch?: boolean;
  start?: string;
}

const ROW: ReturnRow = {
  id: 9101, tin: DEMO.tin, tin_root: "000123453", branch_code: "00000", branch_name: "Head office", taxpayer_name: "Juana Santos", branch_count: 1,
  form_code: "1601C", taxable_year: 2026, period: 10, period_label: "October 2026", status: "Queued", updated_at: "2026-11-14 09:30:00", return_key: 10, submitted_via_relay: false,
};

const reads = signal(0);
const cancels = signal(0);
let sent = false;

function Demo() {
  const p = host.params as Params;
  createDemoTaxpayer();
  const fake = bir as unknown as Record<string, unknown>;
  // The store calls bir[name] at each read, so a swap holds. The counts are shown a tick later: a signal is not written inside a read.
  fake.list_returns = (which: string) => {
    if (which !== "filed") return [];
    setTimeout(() => reads.update((n) => n + 1), 0);
    return [{ ...ROW, status: sent ? "Submitted" : "Queued" }];
  };
  fake.cancel_queued = () => {
    setTimeout(() => cancels.update((n) => n + 1), 0);
    if (sent) throw new Error("Other error: Only an unclaimed queued 1601C snapshot can be canceled");
  };
  setTimeout(() => {
    sent = true;
  }, p.sentAfter ?? 1200);
  return (
    <View class="flex-1">
      <Shell start={p.start ?? "view=filings"} polling={false} watchQueued={p.watch === true} />
      <View class="absolute bottom-0 left-0 flex-row gap-3">
        <Text testID="demo-reads">{`filed reads: ${reads()}`}</Text>
        <Text testID="demo-cancels">{`cancels: ${cancels()}`}</Text>
      </View>
    </View>
  );
}

mount(Demo);
