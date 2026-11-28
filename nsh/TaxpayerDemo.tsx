// A test fixture, not a screen: the frame over the demo taxpayer (Juana Santos) for the taxpayer screens' tests
// (screens/taxpayer/*.test.tsx) and shots (shots/taxpayer.test.tsx). What a test asks for goes in open's host params:
//   demo    default true: the demo taxpayer; false: an empty database
//   years   default true: this tax year with 1601C and 2551Q; false: no tax year yet
//   branch  a second branch (00001, Cebu branch)          peer   a second individual (generated: Sample Person, 1701Q)
//   setup   a setup of the user's ("Bookkeeping clients") conflict  1701 and 1701A on the year's list too
//   pin     the demo taxpayer has the profile PIN 1234    start  the dev route ("view=profile")
//   sent    "dry": every 1601C deadline of the year reads as a Submitted dry run (only simulated: still due); "real": as real sends
//           (every period filed). Generated reads: nothing is stored
//   card    "export": only the Export this profile card and a toaster (Settings draws it)
//   google  true or false: whether this build can start Google's sign-in for mail (else the test process's environment decides)
// It sits at the nsh root because a real-window run serves assets (fonts, icons) from the entry's folder down.
import { host, mount, signal, View } from "nsh";
import "./theme/fonts";
import { bir } from "./bir";
import { createDemoTaxpayer, DEMO, installFakeTins, warmForms, type FakeActivateKind } from "./bir/testing";
import { ProfileExportCard } from "./screens/taxpayer";
import { Shell } from "./shell/Shell";
import { store } from "./shell/store";
import { Text, Toaster } from "./ui";

interface Params {
  demo?: boolean;
  years?: boolean;
  branch?: boolean;
  peer?: boolean;
  setup?: boolean;
  conflict?: boolean;
  pin?: boolean;
  start?: string;
  card?: "export";
  sent?: "dry" | "real";
  google?: boolean;
  exportDownload?: { name?: string; error?: string; cancelled?: boolean };
  exportLayout?: boolean;
  year?: number;
  selfEmployed?: boolean;
  savedReturn?: { formCode: string; year: number; period: number };
  elections?: Record<number, "GraduatedUnspecified" | "EightPercent" | null>;
  /** The account's TIN list the fixture fakes: one state for the demo TIN, "multi" adds the peer, "stale" a stale list, "incomplete" a list without the not_activated rows, "error" a list that cannot be read (signed out). */
  tins?: "activated" | "pending" | "fix" | "not_activated" | "test" | "stale" | "multi" | "incomplete" | "error";
  /** The refusal activate_tin throws (the host's own sentence), or "ok" for the added row. */
  activate?: FakeActivateKind;
  /** After this many account_tins reads the fake throws (the good-list-then-error tests). */
  tinsErrorAfter?: number;
  /** After this many reads the fake's sign-in becomes owner B (the account-switch test). */
  tinsOwnerFlipAfter?: number;
  /** The fake account_tins answer is delayed this long. */
  tinsDelayMs?: number;
  /** An arbitrary refusal sentence activate_tin throws verbatim (the pass-through test). */
  refusalText?: string;
}

if (/form=/.test(String((host.params as Params)?.start ?? ""))) warmForms();

function seed(p: Params) {
  const year = p.year ?? new Date().getFullYear();
  const taxpayer = createDemoTaxpayer();
  const head = taxpayer.branches[0];
  if (p.years !== false && bir.list_tax_years(head.tin).length === 0) {
    bir.add_tax_year(head.tin, year, false);
    bir.set_yearly_forms(head.tin, year, p.conflict ? ["1601C", "2551Q", "1701", "1701A"] : ["1601C", "2551Q"]);
  }
  if (p.selfEmployed) bir.set_year_facts(head.tin, year, { tax_classification: "SelfEmployed" });
  for (const [y, election] of Object.entries(p.elections ?? {})) {
    bir.set_year_facts(head.tin, Number(y), { income_tax_election: election });
  }
  if (p.branch && !taxpayer.branches.some((b) => b.code === "00001")) bir.create_branch(taxpayer.tin_root, { branch_code: "00001", name: "Cebu branch" });
  if (p.peer && !bir.list_taxpayers().some((t) => t.tin_root === "000111233")) {
    const peer = bir.create_taxpayer({ tin_root: "000-111-233", name: "Sample Person", taxpayer_type: "Individual", head_office: { rdo_code: "050", registered_address: "Makati City", zip_code: "1226" } });
    bir.add_tax_year(peer.branches[0].tin, year, false);
    bir.set_yearly_forms(peer.branches[0].tin, year, ["1701Q", "2551Q"]);
  }
  if (p.setup && p.years !== false && !bir.list_setups("Individual").some((s) => !s.built_in)) bir.save_setup(head.tin, year, "Bookkeeping clients");
  if (p.pin) bir.set_taxpayer_pin(taxpayer.tin_root, "1234");
}

