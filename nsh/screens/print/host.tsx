// The print preview's host (track D3): imported once by App.tsx, it listens to the Rust side (src/print.rs) and opens ONE
// preview window per form code in a second window (nsh/desktop). The window's content is PreviewWindow.tsx; what it
// shows comes from store.ts. Nothing here draws.
//
//   a sheet was asked for     -> open the window, or focus the one open (its title and sheet refreshed in place)
//   a newer document          -> the store gets its script; the open page runs it (no reload: the scroll stays)
//   no window on this device  -> a toast says so (a phone: no second window exists there)
//
// A preview belongs to the return it was opened on: leaving that form, or opening another return, closes the window,
// and so does closing the main window. The user closing the window tells the Rust side, which forgets the preview.
import { effect, untracked } from "nsh";
import { displays, isAvailable, Window, type WindowRef } from "nsh/desktop";
import { errorText } from "../../lib/errors";
import { frequencyOf, type Frequency, periodLabel } from "../../lib/periods";
import { app, store, toast } from "../../shell";
import type { FormTarget } from "../form/FormPage";
import { Print, type PrintEvent } from "./native";
import { PreviewWindow } from "./PreviewWindow";
import { previews } from "./store";
import { formIdentity, previewTitle, windowId } from "./titles";

/** The return each open preview was asked on (null: not on a form page, so no page decides when it ends). */
const owners = new Map<string, string | null>();

let hooked = false;

/** The period of the return on screen, as the sidebar says it ("Q1 2026"); null for a Form library sample or no form page. */
function periodNow(): string | null {
  const route = untracked(() => app.route());
  const target = route.view === "form" ? (route.params.target as FormTarget | undefined) : undefined;
  if (!target || target.mode === "sample") return null;
  const index = untracked(() => store.screenIndex()) as { c: string; freq: Frequency }[];
  return periodLabel(frequencyOf(index, target.formCode), target.year, target.period, true);
}

/** A window that fits the display: the sheet is 794 wide, a page of it about 1100 tall. */
function startSize(): { width: number; height: number } {
  const area = untracked(() => displays())[0]?.visibleBounds;
  return { width: 940, height: Math.max(520, Math.min(1100, area ? area.size.height - 60 : 900)) };
}

function gone(key: string) {
  owners.delete(key);
  previews.remove(key);
  Print.closed(key);
}

function closeAll() {
  for (const key of [...owners.keys()]) Window.get(windowId(key))?.close();
}

/** What a device with no second window says (a phone). */
export const UNAVAILABLE = "Print preview is not available on this device.";

function open(event: PrintEvent) {
  if (!isAvailable) {
    toast(UNAVAILABLE, "printer");
    Print.closed(event.key);
    return;
  }
  const title = previewTitle(event.title, periodNow());
  previews.apply(event, title);
  const id = windowId(event.key);
  const existing = Window.get(id);
  if (existing) {
    existing.set({ title });
    existing.focus();
    return;
  }
  // Quitting closes every window (Application.quit); the main window really closing (no tray to hide it to) must take
  // the previews with it, or they would keep the app alive.
  if (!hooked) {
    hooked = true;
    Window.get("main")?.on("close", closeAll);
  }
  owners.set(event.key, formIdentity(untracked(() => app.route())));
  let win: WindowRef;
  try {
    win = Window.open(() => <PreviewWindow previewKey={event.key} />, {
      id,
      kind: "normal",
      title,
      defaultSize: startSize(),
      minSize: { width: 560, height: 420 },
    });
  } catch (error) {
    gone(event.key);
    toast(`Print preview could not open: ${errorText(error)}`, "triangle-alert");
    return;
  }
  win.on("close", () => gone(event.key));
  win.focus();
}

Print.listen((event) => {
  if (event.kind === "unavailable") toast(event.message ?? UNAVAILABLE, "printer");
  else if (event.kind === "open") open(event);
  else if (previews.get(event.key)) previews.apply(event);
});

// Leaving the return a preview was asked on ends it.
effect(() => {
  const here = formIdentity(app.route());
  untracked(() => {
    for (const [key, owner] of [...owners]) if (owner !== null && owner !== here) Window.get(windowId(key))?.close();
  });
});
