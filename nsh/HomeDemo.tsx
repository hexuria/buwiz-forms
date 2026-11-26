// A test fixture, not a screen: the frame over the demo taxpayer (Juana Santos) with what Home shows, for
// screens/home/*.test.tsx and shots/home.test.tsx. What a test asks for goes in `open(entry, { host: { params } })`:
//   demo      default true: create the demo taxpayer; false: an empty database (the welcome)
//   years     default true: tax years last year, this year and next year with 1601C, 2551Q and 2200M; false: no tax year
//   branch    true: add a second branch (00001, Cebu branch)       second true: add a second taxpayer (generated: Sample Trading Corp)
//   drafts    N: N saved 1601C returns of this year (periods 1..N)
//   undated   true: a saved 2200M return that holds no removal date yet (the "As needed" list)
//   filed     N: N filed 1601C returns of last year (periods 12, 11, ...). They are real saved returns that list_returns("filed") shows as
//             Submitted (a headless run has a submission worker for its first database only, so nothing really sends them)
//   sent      "dry": the filed returns (`filed`) read as sends the worker only simulated (send "test_sent") and the undated return
//             (`undated`) as a Submitted dry run; "real": as real sends. Generated reads: nothing is stored
//   dueSent   "dry": the saved Drafts' deadlines (`drafts`) read as Submitted dry runs (only simulated: still due); "real": as real
//             sends (filed: no longer due). Generated reads: nothing is stored
//   template  true: each filed return is saved as its form's template first, so its row shows its status chip
//   queued    true: one queued 2551Q row appears with the filed list once the frame is up (generated; the worker would send a real one)
//   archived  true: archive the taxpayer, so Home has no one to list
//   lateDraft true: a button (testID demo-save-draft) saves a 1601C draft straight through bir, behind the store's back (Home reads Drafts once per visit)
//   readout   true: print the view and its params at the bottom left (testID demo-route)
//   start     the dev route ("view=filings;theme=dark")        theme  the stored theme setting ("Dark")
// It sits at the nsh root because a real-window run serves assets (fonts, icons) from the entry's folder down.
import { effect, host, mount, signal, Text, untracked, View } from "nsh";
import "./theme/fonts";
import { bir, type ReturnRow } from "./bir";
import { AppButton } from "./ui";
import { createDemoTaxpayer } from "./bir/testing";
import { app } from "./shell/app";
import { Shell } from "./shell/Shell";
import { store } from "./shell/store";

interface Params {
  demo?: boolean;
  years?: boolean;
  branch?: boolean;
  second?: boolean;
  drafts?: number;
  undated?: boolean;
  filed?: number;
  sent?: "dry" | "real";
  dueSent?: "dry" | "real";
  template?: boolean;
  queued?: boolean;
  archived?: boolean;
  lateDraft?: boolean;
  readout?: boolean;
  start?: string;
  theme?: string;
}

const ready = signal(false);

/** A saved 1601C return of the period. */
async function saveReturn(tin: string, year: number, period: number): Promise<void> {
  const opened = await bir.screen_form_open(tin, "1601C", year, period);
  bir.screen_form_save("1601C", opened.draft);
}

/**
 * Makes the saved returns with these ids filed: list_returns("filed") lists them as Submitted (with their own ids, so Use as
 * template writes a real template from a real return) and list_returns("saved") leaves them out. The store reads through
 * `bir` at each call, so this holds across every re-read.
 */
function showAsSubmitted(ids: number[], dry: boolean): void {
  const host = bir as { list_returns: typeof bir.list_returns };
  const real = host.list_returns;
  const send = dry ? { stage: "test_sent" as const, line: "Test send. Nothing reached the BIR.", code: null, next_try_at: null, can_resolve: false } : null;
  host.list_returns = (which) => {
    const saved = real("saved").filter((r) => ids.includes(r.id));
    if (which === "filed") return [...saved.map((r) => ({ ...r, status: "Submitted", send })), ...real("filed")];
    if (which === "saved") return real("saved").filter((r) => !ids.includes(r.id));
    return real(which);
  };
}

