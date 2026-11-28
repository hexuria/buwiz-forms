// F8-1 and F8-10: which window the print preview host opens, reuses and closes, with the windows, the Rust side and the frame faked so
// that each order of events can be played, the late close event of a window the host itself closed included.
import { expect, test } from "nsh/test";
import { createPreviewHost, type WindowLike } from "./controller";
import type { PrintEvent } from "./native";
import { windowId } from "./titles";

const TIN = "00012345300000";
const returnOf = (period: number) => ({ view: "form", params: { target: { tin: TIN, formCode: "2551Q", year: 2026, period, mode: "return" } } });
const identityOf = (period: number) => `2551Q|${TIN}|2026|${period}|return`;

/** A window the platform closes in its own time: close() asks, finish() is the close event. */
class FakeWindow implements WindowLike {
  title: string;
  focused = 0;
  closeAsked = false;
  private listeners: (() => void)[] = [];
  constructor(readonly id: string, title: string, private readonly registry: Map<string, FakeWindow>) {
    this.title = title;
  }
  close() {
    this.closeAsked = true;
  }
  focus() {
    this.focused++;
  }
  set(attributes: { title: string }) {
    this.title = attributes.title;
  }
  on(_event: "close", listener: () => void) {
    this.listeners.push(listener);
  }
  /** The close event arrives (after the window was asked, or because the person pressed its button). */
  finish() {
    this.registry.delete(this.id);
    for (const listener of this.listeners) listener();
  }
}

function world(nativePreview?: (key: string, title: string, requestId: string) => Promise<void>) {
  const windows = new Map<string, FakeWindow>();
  const opened: FakeWindow[] = [];
  const closedKeys: string[] = [];
  const toasts: string[] = [];
  const previews = new Map<string, { title: string; revision: number }>();
  const state = { route: returnOf(3) as { view: string; params: Record<string, unknown> }, chrome: true, refuseOpen: false, mainHooked: 0 };
  const host = createPreviewHost({
    available: !nativePreview,
    androidPreview: nativePreview,
    windows: {
      get: (id) => windows.get(id) ?? null,
      open: (id, title) => {
        if (state.refuseOpen) throw new Error("no window");
        const win = new FakeWindow(id, title, windows);
        windows.set(id, win);
        opened.push(win);
        return win;
      },
      onFirstOpen: () => void state.mainHooked++,
    },
    print: { closed: (key) => void closedKeys.push(key) },
    previews: {
      apply: (event, title) => void previews.set(event.key, { title: title ?? event.title, revision: event.revision }),
      get: (key) => previews.get(key),
      remove: (key) => void previews.delete(key),
    },
    route: () => state.route,
    period: () => "Q3 2026",
    chromeVisible: () => state.chrome,
    toast: (message) => void toasts.push(message),
    errorText: (error) => String((error as Error).message ?? error),
  });
  const event = (kind: "open" | "update", key: string, revision: number): PrintEvent =>
    ({ kind, key, requestId: `${key}-${revision}`, title: `BIR Form ${key} — Juana Santos`, url: "file:///preview/sheet.html", script: kind === "update" ? "patch" : "", revision, pages: 2, paper: "A4", sheet: "Folio", zoom: 0.88, message: null }) as PrintEvent;
  return { host, windows, opened, closedKeys, toasts, previews, state, event };
}

test("Android system preview needs no second window and does not follow updates", async () => {
  const calls: string[] = [];
  const w = world(async (key, title) => { calls.push(`${key}: ${title}`); });
  w.host.hear(w.event("open", "2551Q", 1));
  w.host.hear(w.event("update", "2551Q", 2));
  await Promise.resolve();
  expect(calls.length).toBe(1);
  expect(calls[0]).toBe("2551Q: BIR Form 2551Q · Q3 2026");
  expect(calls[0].includes(TIN)).toBe(false);
  expect(calls[0].includes("Juana Santos")).toBe(false);
  expect(w.opened.length).toBe(0);
  expect(w.closedKeys.length).toBe(0);
  expect(w.toasts.length).toBe(0);
});

