// The print preview window's content (track D3): a toolbar over the filled sheet. Opened by host.tsx in a second window
// (no sidebar, no frame chrome); the toolbar follows the theme, the sheet stays white (the page is the Rust side's static
// HTML, a file the web view loads with read access to its own folder). Nothing here prints: Print and Save as PDF
// ask the Rust side (native.ts), which makes one PDF from the same document for both.
import { colorScheme, effect, signal, token, View, WebView, type WebViewRef } from "nsh";
import { errorText } from "../../lib/errors";
import { AppButton, Line, Segmented, Spinner } from "../../ui";
import { type PrintActions, REAL_ACTIONS } from "./native";
import { previews } from "./store";
import { baseName, chromeScript, pageSummary, type ZoomMode } from "./titles";

/** What the toolbar last said about a button's work. */
interface Note {
  tone: "fg2" | "danger";
  text: string;
}

/** The window of the preview of `previewKey` (a form code). `actions` are the buttons' work: a test passes its own. */
export function PreviewWindow(props: { previewKey: string; actions?: PrintActions }) {
  let web!: WebViewRef;
  const info = () => previews.get(props.previewKey);
  const loaded = signal(false);
  const width = signal(0);
  const zoom = signal<ZoomMode>("fit");
  const busy = signal<"" | "print" | "save">("");
  const note = signal<Note | null>(null);

  // The Rust side sends a script for every change of the document: the page is never reloaded, so it keeps its scroll.
  // The newest script is also run when the page finishes loading (a change may have come while it loaded).
  effect(() => {
    const script = info()?.script;
    if (loaded() && script) web.injectJavaScript(script);
  });

  // The desk and the size of the sheets: again when the theme, the choice or the window's width changes.
  effect(() => {
    const mode = zoom();
    const desk = token("selected");
    const dark = colorScheme() === "dark";
    width();
    if (loaded()) web.injectJavaScript(chromeScript(mode, desk, dark));
  });

  const summary = () => {
    const now = info();
    return now ? pageSummary(now) : "";
  };

  const run = async (kind: "print" | "save") => {
    const key = props.previewKey;
    if (busy() !== "" || !info()) return;
    const actions = props.actions ?? REAL_ACTIONS;
    busy.set(kind);
    note.set(null);
    try {
      if (kind === "print") {
        await actions.print(key);
      } else {
        const path = await actions.savePdf(key);
        if (path) note.set({ tone: "fg2", text: `Saved ${baseName(path)}` });
      }
    } catch (error) {
      note.set({ tone: "danger", text: errorText(error) });
    } finally {
      busy.set("");
    }
  };

  return (
    <View testID="print-window" class="flex-1 bg-background">
      <View class="h-12 shrink-0 flex-row items-center gap-3 border-b border-border bg-card px-3">
        <View>
          <Segmented
            testID="print-zoom"
            label="Zoom"
            value={zoom()}
            onChange={(value) => zoom.set(value as ZoomMode)}
            options={[
              { value: "fit", label: "Fit width" },
              { value: "100", label: "100%" },
            ]}
          />
        </View>
        <View class="min-w-0 flex-1 flex-row items-center gap-2">
          <Line testID="print-summary" variant="meta" tone="fg2">
            {summary()}
          </Line>
          <If when={busy() !== ""}>
            <Spinner size={14} label="Preparing the PDF" />
          </If>
          <Line testID="print-note" variant="meta" tone={note()?.tone ?? "fg2"}>
            {busy() !== "" ? "Preparing the PDF…" : (note()?.text ?? "")}
          </Line>
        </View>
        <AppButton testID="print-save-pdf" label="Save as PDF" icon="save" disabled={busy() !== ""} onPress={() => void run("save")} />
        <AppButton testID="print-print" label="Print" icon="printer" variant="primary" disabled={busy() !== ""} onPress={() => void run("print")} />
      </View>
      <View class="flex-1" onLayout={(e) => width.set(Math.round(e.bounds.size.width))}>
        <WebView
          ref={web}
          testID="print-sheet"
          src={info()?.url ?? ""}
          scrollable
          style={{ flex: 1 }}
          onLoadStart={() => loaded.set(false)}
          onLoad={() => {
            previews.noteLoad(props.previewKey);
            loaded.set(true);
          }}
          onError={(e) => note.set({ tone: "danger", text: `The sheet did not load: ${e.message}` })}
        />
      </View>
    </View>
  );
}