async function seed(p: Params): Promise<void> {
  const taxpayer = createDemoTaxpayer();
  const head = taxpayer.branches[0];
  const year = new Date().getFullYear();
  if (p.years !== false && bir.list_tax_years(head.tin).length === 0) {
    for (const y of [year - 1, year, year + 1]) {
      bir.add_tax_year(head.tin, y, false);
      bir.set_yearly_forms(head.tin, y, ["1601C", "2551Q", "2200M"]);
    }
  }
  if (p.branch && !taxpayer.branches.some((b) => b.code === "00001")) bir.create_branch(taxpayer.tin_root, { branch_code: "00001", name: "Cebu branch" });
  if (p.second && !bir.list_taxpayers().some((t) => t.tin_root === "000111225")) {
    const second = bir.create_taxpayer({ tin_root: "000-111-225", name: "Sample Trading Corp", taxpayer_type: "Corporation", head_office: { rdo_code: "039", registered_address: "Quezon City", zip_code: "1100" } });
    bir.add_tax_year(second.branches[0].tin, year, false);
    bir.set_yearly_forms(second.branches[0].tin, year, ["1601C"]);
  }
  for (let period = 1; period <= (p.drafts ?? 0); period++) await saveReturn(head.tin, year, period);
  if (p.undated) {
    const opened = await bir.screen_form_open(head.tin, "2200M", year, 3);
    bir.screen_form_save("2200M", opened.draft);
  }
  const filed = p.filed ?? 0;
  for (let i = 0; i < filed; i++) await saveReturn(head.tin, year - 1, 12 - i);
  const filedIds = bir.list_returns("saved").filter((r) => r.taxable_year === year - 1 && r.period > 12 - filed).map((r) => r.id);
  if (filed > 0) showAsSubmitted(filedIds, p.sent === "dry");
  if (filed > 0 && p.template) for (const id of filedIds) bir.create_template_from_return(id);
  if (p.sent) showUndatedAsSent(p.sent === "dry");
  if (p.dueSent) showDraftDeadlinesAsSent(p.dueSent === "dry");
  if (p.archived) bir.set_taxpayer_archived(taxpayer.tin_root, true);
  if (p.theme) bir.set_setting("theme_preference", p.theme);
}

/** bir.as_needed_forms with every undated return read as Submitted: only simulated (`dry`) or sent for real. */
function showUndatedAsSent(dry: boolean): void {
  const host = bir as { as_needed_forms: typeof bir.as_needed_forms };
  const real = host.as_needed_forms;
  host.as_needed_forms = (year) => real(year).map((f) => ({ ...f, undated: f.undated.map((r) => ({ ...r, status: "Submitted", submitted_dry_run: dry })) }));
}

/** bir.deadlines_between with every Draft's deadline read as Submitted: only simulated (`dry`) or sent for real. */
function showDraftDeadlinesAsSent(dry: boolean): void {
  const host = bir as { deadlines_between: typeof bir.deadlines_between };
  const real = host.deadlines_between;
  host.deadlines_between = (from, to) => real(from, to).map((d) => (d.status === "Draft" ? { ...d, status: "Submitted", submitted_dry_run: dry } : d));
}

/** A queued return that nothing will send: generated, so the list shows Waiting to send and Cancel queue. */
function queuedRow(): ReturnRow {
  const year = new Date().getFullYear();
  return {
    id: 9101,
    tin: "00012345300000",
    tin_root: "000123453",
    branch_code: "00000",
    branch_name: "Head office",
    taxpayer_name: "Juana Santos",
    branch_count: 1,
    form_code: "2551Q",
    taxable_year: year,
    period: 1,
    period_label: `Q1 ${year}`,
    status: "Queued",
    updated_at: new Date().toISOString().slice(0, 19).replace("T", " "),
    return_key: null,
    submitted_via_relay: false,
    send: null,
  };
}

function Demo() {
  const p = host.params as Params;
  if (p.demo !== false) {
    seed(p)
      .catch((error) => console.error(`HomeDemo seed: ${error}`))
      .then(() => ready.set(true));
  }
  else ready.set(true);
  // The generated queued row joins the real filed list once the frame is up (the first look at a database is not news).
  if (p.queued) {
    effect(() => {
      if (!app.booted()) return;
      untracked(() => {
        const rows = [queuedRow(), ...store.returns("filed")];
        store.forget("returns:filed");
        store.read("returns:filed", () => rows, []);
      });
    });
  }
  return (
    <View class="flex-1">
      <If when={ready()}>
        <Shell start={p.start} polling={false} />
      </If>
      <If when={p.lateDraft}>
        <View class="absolute top-0 right-0">
          <AppButton testID="demo-save-draft" label="Save a draft" size="sm" onPress={() => void saveReturn("00012345300000", new Date().getFullYear(), 1)} />
        </View>
      </If>
      <If when={p.readout}>
        <View class="absolute bottom-0 left-0" style={{ pointerEvents: "none" }}>
          <Text testID="demo-route">{`${app.view()} ${JSON.stringify(app.params())}`}</Text>
        </View>
      </If>
    </View>
  );
}

mount(Demo);
