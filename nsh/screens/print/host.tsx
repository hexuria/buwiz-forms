// The print preview's host (track D3): imported once by App.tsx, it listens to the Rust side (src/print.rs) and opens ONE
// preview window per return in a second window (nsh/desktop). The window's content is PreviewWindow.tsx; what it shows comes from
// store.ts; what the host decides (which window, when one ends) is controller.ts. Nothing here draws.
//
//   a sheet was asked for     -> open the window, or focus the one open for that return (its title and sheet refreshed in place)
//   a newer document          -> the store gets its script; the open page runs it (no reload: the scroll stays)
//   no window on this device  -> a toast says so (a phone: no second window exists there)
//
// A preview belongs to the return it was opened on: leaving that form, or opening another return, closes the window, and so does closing
// the main window. So do a lock, a log out and a sign-out (the session changes), and whatever else takes the frame away (the Buwiz gate, the
// sign-in screen): a preview shows a taxpayer's TIN and amounts, and Print and Save as PDF still work in it. The user closing the window
// tells the Rust side, which forgets the preview.
import { effect, untracked } from "nsh";
import { displays, isAvailable, Window } from "nsh/desktop";
import { errorText } from "../../lib/errors";
import { frequencyOf, type Frequency, periodLabel } from "../../lib/periods";
import { app, store, toast } from "../../shell";
import type { FormTarget } from "../form/FormPage";
import { createPreviewHost } from "./controller";
import { Print } from "./native";
import { PreviewWindow } from "./PreviewWindow";
import { previews } from "./store";
import { formIdentity } from "./titles";

export { UNAVAILABLE } from "./titles";

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

const host = createPreviewHost({
  available: isAvailable,
  windows: {
    get: (id) => Window.get(id),
    open: (id, title, key) =>
      Window.open(() => <PreviewWindow previewKey={key} />, {
        id,
        kind: "normal",
        title,
        defaultSize: startSize(),
        minSize: { width: 560, height: 420 },
      }),
    // Quitting closes every window (Application.quit); the main window really closing (no tray to hide it to) must take the previews
    // with it, or they would keep the app alive.
    onFirstOpen: () => void Window.get("main")?.on("close", () => host.closeAll()),
  },
  print: { closed: (key) => Print.closed(key) },
  previews,
  route: () => untracked(() => app.route()),
  period: periodNow,
  chromeVisible: () => untracked(() => app.chromeVisible()),
  toast: (message, icon) => void toast(message, icon),
  errorText,
});

Print.listen(host.hear);

// Leaving the return a preview was asked on ends it.
effect(() => {
  const here = formIdentity(app.route());
  untracked(() => host.leftReturn(here));
});

// Nothing of a taxpayer stays up behind a lock: the frame going away (the account gate, the sign-in screen, a lock or a log out) ends every
// preview, and so does any change of session (another taxpayer signing in, the account gate opening again).
effect(() => {
  if (!app.chromeVisible()) untracked(() => host.closeAll());
});
app.onSessionChange(() => host.closeAll());
