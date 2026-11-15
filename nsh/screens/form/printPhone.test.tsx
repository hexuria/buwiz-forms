// F7-2: Print preview on a phone says only "not available": the form page does not also claim it opened a preview that
// follows the edits. Through PrintDemo.tsx (the frame with the print host mounted, the form page open on a 2551Q).
import { expect, open, test } from "nsh/test";

const DEMO = "../../PrintDemo.tsx";
const START = { start: "form=2551Q/2026/3" };
const NO_PREVIEW = "Print preview is not available on this device.";

test("F7-2 phone: Print preview shows the not-available message once and never says it opened", async () => {
  const app = await open(DEMO, { host: { params: START } }); // 390 x 844, as an iPhone
  await app.waitFor("text:BIR Form 2551Q");
  await app.waitUntil(() => app.enabled("id:fp-print") && !app.shows("text:Opening…"));
  await app.press("id:fp-print");
  await app.waitFor("text:" + NO_PREVIEW, { timeout: 20_000 });
  // Both toasts would be up within the same turn; give the second (the host's, from the Rust event) time to arrive.
  await app.wait(1500);
  expect(app.texts().filter((t) => t === NO_PREVIEW).length).toBe(1);
  expect(app.texts().some((t) => /Print preview opened/.test(t))).toBe(false);
  expect(app.shows("text:no preview of 2551Q")).toBe(true);
}, 120_000);

test("F7-2 desktop: Print preview still says it opened (the second window exists)", async () => {
  const app = await open(DEMO, { width: 1440, height: 900, platform: "macos", host: { params: START } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.waitUntil(() => app.enabled("id:fp-print") && !app.shows("text:Opening…"));
  await app.press("id:fp-print");
  await app.waitFor("text:preview 2551Q: revision 1", { timeout: 20_000 });
  await app.waitFor("text:Print preview opened. It follows your edits live.");
  expect(app.texts().some((t) => t === NO_PREVIEW)).toBe(false);
}, 120_000);
