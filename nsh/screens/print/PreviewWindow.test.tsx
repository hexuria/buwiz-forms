// The preview window's toolbar (PreviewWindow) over a made-up preview, with buttons that only record their calls
// (PreviewDemo.tsx): the summary, Print and Save as PDF, busy and failure notes, the zoom choice. At a desktop window's
// size and at the least width the window allows (560); a phone has no preview window (print.test.tsx).
import { expect, open, test } from "nsh/test";

const DEMO = "./PreviewDemo.tsx";

test("desktop: the toolbar says the page rule; Print and Save as PDF ask for their work and note the result", async () => {
  const app = await open(DEMO, { width: 940, height: 900, platform: "macos" });
  await app.waitFor("id:print-window");
  expect(app.texts()).toContain("A4 · 4 pages · Folio sheet at 88%");
  expect(app.shows("id:print-zoom")).toBe(true);
  expect(app.texts()).toContain("Fit width");
  expect(app.texts()).toContain("100%");

  await app.press("id:print-print");
  await app.waitFor("text:Preparing the PDF…");
  // Busy: neither button works twice.
  expect(app.enabled("id:print-print")).toBe(false);
  expect(app.enabled("id:print-save-pdf")).toBe(false);
  await app.waitFor("text:Preparing the PDF…", { state: "gone" });
  expect(app.texts()).toContain("calls: print 2551Q");

  await app.press("id:print-save-pdf");
  await app.waitFor("text:Saved BIR Form 2551Q.pdf");
  expect(app.texts()).toContain("calls: print 2551Q, save 2551Q");
  expect(app.enabled("id:print-print")).toBe(true);
});

test("desktop: a cancelled save says nothing; a failure says why, in words", async () => {
  const cancelled = await open(DEMO, { width: 940, height: 900, platform: "macos", host: { params: { mode: "cancel" } } });
  await cancelled.press("id:print-save-pdf");
  await cancelled.waitFor("text:calls: save 2551Q");
  await cancelled.waitFor("text:Preparing the PDF…", { state: "gone" });
  expect(cancelled.texts().some((t) => t.startsWith("Saved"))).toBe(false);

  const failing = await open(DEMO, { width: 940, height: 900, platform: "macos", host: { params: { mode: "fail" } } });
  await failing.press("id:print-print");
  await failing.waitFor("text:No printer is available.");
  expect(failing.enabled("id:print-print")).toBe(true);
  await failing.press("id:print-save-pdf");
  await failing.waitFor("text:Could not save the PDF: the disk is full.");
});

test("desktop: a wait that never ends has a Cancel that always works and is logged", async () => {
  const app = await open(DEMO, { width: 940, height: 900, platform: "macos", host: { params: { mode: "hang" } } });
  await app.waitFor("id:print-window");
  expect(app.shows("id:print-stop-waiting")).toBe(false);
  await app.press("id:print-save-pdf");
  await app.waitFor("text:Preparing the PDF…");
  expect(app.enabled("id:print-save-pdf")).toBe(false);
  await app.press("id:print-stop-waiting");
  await app.waitFor("text:Stopped waiting. Press the button again to try once more.");
  expect(app.shows("id:print-stop-waiting")).toBe(false);
  expect(app.enabled("id:print-save-pdf")).toBe(true);
  expect(app.enabled("id:print-print")).toBe(true);
  expect(app.texts()).toContain("calls: save 2551Q, stopped Save as PDF");
  // The buttons work again after it.
  await app.press("id:print-print");
  await app.waitFor("text:calls: save 2551Q, stopped Save as PDF, print 2551Q");
});

test("the least width of the window (560): both buttons are there and press", async () => {
  const app = await open(DEMO, { width: 560, height: 700, platform: "macos" });
  await app.waitFor("id:print-window");
  expect(app.shows("id:print-print")).toBe(true);
  expect(app.shows("id:print-save-pdf")).toBe(true);
  await app.press("id:print-print");
  await app.waitFor("text:calls: print 2551Q");
});

test("the zoom choice is a pressable pair", async () => {
  const app = await open(DEMO, { width: 940, height: 900, platform: "macos" });
  await app.waitFor("id:print-window");
  await app.press("text:100%");
  await app.press("text:Fit width");
  expect(app.shows("id:print-sheet")).toBe(true);
});
