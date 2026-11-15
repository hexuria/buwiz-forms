// A queued return is sent by the submission worker within a second or two, and nothing tells the app. QueueDemo.tsx fakes the filed
// list (the worker's return is Queued, then Submitted) and the host's refusal of a cancel-queue on a return that was sent.
// Wide (the board) and phone (the list under tabs).
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";

const ENTRY = "../QueueDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });
const CARD = "id:return-9101";
const CANCEL = "id:return-9101-cancel";
const inCol = (column: string) => `id:filings-col-${column} >> ${CARD}`;
const readout = (app: App, prefix: string) => Number(/(\d+)$/.exec(app.texts().find((t) => t.startsWith(prefix)) ?? "")?.[1] ?? -1);

test("desktop: while a return is Queued the lists are read again every 3 s, so the card leaves Queued once the worker has sent it", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ watch: true, sentAfter: 1200 }) });
  await app.waitFor(CANCEL);
  expect(app.shows(inCol("queued"))).toBe(true);
  const before = readout(app, "filed reads:");
  // The worker sends it at 1.2 s; nothing the person does moves the cache. The watch reads at 1 s (still Queued), then 4 s.
  await app.waitFor(CANCEL, { state: "gone", timeout: 8000 });
  expect(app.shows(inCol("submitted"))).toBe(true);
  expect(readout(app, "filed reads:")).toBeGreaterThan(before);
});

test("desktop: with nothing watching, a card the worker has sent still says Queued, and Cancel queue says so plainly and refreshes", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ watch: false, sentAfter: 200 }) });
  await app.waitFor(CANCEL);
  await app.wait(1500);
  // Stale: the worker sent it, the board does not know.
  expect(app.shows(CANCEL)).toBe(true);
  await app.press(CANCEL);
  await app.waitFor("text:This return was already sent.");
  expect(app.texts().some((t) => t.includes("Other error"))).toBe(false);
  expect(app.texts().some((t) => t.includes("unclaimed"))).toBe(false);
  expect(readout(app, "cancels:")).toBe(1);
  // The refusal read the lists again: the card is Submitted now.
  await app.waitFor(CANCEL, { state: "gone", timeout: 5000 });
  expect(app.shows(inCol("submitted"))).toBe(true);
});

test("phone: the same refusal on the Queued tab", async () => {
  const app = await open(ENTRY, asking({ watch: false, sentAfter: 200 }));
  await app.waitFor("id:filings");
  await app.press("id:filings-tabs >> text:Queued");
  await app.waitFor(CANCEL);
  await app.wait(1500);
  await app.press(CANCEL);
  await app.waitFor("text:This return was already sent.");
  await app.waitFor(CANCEL, { state: "gone", timeout: 5000 });
});
