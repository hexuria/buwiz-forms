// A test fixture, not a screen: the form page for the demo taxpayer, the form named by the test
// (open(..., { host: { params: { formCode, year, period, mode } } })). Toasts are listed as texts (testID fx-toasts), and the
// ids of dismissed ones too, so a test reads what the page said.
import { cleanup, host, mount, signal, Text, View } from "nsh";
import { bir } from "../../bir";
import "../../theme/fonts";
import { createDemoTaxpayer, DEMO, installFakeTins, type FakeActivateKind, type FakeTinsKind } from "../../bir/testing";
import { app, store } from "../../shell";
import { FormPage, type FormTarget } from "./FormPage";
import { boxFrames, pollBoxFrames, watchBoxFrames } from "./layoutProbe";

interface Params {
  formCode?: string;
  year?: number;
  period?: number;
  mode?: FormTarget["mode"];
  /** Typed into the page once it opened (key -> text), as the filer would. */
  edits?: Record<string, string>;
  /** Ordered host edits and radio picks for a page-level regression that needs a populated return. */
  setup?: ({ pick: string } | { edits: Record<string, string> })[];
  /** A row a step lands on once the page opened ("part:item"), as the sections list's jumpTo does. */
  stepRow?: string;
  /** The parity world first (nsh/verify/parity/seed.test.tsx): tax year 2026 with 1601C and 2551Q, and four saved
   * drafts in this order: 2551Q Q3, 1601C Sep, 1601C Oct, 2551Q Q1 (each opened and saved, as the old app saved them). */
  seed?: boolean;
  /** Settings > Compact form sections on (the app pref compactFormNav). */
  compact?: boolean;
  /** Parts to open once the page opened (the rest stay folded, as a fresh return leaves them); "all" opens every part. */
  openParts?: number[] | "all";
  /** A form key whose host-held value the fixture republishes as `probe <value>`. */
  probeKey?: string;
  /** The TIN the page runs on (default the demo taxpayer's); tests pass a legacy 12-digit row through this. */
  tin?: string;
  /** The account's TIN list faked for the queue gate (D2); the calls are published as `tinscalls ...` lines. */
  tins?: FakeTinsKind;
  /** After this many account_tins reads the fake throws. */
  tinsErrorAfter?: number;
  /** After this many reads the fake's sign-in becomes owner B. */
  tinsOwnerFlipAfter?: number;
  /** The fake's account_tins answer is delayed this long (a slow read a test can act during). */
  tinsDelayMs?: number;
  /** Reopen the same return the moment the first account_tins(0) read starts (a guaranteed overlap, not a guessed one). */
  reopenOnTinsRead?: boolean;
  /** Open the filed original of the same form/period/tin the moment the first account_tins(0) read starts. */
  swapFiledOriginalOnTinsRead?: boolean;
  /** Open ANOTHER return (another period of the same form: 1601C does not open in the scratch world) the moment the first account_tins(0) read starts. */
  swapTargetOnTinsRead?: boolean;
  /** Pick this radio the moment the first account_tins(0) read starts (a choose-path edit during the read, guaranteed overlap). */
  pickOnTinsRead?: string;
  /** Type these edits the moment the first account_tins(0) read starts. */
  typeOnTinsRead?: Record<string, string>;
  /** The refusal activate_tin throws. */
  activate?: FakeActivateKind;
  /** Year facts set for the demo taxpayer before the page opens (e.g. the EOPT tier a queue needs answered). */
  facts?: Record<string, string>;
}

async function seedParity(tin: string) {
  if (!bir.list_tax_years(tin).includes(2026)) {
    bir.add_tax_year(tin, 2026, false);
    bir.set_yearly_forms(tin, 2026, ["1601C", "2551Q"]);
  }
  for (const [code, period] of [["2551Q", 3], ["1601C", 9], ["1601C", 10], ["2551Q", 1]] as const) {
    const opened = await bir.screen_form_open(tin, code, 2026, period);
    bir.screen_form_save(code, opened.draft);
  }
}

const toasts = signal<string[]>([]);
const sessionFields = signal("");
const probeValue = signal("");
let count = 0;

const tinsCalls = signal<string[]>([]);

