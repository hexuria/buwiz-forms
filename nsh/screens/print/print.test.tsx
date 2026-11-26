// The print preview, end to end against the real Rust side (src/print.rs) in headless runs: Print preview on a 2551Q opens ONE
// second window with the sheet's toolbar, pressing it again focuses that window, edits reach it, leaving the form or closing
// the window ends it, and a phone has no second window. The web view itself draws only in a real window
// (shots/print.test.tsx); here the windows, their titles and what the Rust side sent are what is checked.
import { type App, expect, open, test } from "nsh/test";

const DEMO = "../../PrintDemo.tsx";
const START = { start: "form=2551Q/2026/3" };
/** The window of the 2551Q of the third quarter of 2026 (the demo taxpayer): the form code and the return it was asked on. */
const WINDOW = "print:2551Q:2551Q-00012345300000-2026-3-return";
const WINDOW_Q2 = "print:2551Q:2551Q-00012345300000-2026-2-return";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };

/** The form page is open and its Print preview button works (it is disabled while the return opens). */
async function ready(app: App) {
  await app.waitFor("text:BIR Form 2551Q");
  await app.waitUntil(() => app.enabled("id:fp-print") && !app.shows("text:Opening…"));
}

test("desktop: Print preview opens one window, again focuses it, edits follow, leaving the form closes it", async () => {
  const app = await open(DEMO, { width: 1440, height: 900, platform: "macos", host: { params: START } });
  await ready(app);
  expect(app.window(WINDOW).state()).toBeNull();
  expect(app.shows("text:no preview of 2551Q")).toBe(true);

  await app.press("id:fp-print");
  await app.waitFor("text:preview 2551Q: revision 1", { timeout: 20_000 });
  const first = app.window(WINDOW).state();
  expect(first).not.toBeNull();
  // The window's title says the form code, the period and the taxpayer.
  await app.waitFor("text:title: BIR Form 2551Q · Q3 2026 — Juana Santos");
  // The window's own content: the toolbar over the sheet, with the page rule said (A4, the Folio sheet scaled to fit).
  await app.waitFor("id:print-window");
  expect(app.shows("id:print-print")).toBe(true);
  expect(app.shows("id:print-save-pdf")).toBe(true);
  expect(app.texts().some((t) => /^A4 · \d+ pages? · Folio sheet at \d+%$/.test(t))).toBe(true);

  // Pressing it again refreshes the same window (a newer revision), never a second one.
  await app.press("id:fp-print");
  await app.waitFor("text:preview 2551Q: revision 2");
  expect(app.window(WINDOW).state()).not.toBeNull();

  // An edit in the form reaches the open preview within about a second.
  await app.type("id:fp-frm2551Qv2018:txtSheets", "2");
  await app.waitFor("text:/^preview 2551Q: revision [3-9]/", { timeout: 5000 });

  // Leaving the form ends its preview. The move first waits for the form page to save the edit just typed (flushNow, up to
  // LEAVE_HOOKS_CAP_MS = 3 s in shell/app.ts), then Home renders and the window closes: 15 s, so a slow runner has room.
  await app.press("id:px-home");
  await app.waitUntil(() => app.window(WINDOW).state() === null, { timeout: 15_000 });
  await app.waitFor("text:no preview of 2551Q");
}, 120_000);

test("desktop: the person closing the window ends the preview, and the next press opens a new one", async () => {
  const app = await open(DEMO, { width: 1440, height: 900, platform: "macos", host: { params: START } });
  await ready(app);
  await app.press("id:fp-print");
  await app.waitFor("text:preview 2551Q: revision 1", { timeout: 20_000 });
  await app.window(WINDOW).pressClose();
  await app.waitFor("text:no preview of 2551Q");
  expect(app.window(WINDOW).state()).toBeNull();
  // An edit now has no window to follow (the host function answers false) and opens nothing.
  await app.type("id:fp-frm2551Qv2018:txtSheets", "2");
  await app.wait(600);
  expect(app.window(WINDOW).state()).toBeNull();
  await app.press("id:fp-print");
  await app.waitFor("text:preview 2551Q: revision 1");
  expect(app.window(WINDOW).state()).not.toBeNull();
}, 120_000);

