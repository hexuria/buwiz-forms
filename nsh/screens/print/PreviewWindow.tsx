// The print preview window's content (track D3): a toolbar over the filled sheet. Opened by host.tsx in a second window
// (no sidebar, no frame chrome); the toolbar follows the theme, the sheet stays white (the page is the Rust side's static
// HTML, a file the web view loads with read access to its own folder). Nothing here prints: Print and Save as PDF
// ask the Rust side (native.ts), which makes one PDF from the same document for both.
import { colorScheme, effect, Image, Scroll, signal, token, View, WebView, type WebViewRef } from "nsh";
import { errorText } from "../../lib/errors";
import { AppButton, Line, Segmented, Spinner } from "../../ui";
import { type PrintActions, REAL_ACTIONS, type SheetImage } from "./native";
import { previews } from "./store";
import { baseName, chromeScript, pageSummary, type ZoomMode } from "./titles";

/** The desk around the sheets, in points. */
const DESK_PAD = 16;

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
  // Where the app draws the sheets itself they are pictures in a scroll view; elsewhere a web view shows the sheet's page.
  const draw = (props.actions ?? REAL_ACTIONS).sheets;
  const sheets = signal<SheetImage[]>([]);
  let asked = 0;
  let drawnRevision = -1;

  // The pictures of the newest document: asked again at every revision (the Rust side draws only the sheets that changed).
  // An answer that a newer question overtook is dropped.
  effect(() => {
    const revision = info()?.revision;
    // Once per revision: the store changes for other reasons too (the load count below).
    if (!draw || revision === undefined || revision === drawnRevision) return;
    drawnRevision = revision;
    const mine = ++asked;
    void draw(props.previewKey)
      .then((drawn) => {
        if (mine !== asked) return;
        const first = sheets().length === 0;
        sheets.set(drawn);
        if (first) previews.noteLoad(props.previewKey);
      })
      .catch((error) => {
        if (mine === asked) note.set({ tone: "danger", text: `The sheet did not load: ${errorText(error)}` });
      });
  });

  /** A sheet's size on screen: the window's width at "Fit width" (never wider than twice its own), its own at 100%. */
  const shown = (sheet: SheetImage) => {
    const fit = Math.max(120, Math.min(width() - 2 * DESK_PAD, sheet.width * 2));
    const w = zoom() === "fit" && width() > 0 ? fit : sheet.width;
    return { width: w, height: (w * sheet.height) / sheet.width };
  };

  // The Rust side sends a script for every change of the document: the page is never reloaded, so it keeps its scroll.
  // The newest script is also run when the page finishes loading (a change may have come while it loaded).
  effect(() => {
    const script = info()?.script;
    if (!draw && loaded() && script) web.injectJavaScript(script);
  });

  // The desk and the size of the sheets: again when the theme, the choice or the window's width changes.
  effect(() => {
    const mode = zoom();
    const desk = token("selected");
    const dark = colorScheme() === "dark";
    width();
    if (!draw && loaded()) web.injectJavaScript(chromeScript(mode, desk, dark));
  });

  const summary = () => {
    const now = info();
    return now ? pageSummary(now) : "";
  };

  // Each press is a ticket; Cancel takes a new one, so a press's answer that comes after its Cancel is dropped.
  let ticket = 0;
  const run = async (kind: "print" | "save") => {
    const key = props.previewKey;
    if (busy() !== "" || !info()) return;
    const actions = props.actions ?? REAL_ACTIONS;
    const mine = ++ticket;
    busy.set(kind);
    note.set(null);
    try {
      if (kind === "print") {
        await actions.print(key);
      } else {
        const path = await actions.savePdf(key);
        if (mine === ticket && path) note.set({ tone: "fg2", text: `Saved ${baseName(path)}` });
      }
    } catch (error) {
      if (mine === ticket) note.set({ tone: "danger", text: errorText(error) });
    } finally {
      if (mine === ticket) busy.set("");
    }
  };

  // The way out of a wait that never ends (a panel that never answers): it always works, the buttons work again, and the
  // app's log says it was used.
  const stopWaiting = () => {
    const kind = busy();
    if (kind === "") return;
    ticket++;
    busy.set("");
    note.set({ tone: "fg2", text: "Stopped waiting. Press the button again to try once more." });
    (props.actions ?? REAL_ACTIONS).stoppedWaiting?.(kind === "print" ? "Print" : "Save as PDF");
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
          <If when={busy() !== ""}>
            <AppButton testID="print-stop-waiting" label="Cancel" variant="ghost" size="sm" onPress={stopWaiting} />
          </If>
        </View>
        <AppButton testID="print-save-pdf" label="Save as PDF" icon="save" disabled={busy() !== ""} onPress={() => void run("save")} />
        <AppButton testID="print-print" label="Print" icon="printer" variant="primary" disabled={busy() !== ""} onPress={() => void run("print")} />
      </View>
      <View class="flex-1" onLayout={(e) => width.set(Math.round(e.bounds.size.width))}>
        <If when={draw !== undefined}>
          <Scroll testID="print-sheet" class="min-h-0 flex-1 bg-selected" contentClass="items-center" contentStyle={{ padding: DESK_PAD, gap: DESK_PAD }}>
            <Each of={sheets().map((sheet, at) => ({ ...sheet, page: at + 1 }))} key={(sheet) => sheet.page}>
              {(sheet) => (
                <Image
                  testID={`print-sheet-${sheet().page}`}
                  src={sheet().src}
                  style={{ ...shown(sheet()), backgroundColor: "#ffffff", boxShadow: "0 2px 10px rgba(0,0,0,0.25)" }}
                />
              )}
            </Each>
          </Scroll>
        </If>
        <If when={draw === undefined}>
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
        </If>
      </View>
    </View>
  );
}
