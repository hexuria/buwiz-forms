// A test fixture, not a screen: the form page for the demo taxpayer, the form named by the test
// (open(..., { host: { params: { formCode, year, period, mode } } })). Toasts are listed as texts (testID fx-toasts), and the
// ids of dismissed ones too, so a test reads what the page said.
import { host, mount, signal, Text, View } from "nsh";
import { bir } from "../../bir";
import "../../theme/fonts";
import { createDemoTaxpayer, DEMO } from "../../bir/testing";
import { app, store } from "../../shell";
import { FormPage, type FormTarget } from "./FormPage";

interface Params {
  formCode?: string;
  year?: number;
  period?: number;
  mode?: FormTarget["mode"];
  /** Typed into the page once it opened (key -> text), as the filer would. */
  edits?: Record<string, string>;
  /** The parity world first (nsh/verify/parity/seed.test.tsx): tax year 2026 with 1601C and 2551Q, and four saved
   * drafts in this order: 2551Q Q3, 1601C Sep, 1601C Oct, 2551Q Q1 (each opened and saved, as the old app saved them). */
  seed?: boolean;
  /** Settings > Compact form sections on (the app pref compactFormNav). */
  compact?: boolean;
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
let count = 0;

function Fixture() {
  createDemoTaxpayer();
  const p = (host.params || {}) as Params;
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
            void s.ready().then(() => {
              for (const [k, v] of Object.entries(p.edits || {})) s.edit(k, v);
            })
          }
        />
      </If>
      <View testID="fx-toasts" class="absolute top-0 left-0" style={{ pointerEvents: "none", opacity: 0 }}>
        <Text>{`saved returns ${store.savedReturns().length}`}</Text>
        <Each of={toasts()}>{(t) => <Text>{t()}</Text>}</Each>
      </View>
    </View>
  );
}

mount(Fixture);
