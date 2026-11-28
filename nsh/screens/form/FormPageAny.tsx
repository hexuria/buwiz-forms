// A test fixture, not a screen: the form page for the demo taxpayer, the form named by the test
// (open(..., { host: { params: { formCode, year, period, mode } } })). Toasts are listed as texts (testID fx-toasts), and the
// ids of dismissed ones too, so a test reads what the page said.
import { cleanup, host, mount, signal, Text, View } from "nsh";
import { bir } from "../../bir";
import "../../theme/fonts";
import { createDemoTaxpayer, DEMO } from "../../bir/testing";
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
  setup?: ({ pick: string } | { edits: Record<string, string> } | { confirmRemoval: true })[];
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

function Fixture() {
  watchBoxFrames(true);
  createDemoTaxpayer();
  const p = (host.params || {}) as Params;
  let probeTimer: ReturnType<typeof setInterval> | undefined;
  cleanup(() => {
    if (probeTimer !== undefined) clearInterval(probeTimer);
  });
  if (p.compact) app.setPref("compactFormNav", true);
  const seeded = signal(!p.seed);
  if (p.seed) void seedParity(DEMO.tin).then(() => seeded.set(true));
  return (
    <View class="flex-1 bg-background">
      <If when={seeded()}>
        <FormPage
          tin={DEMO.tin}
          formCode={p.formCode ?? "2551Q"}
          year={p.year ?? 2025}
          period={p.period ?? 1}
          mode={p.mode ?? "return"}
          toast={(message) => {
            toasts.set([...toasts(), message]);
            return `fx-${++count}`;
          }}
          dismissToast={(id) => toasts.set([...toasts(), `dismissed ${id}`])}
          onSession={(s) =>
            void s.ready().then(async () => {
              sessionFields.set(String(s.fieldCount()));
              const openAll = p.openParts === "all" ? s.schema!.parts.map((_x, i) => i) : p.openParts || [];
              for (const pi of openAll) if (!s.part(pi)().open) s.togglePart(pi);
              for (const step of p.setup || []) {
                if ("pick" in step) s.pickRadio(step.pick);
                else if ("confirmRemoval" in step) s.confirmRemoval();
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
          }
        />
      </If>
      <View testID="fx-toasts" class="absolute top-0 left-0" style={{ pointerEvents: "none", opacity: 0 }}>
        <Text>{`saved returns ${store.savedReturns().length}`}</Text>
        <Text testID="fx-session-fields">{`session fields ${sessionFields()}`}</Text>
        <Text>{`probe ${probeValue()}`}</Text>
        <Text testID="fx-box-xs">{`boxx ${JSON.stringify(boxFrames())}`}</Text>
        <Each of={toasts()}>{(t) => <Text>{t()}</Text>}</Each>
      </View>
    </View>
  );
}

mount(Fixture);
