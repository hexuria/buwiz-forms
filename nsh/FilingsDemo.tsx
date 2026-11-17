// A test fixture, not a screen: the frame over a scratch database for screens/filings/*.test.tsx and shots/filings.test.tsx.
// What a test asks for goes in `open(entry, { host: { params } })`:
//   seed   "full" (default): the demo taxpayer, her Cebu branch, a second taxpayer and seven returns (screens/filings/seed.ts IDS)
//          "none": the demo taxpayer alone, no returns
//   start  the dev route (default "view=filings")
//   go     params to open Filings with once the frame is up, as Home's tiles do: { statuses: ["Draft"], updatedMonth: "2026-11" }
//   generated  true: one generated Queued return joins the list in the store's cache a moment after the start. Real-window shots only:
//          the dry-run submission worker sends a queued return at once (a headless test pins the worker elsewhere, see Filings.test.tsx)
// It sits at the nsh root because a real-window run serves assets (fonts, icons) from the entry's folder down.
import { batch, effect, host, mount, untracked, View } from "nsh";
import "./theme/fonts";
import { bir, type ReturnRow } from "./bir";
import { createDemoTaxpayer, DEMO } from "./bir/testing";
import { seedFilings } from "./screens/filings/seed";
import { go, store } from "./shell";
import { app } from "./shell/app";
import { Shell } from "./shell/Shell";

interface Params {
  seed?: "full" | "none";
  start?: string;
  go?: Record<string, unknown>;
  generated?: boolean;
}

/** A generated queued return (not in the database): what a window shows for the moment the worker has not taken one. */
const QUEUED: ReturnRow = {
  id: 9001, tin: DEMO.tin, tin_root: "000123453", branch_code: "00000", branch_name: "Head office", taxpayer_name: "Juana Santos", branch_count: 2,
  form_code: "1601C", taxable_year: 2026, period: 10, period_label: "October 2026", status: "Queued", updated_at: "2026-11-14 09:30:00", return_key: 10,
};

function Demo() {
  const p = host.params as Params;
  if (p.seed === "none") createDemoTaxpayer();
  else seedFilings();
  if (p.go) {
    const params = p.go;
    effect(() => {
      if (!app.booted()) return;
      untracked(() => go("filings", params));
    });
  }
  if (p.generated) {
    effect(() => {
      if (!app.booted()) return;
      untracked(() =>
        setTimeout(() => {
          // One batch: what reads the list must not re-read it between the forget and our answer.
          batch(() => {
            store.forget("returns:filed");
            store.read("returns:filed", () => [...bir.list_returns("filed"), QUEUED], []);
          });
        }, 200),
      );
    });
  }
  return (
    <View class="flex-1">
      <Shell start={p.start ?? "view=filings"} polling={false} />
    </View>
  );
}

mount(Demo);