function Fixture() {
  watchBoxFrames(true);
  createDemoTaxpayer();
  const p = (host.params || {}) as Params;
  if (p.tins) installFakeTins(bir as unknown as Record<string, unknown>, p.tins, p.activate, (line) => tinsCalls.set([...tinsCalls(), line]), { errorAfter: p.tinsErrorAfter, ownerFlipAfter: p.tinsOwnerFlipAfter, delayMs: p.tinsDelayMs });
  const tin = typeof p.tin === "string" ? p.tin : DEMO.tin;
  if (p.facts) {
    const year = p.year ?? 2025;
    if (!bir.list_tax_years(tin).includes(year)) bir.add_tax_year(tin, year, false);
    bir.set_year_facts(tin, year, p.facts as never);
  }
  let probeTimer: ReturnType<typeof setInterval> | undefined;
  cleanup(() => {
    if (probeTimer !== undefined) clearInterval(probeTimer);
  });
  if (p.compact) app.setPref("compactFormNav", true);
  const seeded = signal(!p.seed);
  if (p.seed) void seedParity(tin).then(() => seeded.set(true));
  return (
    <View class="flex-1 bg-background">
      <If when={seeded()}>
        <FormPage
          tin={tin}
          formCode={p.formCode ?? "2551Q"}
          year={p.year ?? 2025}
          period={p.period ?? 1}
          mode={p.mode ?? "return"}
          toast={(message) => {
            toasts.set([...toasts(), message]);
            return `fx-${++count}`;
          }}
          dismissToast={(id) => toasts.set([...toasts(), `dismissed ${id}`])}
          onSession={(s) => {
            if (p.reopenOnTinsRead || p.swapFiledOriginalOnTinsRead || p.swapTargetOnTinsRead || p.pickOnTinsRead || p.typeOnTinsRead) {
              let done = false;
              const timer = setInterval(() => {
                if (done) { clearInterval(timer); return; }
                if (tinsCalls().some((line) => line.startsWith("account_tins(0)"))) {
                  done = true;
                  clearInterval(timer);
                  if (p.pickOnTinsRead) { try { s.pickRadio(p.pickOnTinsRead); tinsCalls.set([...tinsCalls(), `pick-fired value=${s.valueOf(p.pickOnTinsRead) ?? "none"}`]); } catch (e) { tinsCalls.set([...tinsCalls(), `pick-error ${e}`]); } }
                  for (const [k, v] of Object.entries(p.typeOnTinsRead || {})) { s.edit(k, v); tinsCalls.set([...tinsCalls(), `edit-fired value=${s.valueOf(k) ?? "none"}`]); }
                  if (p.swapFiledOriginalOnTinsRead) {
                    void s.open({ tin, formCode: p.formCode ?? "2551Q", year: p.year ?? 2025, period: p.period ?? 1, mode: "filed_original", snapshotId: 1 } as never);
                  } else if (p.swapTargetOnTinsRead) {
                    void s.open({ tin, formCode: p.formCode ?? "2551Q", year: p.year ?? 2025, period: 2 } as never).then((err) => tinsCalls.set([...tinsCalls(), err ? `swap-target-error ${err}` : "swap-target-opened"]));
                  } else if (p.reopenOnTinsRead) {
                    void s.open({ tin, formCode: p.formCode ?? "2551Q", year: p.year ?? 2025, period: p.period ?? 1 } as never, true);
                  }
                }
              }, 100);
            }
            void s.ready().then(async () => {
              sessionFields.set(String(s.fieldCount()));
              const openAll = p.openParts === "all" ? s.schema!.parts.map((_x, i) => i) : p.openParts || [];
              for (const pi of openAll) if (!s.part(pi)().open) s.togglePart(pi);
              for (const step of p.setup || []) {
                if ("pick" in step) s.pickRadio(step.pick);
                else for (const [k, v] of Object.entries(step.edits)) s.edit(k, v);
                await s.idle();
              }
              for (const [k, v] of Object.entries(p.edits || {})) s.edit(k, v);
              if (p.stepRow) s.jumpTo(Number(p.stepRow.split(":")[0]), p.stepRow);
              probeTimer = setInterval(() => {
              if (p.probeKey) probeValue.set(s.valueOf(p.probeKey!) || "");
              pollBoxFrames();
            }, 100);
            })
          }}
        />
      </If>
      <View testID="fx-toasts" class="absolute top-0 left-0" style={{ pointerEvents: "none", opacity: 0 }}>
        <Text>{`saved returns ${store.savedReturns().length}`}</Text>
        <Text testID="fx-session-fields">{`session fields ${sessionFields()}`}</Text>
        <Text>{`probe ${probeValue()}`}</Text>
        <Text testID="fx-box-xs">{`boxx ${JSON.stringify(boxFrames())}`}</Text>
        <Each of={toasts()}>{(t) => <Text>{t()}</Text>}</Each>
        <Each of={tinsCalls()}>{(t) => <Text>{`tins ${t()}`}</Text>}</Each>
      </View>
    </View>
  );
}

mount(Fixture);
