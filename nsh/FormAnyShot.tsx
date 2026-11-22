// D8's fixture (not a screen): the form page for the demo taxpayer, the form named by the test
// (shots/forms43.test.tsx: open("../FormAnyShot.tsx", { host: { params: { formCode, year, period, mode } } })). It lives at the
// nsh root, as FormShot.tsx does: a real-window run serves assets (fonts, icons) only from the entry's folder down (nsh/AGENTS.md
// pitfall 14), so FormPageAny.tsx in screens/form/ cannot be shot. Like FormShot: the frame's surface (bg-card) and a Toaster
// above the bottom bar (bottom 96).
import { host, mount, View } from "nsh";
import "./theme/fonts";
import { createDemoTaxpayer, DEMO } from "./bir/testing";
import { FormPage, type FormTarget } from "./screens/form/FormPage";
import { Toaster } from "./ui";

interface Params {
  formCode?: string;
  year?: number;
  period?: number;
  mode?: FormTarget["mode"];
  /** Parts to open once the page opened; "all" opens every part (the rest stay folded, as a fresh return leaves them). */
  openParts?: number[] | "all";
  /** Edits applied once the page opened (a refused write on a gated box keeps it drawn with its sentence). */
  edits?: Record<string, string>;
}

function Preview() {
  createDemoTaxpayer();
  const p = (host.params || {}) as Params;
  return (
    <View class="flex-1 bg-card">
      <FormPage
        tin={DEMO.tin}
        formCode={p.formCode ?? "2551Q"}
        year={p.year ?? 2025}
        period={p.period ?? 1}
        mode={p.mode ?? "return"}
        onSession={(s) =>
          void s.ready().then(() => {
            const openAll = p.openParts === "all" ? s.schema!.parts.map((_x, i) => i) : p.openParts || [];
            for (const pi of openAll) if (!s.part(pi)().open) s.togglePart(pi);
            for (const [k, v] of Object.entries(p.edits || {})) s.edit(k, v);
          })
        }
      />
      <Toaster bottom={96} />
    </View>
  );
}

mount(Preview);
