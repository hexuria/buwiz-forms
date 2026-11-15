// A test fixture, not a screen: the form page for the demo taxpayer, the form named by the test
// (open(..., { host: { params: { formCode, year, period, mode } } })). Toasts are listed as texts (testID fx-toasts), and the
// ids of dismissed ones too, so a test reads what the page said.
import { host, mount, signal, Text, View } from "nsh";
import "../../theme/fonts";
import { createDemoTaxpayer, DEMO } from "../../bir/testing";
import { store } from "../../shell";
import { FormPage, type FormTarget } from "./FormPage";

interface Params {
  formCode?: string;
  year?: number;
  period?: number;
  mode?: FormTarget["mode"];
  /** Typed into the page once it opened (key -> text), as the filer would. */
  edits?: Record<string, string>;
}

const toasts = signal<string[]>([]);
let count = 0;

function Fixture() {
  createDemoTaxpayer();
  const p = (host.params || {}) as Params;
  return (
    <View class="flex-1 bg-background">
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
      <View testID="fx-toasts" class="absolute top-0 left-0" style={{ pointerEvents: "none", opacity: 0 }}>
        <Text>{`saved returns ${store.savedReturns().length}`}</Text>
        <Each of={toasts()}>{(t) => <Text>{t()}</Text>}</Each>
      </View>
    </View>
  );
}

mount(Fixture);
