// A test fixture, not a screen: the frame over a scratch database for screens/filings/*.test.tsx and shots/filings.test.tsx.
// What a test asks for goes in `open(entry, { host: { params } })`:
//   seed   "full" (default): the demo taxpayer, her Cebu branch, a second taxpayer and seven returns (screens/filings/seed.ts IDS)
//          "none": the demo taxpayer alone, no returns
//   start  the dev route (default "view=filings")
//   go     params to open Filings with once the frame is up, as Home's tiles do: { statuses: ["Draft"], updatedMonth: "2026-11" }
//   generated  true: one generated Queued return joins the list in the store's cache a moment after the start. Real-window shots only:
//          the dry-run submission worker sends a queued return at once (a headless test pins the worker elsewhere, see Filings.test.tsx)
//   legacy "rows": bir.list_legacy_filed_returns answers the returns imported from the old program (LEGACY below: the demo taxpayer's head
//          office and Cebu branch, one for the second taxpayer); "second": only the second taxpayer's. Default: the host's own answer
//          (none: nothing was imported). An import cannot be made through the host here (the folder dialog is the host's), so a test
//          stands in for the host's answer, as SettingsDemo does for the import.
// It sits at the nsh root because a real-window run serves assets (fonts, icons) from the entry's folder down.
import { batch, effect, host, mount, untracked, View } from "nsh";
import "./theme/fonts";
import { bir, type LegacyFiledReturn, type ReturnRow } from "./bir";
import { createDemoTaxpayer, DEMO } from "./bir/testing";
import { CEBU, SECOND, seedFilings } from "./screens/filings/seed";
import { go, store } from "./shell";
import { app } from "./shell/app";
import { Shell } from "./shell/Shell";

interface Params {
  seed?: "full" | "none";
  start?: string;
  go?: Record<string, unknown>;
  generated?: boolean;
  legacy?: "rows" | "second";
}

/** What the host answers for a TIN once the old program's folder was imported (made up: the demo taxpayers, their branch). */
const legacy = (id: number, form: string, code: string | null, period: string, extra: Partial<LegacyFiledReturn> = {}): LegacyFiledReturn => ({
  id, form, form_code: code, period, amendment_version: null, email_copy: false, imported_at: "2026-11-14 12:00:00",
  source_file: `demo-${id}.xml`, ...extra,
});
const LEGACY: Record<string, LegacyFiledReturn[]> = {
  [DEMO.tin]: [
    legacy(1, "1601Cv2018", "1601C", "062025"),
    legacy(2, "1601Cv2018", "1601C", "062025", { amendment_version: 2 }),
    legacy(3, "1601Cv2018", "1601C", "062025", { email_copy: true }),
    legacy(4, "2550Qv2018", "2550Q", "032025"),
    legacy(5, "1701v2018", "1701", "2024", { imported_at: "2026-11-13 12:00:00" }),
    legacy(6, "9999Xv2000", null, "2023", { imported_at: "2026-11-13 12:00:00" }),
  ],
  [CEBU.tin]: [legacy(7, "1601Cv2018", "1601C", "072025")],
  [SECOND.tin]: [legacy(8, "2551Qv2018", "2551Q", "2025Q3")],
};

/** The host's answer for a TIN, from LEGACY (all of it, or the second taxpayer's alone). */
function answerLegacy(only: "rows" | "second"): void {
  const table = bir as unknown as Record<string, (tin: string) => LegacyFiledReturn[]>;
  table.list_legacy_filed_returns = (tin) => (only === "second" && tin !== SECOND.tin ? [] : (LEGACY[tin] ?? []));
}

/** A generated queued return (not in the database): what a window shows for the moment the worker has not taken one. */
const QUEUED: ReturnRow = {
  id: 9001, tin: DEMO.tin, tin_root: "000123453", branch_code: "00000", branch_name: "Head office", taxpayer_name: "Juana Santos", branch_count: 2,
  form_code: "1601C", taxable_year: 2026, period: 10, period_label: "October 2026", status: "Queued", updated_at: "2026-11-14 09:30:00", return_key: 10, submitted_via_relay: false,
};

function Demo() {
  const p = host.params as Params;
  if (p.legacy) answerLegacy(p.legacy);
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
