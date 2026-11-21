// The print preview host's decisions, with its world handed in (windows, the Rust side, the store, the frame), so a plain test plays
// every order of events. host.tsx wires the real ones and owns the effects; nothing here draws.
//
// One preview per form code on the Rust side (it keys by code), one window per RETURN here: the window's id carries the return it was
// asked on (form, TIN, year, period). A request for another return of the same form never lands in a window that is closing for the
// last one, and a window that was ended from this side never speaks for a newer one: its late close event finds another owner and
// does nothing (it used to tell Rust the preview was closed, which deleted the new preview's files).
import { UNAVAILABLE, type RouteLike, formIdentity, previewTitle, windowId } from "./titles";
import type { PrintEvent } from "./native";

export { UNAVAILABLE };

/** What a preview window is, as the host uses it. */
export interface WindowLike {
  close(): void;
  focus(): void;
  set(attributes: { title: string }): void;
  /** Hears the window close, whoever closed it. */
  on(event: "close", listener: () => void): void;
}

/** The world the host acts in. */
export interface HostDeps {
  /** This device can open a second window (a desktop). */
  available: boolean;
  /** System preview instead of a second window (Android); the document is a snapshot. */
  androidPreview?(key: string, title: string): Promise<void>;
  windows: {
    get(id: string): WindowLike | null;
    /** Opens the window of `id` showing the preview of `key`; throws when it cannot. */
    open(id: string, title: string, key: string): WindowLike;
    /** The first window opened: the main window closing must take the previews with it. */
    onFirstOpen?(): void;
  };
  /**
   * This device previews in the system's own print sheet (iOS) instead of a window: shows it for `key` (the Rust side makes the PDF
   * first) and settles once the sheet is up. Absent where previews are windows.
   */
  nativePreview?(key: string): Promise<void>;
  /** Takes down the system's print sheet if it is up (with `nativePreview`). */
  dismissNative?(): void;
  /** The Rust side: the window of `key` is gone (it deletes the files). */
  print: { closed(key: string): void };
  previews: { apply(event: PrintEvent, title?: string): void; get(key: string): unknown; remove(key: string): void };
  /** The route on screen now (not a tracked read). */
  route(): RouteLike;
  /** The period of the return on screen, as the sidebar says it ("Q1 2026"), or null. */
  period(): string | null;
  /** The frame is up: nothing (the Buwiz gate, first launch, the sign-in screen) stands in front of it. */
  chromeVisible(): boolean;
  toast(message: string, icon: string): void;
  /** A sentence for a failure, from what was thrown. */
  errorText(error: unknown): string;
}

/** A preview the host has a window for. */
interface Owner {
  /** The window's id. */
  id: string;
  /** The return it was asked on (null: not on a form page, so no page decides when it ends). */
  identity: string | null;
}