test("Android system preview refuses requests behind the lock", () => {
  let calls = 0;
  const w = world(async () => { calls++; });
  w.state.chrome = false;
  w.host.hear(w.event("open", "2551Q", 1));
  expect(calls).toBe(0);
  expect(w.closedKeys).toEqual(["2551Q"]);
});

test("closing all previews cancels an Android job after native opening has settled", async () => {
  const w = world(async () => {});
  w.host.closeAll();
  expect(w.closedKeys).toEqual([]);
  w.host.hear(w.event("open", "2551Q", 1));
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  w.state.chrome = false;
  w.host.closeAll();
  expect(w.closedKeys).toEqual(["2551Q"]);
  w.host.closeAll();
  expect(w.closedKeys).toEqual(["2551Q"]);
  expect(w.toasts).toEqual([]);
});

test("closing all previews cancels an Android preview that is still loading", async () => {
  let reject!: (error: Error) => void;
  const w = world(() => new Promise<void>((_, no) => { reject = no; }));
  w.host.hear(w.event("open", "2551Q", 1));
  w.state.chrome = false;
  w.host.closeAll();
  expect(w.closedKeys).toEqual(["2551Q"]);
  reject(new Error("Android print preview was superseded or closed."));
  await Promise.resolve();
  expect(w.toasts).toEqual([]);
});

test("Android system preview reports a native failure without deleting its sheet", async () => {
  const w = world(async () => { throw new Error("Activity lookup blocked"); });
  w.host.hear(w.event("open", "2551Q", 1));
  await Promise.resolve();
  expect(w.toasts).toEqual(["Print preview could not open: Activity lookup blocked"]);
  expect(w.closedKeys.length).toBe(0);
  w.state.chrome = false;
  w.host.closeAll();
  expect(w.closedKeys).toEqual(["2551Q"]);
});

test("Android: session changes preserve the new same-form owner and pass distinct native request identities", async () => {
  const pending: { requestId: string; reject(error: Error): void }[] = [];
  const w = world((_key, _title, requestId) => new Promise<void>((_resolve, reject) => pending.push({ requestId, reject })));
  w.host.hear(w.event("open", "2551Q", 1));
  w.host.closeAll();
  w.host.hear(w.event("open", "2551Q", 2));
  expect(pending[0].requestId === pending[1].requestId).toBe(false);
  pending[0].reject(new Error("Old session's print failed"));
  await Promise.resolve();
  expect(w.toasts).toEqual([]);
  expect(w.closedKeys).toEqual(["2551Q"]);
  pending[1].reject(new Error("New print failed"));
  await Promise.resolve();
  expect(w.toasts).toEqual(["Print preview could not open: New print failed"]);
  w.host.closeAll();
  expect(w.closedKeys).toEqual(["2551Q", "2551Q"]);
});

test("the window belongs to the return it was asked on, and asking again for that return focuses it", () => {
  const w = world();
  w.host.hear(w.event("open", "2551Q", 1));
  expect(w.opened.length).toBe(1);
  expect(w.opened[0].id).toBe(windowId("2551Q", identityOf(3)));
  expect(w.opened[0].title).toBe("BIR Form 2551Q · Q3 2026");
  expect(w.opened[0].title.includes("Juana Santos")).toBe(false);
  expect(w.opened[0].title.includes(TIN)).toBe(false);
  expect(w.host.windowOf("2551Q")).toBe(w.opened[0].id);
  expect(w.state.mainHooked).toBe(1);
  // Asked again: the same window, its title refreshed, in front.
  const focused = w.opened[0].focused;
  w.host.hear(w.event("open", "2551Q", 2));
  expect(w.opened.length).toBe(1);
  expect(w.opened[0].focused).toBe(focused + 1);
  expect(w.previews.get("2551Q")?.revision).toBe(2);
  // A newer document reaches the preview; one for a form with no preview is ignored.
  w.host.hear(w.event("update", "2551Q", 3));
  expect(w.previews.get("2551Q")?.revision).toBe(3);
  w.host.hear(w.event("update", "1601C", 1));
  expect(w.previews.has("1601C")).toBe(false);
  expect(w.closedKeys).toEqual([]);
});