test("phone: there is no second window, and the form page's button says so", async () => {
  const app = await open(DEMO, { host: { params: START } }); // 390 x 844, as an iPhone
  await ready(app);
  await app.press("id:fp-print");
  await app.waitFor("text:Print preview is not available on this device.", { timeout: 20_000 });
  expect(app.shows("text:no preview of 2551Q")).toBe(true);
}, 120_000);

/** Signs the owner in on the sign-in screen the `pin` fixture starts at. */
async function signIn(app: App) {
  await app.waitFor("id:auth-card");
  await app.type("id:auth-tin", "000123453");
  await app.type("id:auth-tp-pin", "1234");
  await app.waitFor("id:frame");
}

test("F8-1 desktop: Lock app closes the preview window, and signing in again does not bring it back", async () => {
  const app = await open(DEMO, { ...WIDE, host: { params: { pin: true } } });
  await signIn(app);
  await app.press("id:px-open");
  await ready(app);
  await app.press("id:fp-print");
  await app.waitFor("text:preview 2551Q: revision 1", { timeout: 20_000 });
  expect(app.window(WINDOW).state()).not.toBeNull();

  await app.keyboard.press("mod+l");
  await app.waitFor("id:auth-card");
  await app.waitUntil(() => app.window(WINDOW).state() === null, { timeout: 5000 });
  await app.waitFor("text:no preview of 2551Q");

  await signIn(app);
  await app.wait(500);
  expect(app.window(WINDOW).state()).toBeNull();
  expect(app.shows("text:no preview of 2551Q")).toBe(true);
}, 120_000);

test("desktop: switching to another taxpayer closes the preview window, so nothing of the last taxpayer stays up", async () => {
  const app = await open(DEMO, { ...WIDE, host: { params: START } });
  await ready(app);
  await app.press("id:fp-print");
  await app.waitFor("text:preview 2551Q: revision 1", { timeout: 20_000 });
  expect(app.window(WINDOW).state()).not.toBeNull();

  await app.press("id:px-switch");
  await app.waitUntil(() => app.window(WINDOW).state() === null, { timeout: 5000 });
  await app.waitFor("text:no preview of 2551Q");
}, 120_000);

test("F8-10 desktop: a quick switch to another return of the same form gets its own window, which survives the old one's close", async () => {
  const app = await open(DEMO, { ...WIDE, host: { params: START } });
  await ready(app);
  await app.press("id:fp-print");
  await app.waitFor("text:preview 2551Q: revision 1", { timeout: 20_000 });
  expect(app.window(WINDOW).state()).not.toBeNull();

  // The other return of the form, then its preview at once.
  await app.press("id:px-open-q2");
  await app.waitFor("text:BIR Form 2551Q");
  await app.waitUntil(() => app.enabled("id:fp-print") && !app.shows("text:Opening…"));
  await app.press("id:fp-print");
  await app.waitFor("text:title: BIR Form 2551Q · Q2 2026 — Juana Santos", { timeout: 20_000 });
  await app.waitUntil(() => app.window(WINDOW).state() === null, { timeout: 5000 });
  expect(app.window(WINDOW_Q2).state()).not.toBeNull();
  // The first window is long gone: the new preview is still the one Rust holds, and still follows an edit.
  await app.wait(800);
  expect(app.window(WINDOW_Q2).state()).not.toBeNull();
  await app.waitFor("text:/^preview 2551Q: revision 1$/");
  await app.type("id:fp-frm2551Qv2018:txtSheets", "2");
  await app.waitFor("text:/^preview 2551Q: revision [2-9]/", { timeout: 5000 });
}, 120_000);
