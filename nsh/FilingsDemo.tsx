// A test fixture, not a screen: the frame over a scratch database for screens/filings/*.test.tsx and shots/filings.test.tsx.
// What a test asks for goes in `open(entry, { host: { params } })`:
//   seed   "full" (default): the demo taxpayer, her Cebu branch, a second taxpayer and seven returns (screens/filings/seed.ts IDS)
//          "none": the demo taxpayer alone, no returns
//   start  the dev route (default "view=filings")
//   go     params to open Filings with once the frame is up, as a navigation preset: { statuses: ["Draft"], updatedMonth: "2026-11" }
//   generated  true: one generated Queued return joins the list in the store's cache a moment after the start. Real-window shots only:
//          the dry-run submission worker sends a queued return at once (a headless test pins the worker elsewhere, see Filings.test.tsx)
//   legacy "rows": bir.list_legacy_filed_returns answers the returns imported from the old program (LEGACY below: the demo taxpayer's head
//          office and Cebu branch, one for the second taxpayer); "second": only the second taxpayer's. Default: the host's own answer
//          (none: nothing was imported). An import cannot be made through the host here (the folder dialog is the host's), so a test
//          stands in for the host's answer, as SettingsDemo does for the import.
//   send   true: bir.list_returns answers the send states the worker leaves (SENDS below) on four seeded returns: the 2551Q draft
//          refused, the queued 1601C with an unknown outcome, the Cebu 2551Q held and the second taxpayer's 1601C waiting for its next
//          try (those two as Queued). A worker that leaves them cannot run here (nothing is sent from a test), so a test stands in for it.
//   dry    true: bir.list_returns answers the seeded Submitted 2550Q as the dry-run worker leaves it (send "test_sent": only simulated,
//          nothing reached BIR); the cards show "Dry run". The page's save never stores a dry-run fact, so a test stands in for the worker.
// It sits at the nsh root because a real-window run serves assets (fonts, icons) from the entry's folder down.
import { batch, effect, host, mount, untracked, View } from "nsh";
import "./theme/fonts";
import { bir, type LegacyFiledReturn, type ReturnRow, type SendState } from "./bir";
import { createDemoTaxpayer, DEMO } from "./bir/testing";
import { CEBU, IDS, SECOND, seedFilings } from "./screens/filings/seed";
import { go, store } from "./shell";
import { app } from "./shell/app";
import { Shell } from "./shell/Shell";

interface Params {
  seed?: "full" | "none";
  start?: string;
  go?: Record<string, unknown>;
  generated?: boolean;
  legacy?: "rows" | "second";
  send?: boolean;
  dry?: boolean;
}

const sendOf = (stage: SendState["stage"], line: string, code: string | null = null): SendState => ({
  stage, line, code, next_try_at: stage === "held" || stage === "waiting" ? "2026-11-24T07:40:00+00:00" : null, can_resolve: stage === "needs_attention",
});
/** The send states the host answers on four seeded returns (param `send`), and the status each then has. */
const SENDS: Record<number, { status: string; send: SendState }> = {
  [IDS.draft]: { status: "Draft", send: sendOf("refused", "This TIN is not on your Buwiz account.", "tin_not_active") },
  [IDS.queued]: { status: "Queued", send: sendOf("needs_attention", "The BIR may have received this return. Check for the BIR's confirmation email before sending it again.") },
  [IDS.cebu]: { status: "Queued", send: sendOf("held", "Buwiz is holding sends for now.") },
  [IDS.second]: { status: "Queued", send: sendOf("waiting", "Not sent yet: BIR is not accepting returns right now. It tries again by itself.") },
};

/** bir.list_returns with SENDS on its rows. */
function answerSends(): void {
  const real = bir.list_returns;
  const table = bir as unknown as Record<string, typeof bir.list_returns>;
  table.list_returns = (which) => real(which).map((r) => (SENDS[r.id] ? { ...r, ...SENDS[r.id] } : r));
}

/** bir.list_returns with the seeded Submitted return answered as a dry-run send (param `dry`). */
function answerDryRun(): void {
  const real = bir.list_returns;
  const table = bir as unknown as Record<string, typeof bir.list_returns>;
  const dry = sendOf("test_sent", "Test send. Nothing reached the BIR.");
  table.list_returns = (which) => real(which).map((r) => (r.id === IDS.submitted ? { ...r, send: dry } : r));
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
  form_code: "1601C", taxable_year: 2026, period: 10, period_label: "October 2026", status: "Queued", updated_at: "2026-11-14 09:30:00", return_key: 10, submitted_via_relay: false, send: null,
};

function Demo() {
  const p = host.params as Params;
  if (p.legacy) answerLegacy(p.legacy);
  if (p.send) answerSends();
  if (p.dry) answerDryRun();
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