test("F8-10: a quick switch to another return of the same form opens its own window; the old window's late close event deletes nothing of the new one", () => {
  const w = world();
  w.host.hear(w.event("open", "2551Q", 1));
  const first = w.opened[0];
  // The person moves to another return of the form: the frame leaves the first one, so its preview ends (Rust forgets it now)...
  w.state.route = returnOf(2);
  w.host.leftReturn(identityOf(2));
  expect(first.closeAsked).toBe(true);
  expect(w.closedKeys).toEqual(["2551Q"]);
  expect(w.windows.has(first.id)).toBe(true); // ...but the platform has not closed the window yet
  // The new return's preview is asked for at once: it gets a window of its own, not the closing one.
  w.host.hear(w.event("open", "2551Q", 1));
  expect(w.opened.length).toBe(2);
  const second = w.opened[1];
  expect(second.id).not.toBe(first.id);
  expect(second.id).toBe(windowId("2551Q", identityOf(2)));
  expect(w.host.windowOf("2551Q")).toBe(second.id);
  // The old window's close event arrives late: it speaks for nobody.
  first.finish();
  expect(w.closedKeys).toEqual(["2551Q"]);
  expect(w.previews.has("2551Q")).toBe(true);
  expect(w.host.windowOf("2551Q")).toBe(second.id);
  expect(second.closeAsked).toBe(false);
  // The new preview still follows edits, and ends when its own window closes.
  w.host.hear(w.event("update", "2551Q", 2));
  expect(w.previews.get("2551Q")?.revision).toBe(2);
  second.finish();
  expect(w.closedKeys).toEqual(["2551Q", "2551Q"]);
  expect(w.previews.has("2551Q")).toBe(false);
  expect(w.host.windowOf("2551Q")).toBeNull();
});

test("F8-10: a request for another return while the old window is still up closes the old one and keeps the new preview on the Rust side", () => {
  const w = world();
  w.host.hear(w.event("open", "2551Q", 1));
  const first = w.opened[0];
  // No move was seen (the page changed under the same screen): the request itself says it is another return.
  w.state.route = returnOf(2);
  w.host.hear(w.event("open", "2551Q", 1));
  expect(first.closeAsked).toBe(true);
  expect(w.opened.length).toBe(2);
  expect(w.opened[1].id).toBe(windowId("2551Q", identityOf(2)));
  // Rust already holds the new document: it is never told the old window closed.
  expect(w.closedKeys).toEqual([]);
  first.finish();
  expect(w.closedKeys).toEqual([]);
  expect(w.previews.has("2551Q")).toBe(true);
  expect(w.host.windowOf("2551Q")).toBe(w.opened[1].id);
});

test("F8-1: a lock ends every preview at once, and nothing opens while the frame is not up", () => {
  const w = world();
  w.host.hear(w.event("open", "2551Q", 1));
  w.host.hear(w.event("open", "1601C", 1));
  expect(w.opened.length).toBe(2);
  w.host.closeAll();
  expect(w.opened.every((win) => win.closeAsked)).toBe(true);
  expect(w.closedKeys.sort()).toEqual(["1601C", "2551Q"]);
  expect(w.previews.size).toBe(0);
  expect(w.host.windowOf("2551Q")).toBeNull();
  // The windows close later: nothing more is said to Rust.
  for (const win of w.opened) win.finish();
  expect(w.closedKeys.length).toBe(2);
  // The sign-in screen (or the account gate) is in front: a request that arrives then opens no window.
  w.state.chrome = false;
  w.host.hear(w.event("open", "2551Q", 1));
  expect(w.opened.length).toBe(2);
  expect(w.closedKeys.length).toBe(3);
  expect(w.previews.size).toBe(0);
});