/** bir.deadlines_between with every 1601C deadline read as Submitted: only simulated (`dry`) or sent for real. */
function show1601CAsSent(dry: boolean): void {
  const fake = bir as { deadlines_between: typeof bir.deadlines_between };
  const real = fake.deadlines_between;
  fake.deadlines_between = (from, to) => real(from, to).map((d) => (d.form_code === "1601C" ? { ...d, status: "Submitted", submitted_dry_run: dry } : d));
}

/** The inbox state as a build with or without a Google client id answers it. */
function fixGoogle(available: boolean) {
  const table = bir as unknown as Record<string, unknown>;
  const real = bir.inbox_state;
  table.inbox_state = (tin: string) => ({ ...real(tin), google_available: available });
}

function fakeTins(kind: NonNullable<Params["tins"]>, p: Params, readout: (line: string) => void): void {
  installFakeTins(bir as unknown as Record<string, unknown>, kind, p.activate, readout, {
    errorAfter: p.tinsErrorAfter,
    ownerFlipAfter: p.tinsOwnerFlipAfter, delayMs: p.tinsDelayMs,
    refusalText: p.refusalText,
  });
}

function Demo() {
  const p = host.params as Params;
  if (p.demo !== false) seed(p);
  if (p.sent) show1601CAsSent(p.sent === "dry");
  if (typeof p.google === "boolean") fixGoogle(p.google);
  const tinsCalls = signal<string[]>([]);
  if (p.tins) fakeTins(p.tins, p, (line) => tinsCalls.set([...tinsCalls(), line]));
  const ready = signal(!p.savedReturn);
  if (p.savedReturn) {
    const target = p.savedReturn;
    const tin = createDemoTaxpayer().branches[0].tin;
    void bir.screen_form_open(tin, target.formCode, target.year, target.period).then((opened) => {
      bir.screen_form_save(target.formCode, opened.draft);
      ready.set(true);
    });
  }
  if (p.card === "export") {
    const calls = signal(0);
    const cardHeight = signal(0);
    if (p.exportDownload) {
      const download = p.exportDownload;
      const fake = bir as unknown as { export_profile: typeof bir.export_profile };
      fake.export_profile = async () => {
        calls.set(calls() + 1);
        if (download.error) throw new Error(download.error);
        if (download.cancelled) return { kind: "cancelled" };
        return { kind: "exported", path: `Downloads/eBIRForms/${download.name}` };
      };
    }
    // No Shell, so no boot: what was read before the seed is read again.
    store.bump();
    return (
      <View class="flex-1 bg-background p-6">
        <View onLayout={(e) => cardHeight.set(e.bounds.size.height)}>
          <ProfileExportCard />
        </View>
        <If when={p.exportLayout}>
          <Text>{`Export card height: ${cardHeight()}`}</Text>
        </If>
        <If when={p.exportDownload !== undefined}>
          <Text testID="export-calls">{`Export requests: ${calls()}`}</Text>
        </If>
        <Toaster />
      </View>
    );
  }
  return (
    <View class="flex-1">
      <If when={ready()}><Shell start={p.start} polling={false} /></If>
      <View class="absolute top-0 left-0" style={{ pointerEvents: "none", opacity: 0 }}>
        <Each of={tinsCalls()}>{(t) => <Text>{`tins ${t()}`}</Text>}</Each>
        <Each of={Object.keys(p.elections ?? {})} key={(y) => y}>
          {(y) => <Text>{`Election ${y()}: ${store.yearFacts(DEMO.tin, Number(y()))?.facts.income_tax_election ?? "Not set"}`}</Text>}
        </Each>
      </View>
    </View>
  );
}

mount(Demo);