export function createPreviewHost(deps: HostDeps) {
  /** The preview of each form code, as the Rust side holds it. */
  const owners = new Map<string, Owner>();
  const androidPreviews = new Map<string, object>();
  /** The forms whose system print sheet is being prepared or is up (a phone). A lock takes them away. */
  const natives = new Set<string>();
  let hooked = false;

  /** The preview of `key` is over: the host forgets it and the Rust side deletes its files. */
  function gone(key: string): void {
    owners.delete(key);
    deps.previews.remove(key);
    deps.print.closed(key);
  }

  /** Closes the window of `key` from this side (a lock, leaving the return): the preview is over at once, not when the window says so. */
  function end(key: string): void {
    const owner = owners.get(key);
    if (!owner) return;
    gone(key);
    deps.windows.get(owner.id)?.close();
  }

  /**
   * A request for another return of a form that has a window: the old window closes, and the Rust side keeps the new preview (it
   * already holds the new document; telling it the old window closed would delete it).
   */
  function supersede(key: string): void {
    const owner = owners.get(key);
    if (!owner) return;
    owners.delete(key);
    deps.windows.get(owner.id)?.close();
  }

  /**
   * A phone that previews in the system's print sheet: the sheet comes up with the PDF, then the Rust side forgets the preview. A lock
   * while the PDF is made ends it: the Rust side forgets it (so it refuses to show the sheet), and a sheet that came up anyway is taken down.
   */
  function openNative(event: PrintEvent, show: (key: string) => Promise<void>): void {
    if (!deps.chromeVisible()) {
      deps.print.closed(event.key);
      return;
    }
    natives.add(event.key);
    deps.toast("Preparing the print preview...", "printer");
    const ended = () => !natives.has(event.key) || !deps.chromeVisible();
    show(event.key).then(
      () => {
        if (ended()) deps.dismissNative?.();
        deps.print.closed(event.key);
      },
      (error: unknown) => {
        const quiet = ended();
        natives.delete(event.key);
        deps.print.closed(event.key);
        if (!quiet) deps.toast(`Print preview could not open: ${deps.errorText(error)}`, "triangle-alert");
      },
    );
  }

  /** Every system print sheet ends: the ones being prepared never show, the one up is taken down. */
  function endNative(): void {
    if (!deps.nativePreview) return;
    for (const key of [...natives]) {
      natives.delete(key);
      deps.print.closed(key);
    }
    deps.dismissNative?.();
  }

  function open(event: PrintEvent): void {
    if (!deps.available && deps.nativePreview) return openNative(event, deps.nativePreview);
    if (deps.androidPreview) {
      if (!deps.chromeVisible()) {
        deps.print.closed(event.key);
        return;
      }
      const identity = formIdentity(deps.route());
      const title = `${previewTitle(event.title, deps.period())} ${identity?.split("|").slice(0, 4).join(" ") ?? event.key}`;
      const pending = {};
      androidPreviews.set(event.key, pending);
      deps.androidPreview(event.key, title).catch((error) => {
        if (androidPreviews.get(event.key) === pending && deps.chromeVisible()) {
          deps.toast(`Print preview could not open: ${deps.errorText(error)}`, "triangle-alert");
        }
      });
      return;
    }
    if (!deps.available) {
      deps.toast(UNAVAILABLE, "printer");
      deps.print.closed(event.key);
      return;
    }
    // Behind a lock nothing shows: no window opens for a request that came while the frame is not up.
    if (!deps.chromeVisible()) {
      deps.print.closed(event.key);
      return;
    }
    const identity = formIdentity(deps.route());
    const id = windowId(event.key, identity);
    if (owners.get(event.key)?.id !== id) supersede(event.key);
    const title = previewTitle(event.title, deps.period());
    deps.previews.apply(event, title);
    const kept = owners.get(event.key);
    const existing = kept ? deps.windows.get(id) : null;
    if (kept && existing) {
      kept.identity = identity;
      existing.set({ title });
      existing.focus();
      return;
    }
    if (!hooked) {
      hooked = true;
      deps.windows.onFirstOpen?.();
    }
    const owner: Owner = { id, identity };
    owners.set(event.key, owner);
    let win: WindowLike;
    try {
      win = deps.windows.open(id, title, event.key);
    } catch (error) {
      gone(event.key);
      deps.toast(`Print preview could not open: ${deps.errorText(error)}`, "triangle-alert");
      return;
    }
    // Only the window that is this form's preview now may end it.
    win.on("close", () => {
      if (owners.get(event.key) === owner) gone(event.key);
    });
    win.focus();
  }

  return {
    /** What the Rust side says. */
    hear(event: PrintEvent): void {
      if (event.kind === "unavailable") deps.toast(event.message ?? UNAVAILABLE, "printer");
      else if (event.kind === "open") open(event);
      else if (deps.previews.get(event.key)) deps.previews.apply(event);
    },
    /** Every preview ends: a lock, a sign-out, the main window closing. */
    closeAll(): void {
      for (const key of androidPreviews.keys()) deps.print.closed(key);
      androidPreviews.clear();
      for (const key of [...owners.keys()]) end(key);
      endNative();
    },
    /** The page on screen is now `here` (a return's identity, or null): the previews of other returns end. */
    leftReturn(here: string | null): void {
      for (const [key, owner] of [...owners]) if (owner.identity !== null && owner.identity !== here) end(key);
    },
    /** For a test: the window id each form code's preview has. */
    windowOf(key: string): string | null {
      return owners.get(key)?.id ?? null;
    },
  };
}