test("leaving the return ends its preview; a preview asked off a form page waits for no page; the person closing the window ends it", () => {
  const w = world();
  w.state.route = { view: "home", params: {} };
  w.host.hear(w.event("open", "2551Q", 1));
  expect(w.host.windowOf("2551Q")).toBe("print:2551Q");
  w.host.leftReturn(null);
  w.host.leftReturn(identityOf(3));
  expect(w.opened[0].closeAsked).toBe(false);
  // The person's own close.
  w.opened[0].finish();
  expect(w.closedKeys).toEqual(["2551Q"]);
  expect(w.previews.has("2551Q")).toBe(false);
});

test("a window that cannot open says so and leaves nothing behind; a device with no second window says so", () => {
  const w = world();
  w.state.refuseOpen = true;
  w.host.hear(w.event("open", "2551Q", 1));
  expect(w.toasts).toEqual(["Print preview could not open: no window"]);
  expect(w.closedKeys).toEqual(["2551Q"]);
  expect(w.host.windowOf("2551Q")).toBeNull();
  const phone = createPreviewHost({
    available: false,
    windows: { get: () => null, open: () => { throw new Error("none"); } },
    print: { closed: (key) => void w.closedKeys.push(key) },
    previews: { apply: () => {}, get: () => undefined, remove: () => {} },
    route: () => w.state.route,
    period: () => null,
    chromeVisible: () => true,
    toast: (message) => void w.toasts.push(message),
    errorText: String,
  });
  phone.hear(w.event("open", "1601C", 1));
  expect(w.toasts[w.toasts.length - 1]).toBe("Print preview is not available on this device.");
  expect(w.closedKeys[w.closedKeys.length - 1]).toBe("1601C");
});

/** A phone: no second window; the system print sheet settles when dismissed or finished. */
function phone() {
  const closedKeys: string[] = [];
  const current = new Map<string, string>();
  let sequence = 0;
  const toasts: string[] = [];
  const state = { chrome: true, dismissed: 0, sheetUp: false };
  let settle: { resolve: () => void; reject: (error: Error) => void } | null = null;
  const host = createPreviewHost({
    available: false,
    windows: { get: () => null, open: () => { throw new Error("no window on a phone"); } },
    nativePreview: (key, requestId) =>
      new Promise<void>((resolve, reject) => {
        settle = { resolve: () => { if (current.get(key) === requestId) state.sheetUp = false; resolve(); }, reject };
      }),
    dismissNative: () => {
      state.dismissed++;
      state.sheetUp = false;
    },
    print: {
      closed: (key, requestId) => {
        closedKeys.push(key);
        if (current.get(key) === requestId) current.delete(key);
      },
      current: (key, requestId) => current.get(key) === requestId,
      reset: () => current.clear(),
    },
    previews: { apply: () => {}, get: () => undefined, remove: () => {} },
    route: () => returnOf(3),
    period: () => null,
    chromeVisible: () => state.chrome,
    toast: (message) => void toasts.push(message),
    errorText: (error) => String((error as Error).message ?? error),
  });
  const open = (key: string) => {
    const requestId = `session-request-${++sequence}`;
    current.set(key, requestId);
    const event = { kind: "open", key, requestId, title: `BIR Form ${key}`, url: "", script: "", revision: 1, pages: 2, paper: "A4", sheet: "Folio", zoom: 1, message: null } as PrintEvent;
    host.hear(event);
    return event;
  };
  const tick = () => new Promise<void>((resolve) => setTimeout(resolve, 0));
  return { host, closedKeys, toasts, state, open, settle: () => settle!, tick };
}

