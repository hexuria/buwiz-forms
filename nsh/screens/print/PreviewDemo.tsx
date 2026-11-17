// A test fixture, not a screen: the preview window's content (PreviewWindow) in the main window, over a made-up preview of
// the demo form, with buttons that only record what they were asked (the real ones make a PDF and open a panel).
// `open(entry, { host: { params } })`: `mode` "ok" (default), "fail" (every button fails) or "cancel" (Save as PDF is cancelled).
// The recorded calls are the text under the window (testID pd-log).
import { host, mount, signal, Text, View } from "nsh";
import "../../theme/fonts";
import type { PrintActions } from "./native";
import { PreviewWindow } from "./PreviewWindow";
import { previews } from "./store";

const calls = signal<string[]>([]);

const wait = (ms: number) => new Promise<void>((done) => setTimeout(done, ms));

function Demo() {
  const mode = ((host.params ?? {}) as { mode?: string }).mode ?? "ok";
  previews.apply({
    kind: "open",
    key: "2551Q",
    title: "BIR Form 2551Q — Juana Santos",
    url: "",
    script: "",
    revision: 1,
    pages: 4,
    paper: "A4",
    sheet: "Folio",
    zoom: 0.8782,
    message: null,
  });
  const actions: PrintActions = {
    async print(key) {
      calls.update((all) => [...all, `print ${key}`]);
      await wait(250);
      if (mode === "fail") throw new Error("No printer is available.");
    },
    async savePdf(key) {
      calls.update((all) => [...all, `save ${key}`]);
      await wait(250);
      if (mode === "fail") throw new Error("Could not save the PDF: the disk is full.");
      return mode === "cancel" ? null : "/Users/someone/Documents/BIR Form 2551Q - Juana Santos.pdf";
    },
  };
  return (
    <View class="flex-1">
      <PreviewWindow previewKey="2551Q" actions={actions} />
      <Text testID="pd-log">{`calls: ${calls().join(", ") || "none"}`}</Text>
    </View>
  );
}

mount(Demo);