test("phone: a finished or dismissed sheet is forgotten and later locks leave it alone", async () => {
  const p = phone();
  p.open("1601C");
  expect(p.toasts).toEqual(["Preparing the print preview..."]);
  p.settle().resolve();
  await p.tick();
  expect(p.state.sheetUp).toBe(false);
  expect(p.state.dismissed).toBe(0);
  expect(p.closedKeys).toEqual(["1601C"]);
  p.host.closeAll();
  expect(p.state.dismissed).toBe(0);
  expect(p.closedKeys).toEqual(["1601C"]);
});

test("phone: a lock while the PDF is made forgets the preview at once, and a sheet that comes up anyway is taken down", async () => {
  const p = phone();
  p.open("1601C");
  p.state.chrome = false;
  p.host.closeAll();
  // The Rust side forgets it now, so its print call refuses to show the sheet.
  expect(p.closedKeys).toEqual(["1601C"]);
  p.settle().resolve();
  await p.tick();
  expect(p.state.sheetUp).toBe(false);
  expect(p.state.dismissed).toBeGreaterThan(0);
});

test("phone: the refusal after a lock is quiet; a lock with the sheet up takes it down; nothing is asked while locked", async () => {
  const p = phone();
  p.open("1601C");
  p.state.chrome = false;
  p.host.closeAll();
  p.settle().reject(new Error("The preview is closed."));
  await p.tick();
  expect(p.toasts).toEqual(["Preparing the print preview..."]);

  p.state.chrome = true;
  p.open("2551Q");
  p.state.sheetUp = true;
  expect(p.state.sheetUp).toBe(true);
  p.state.chrome = false;
  p.host.closeAll();
  expect(p.state.sheetUp).toBe(false);
  p.settle().resolve();
  await p.tick();

  const before = p.toasts.length;
  p.open("0619E");
  expect(p.toasts.length).toBe(before);
  expect(p.closedKeys.at(-1)).toBe("0619E");
});

test("phone: no sheet or a failed sheet leaves nothing for a later lock to close", async () => {
  const p = phone();
  p.host.closeAll();
  expect(p.state.dismissed).toBe(0);
  p.open("1601C");
  p.settle().reject(new Error("Printer unavailable"));
  await p.tick();
  expect(p.toasts.at(-1)).toBe("Print preview could not open: Printer unavailable");
  p.host.closeAll();
  expect(p.state.dismissed).toBe(0);
  expect(p.closedKeys).toEqual(["1601C"]);
});

for (const sessionChange of [false, true]) {
  for (const reject of [false, true]) {
    test(`phone: an old same-form request cannot close a replacement (session change: ${sessionChange}, failure: ${reject})`, async () => {
      const p = phone();
      const oldEvent = p.open("2551Q");
      const old = p.settle();
      if (sessionChange) p.host.closeAll();
      p.open("2551Q");
      const replacement = p.settle();
      p.state.sheetUp = true;
      const dismissed = p.state.dismissed;
      const closed = p.closedKeys.length;
      if (reject) old.reject(new Error("Old taxpayer's PDF failed"));
      else old.resolve();
      await p.tick();
      expect(p.state.sheetUp).toBe(true);
      expect(p.state.dismissed).toBe(dismissed);
      expect(p.closedKeys.length).toBe(closed);
      expect(p.toasts.some((text) => text.includes("failed"))).toBe(false);
      p.host.hear(oldEvent);
      expect(p.settle()).toBe(replacement);
      replacement.resolve();
      await p.tick();
      expect(p.closedKeys.length).toBe(closed + 1);
    });
  }
}

test("phone: an open event queued before the session ended cannot reopen a sheet", async () => {
  const p = phone();
  const event = p.open("2551Q");
  const pending = p.settle();
  p.host.closeAll();
  const before = p.toasts.length;
  p.host.hear(event);
  expect(p.toasts.length).toBe(before);
  expect(p.settle()).toBe(pending);
  pending.resolve();
  await p.tick();
  expect(p.state.sheetUp).toBe(false);
});
