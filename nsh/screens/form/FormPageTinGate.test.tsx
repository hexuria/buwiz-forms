// The Queue TIN gate (D2): before a return is queued the page asks the account (max_age 0). A fresh, complete list that
// says this TIN and branch is not activated — pending, fix or not on the account — stops Queue on a sheet with the
// status, the message and the action, and NOTHING is queued. A stale, incomplete or unreadable list lets Queue go
// ahead as today. FormPageAny.tsx fakes the list and records the host calls.
import { tinKeyParts } from "../taxpayer/tins";
import { expect, open, test } from "nsh/test";
import { warmForms } from "../../bir/testing";

const ENTRY = "./FormPageAny.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };

warmForms();

interface App {
  texts: () => string[];
  shows: (sel: string) => boolean;
  press: (sel: string) => Promise<void>;
  waitFor: (sel: string, opts?: object) => Promise<void>;
  waitUntil: (fn: () => boolean, opts?: object) => Promise<void>;
}

function tinsCalls(app: App): string[] {
  return app.texts().filter((t) => t.startsWith("tins ")).map((t) => t.slice("tins ".length));
}

/** Opens 2551Q with the tins fake and presses Queue past the page's own stops until the gate or the host answers. */
async function pageAndQueue(tins: string, extra: Record<string, unknown> = {}): Promise<App> {
  const app = (await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 1, tins, facts: { eopt_tier: "Micro", tax_classification: "SelfEmployed" }, ...extra } } })) as unknown as App;
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-queue");
  return app;
}

test("desktop: a fresh complete list saying not_activated stops Queue on the sheet and nothing is queued", async () => {
  const app = await pageAndQueue("not_activated");
  // The page's own notes may stop Queue first; press past them until the gate sheet or the host call.
  for (let i = 0; i < 6; i++) {
    if (app.shows("id:tin-queue-blocked") || tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue"))) break;
    if (app.shows("id:queue-continue")) await app.press("id:queue-continue");
    else await app.press("id:fp-queue");
    await new Promise((r) => setTimeout(r, 700));
  }
  await app.waitFor("id:tin-queue-blocked", { timeout: 10000 });
  // The list was asked at max_age 0 and no queue verb ever ran.
  expect(tinsCalls(app)).toContain("account_tins(0)");
  expect(tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue"))).toBe(false);
  // The sheet names the status and offers Close; the not-activated row has no message and no action link.
  expect(app.texts()).toContain("This TIN is not activated");
  expect(app.shows("id:tin-blocked-close")).toBe(true);
  await app.press("id:tin-blocked-close");
  await app.waitFor("id:tin-queue-blocked", { state: "gone" });
}, 40_000);

test("desktop: a fresh complete list saying pending stops Queue with the row's message", async () => {
  const app = await pageAndQueue("pending");
  for (let i = 0; i < 6; i++) {
    if (app.shows("id:tin-queue-blocked")) break;
    if (app.shows("id:queue-continue")) await app.press("id:queue-continue");
    else await app.press("id:fp-queue");
    await new Promise((r) => setTimeout(r, 700));
  }
  await app.waitFor("id:tin-queue-blocked", { timeout: 10000 });
  expect(app.texts()).toContain("This TIN is waiting to be verified");
  expect(app.texts()).toContain("Buwiz is verifying this TIN. It usually takes a day.");
}, 40_000);

test("desktop: a fix row shows its action on the sheet", async () => {
  const app = await pageAndQueue("fix");
  for (let i = 0; i < 6; i++) {
    if (app.shows("id:tin-queue-blocked")) break;
    if (app.shows("id:queue-continue")) await app.press("id:queue-continue");
    else await app.press("id:fp-queue");
    await new Promise((r) => setTimeout(r, 700));
  }
  await app.waitFor("id:tin-queue-blocked", { timeout: 10000 });
  expect(app.texts()).toContain("This TIN needs a step on your account");
  expect(app.texts()).toContain("The registered name does not match. Check it on your account.");
  expect(app.shows("id:tin-blocked-action")).toBe(true);
}, 40_000);

test("desktop: an activated TIN queues as today", async () => {
  const app = await pageAndQueue("activated");
  for (let i = 0; i < 8; i++) {
    if (tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue"))) break;
    if (app.shows("id:queue-continue")) await app.press("id:queue-continue");
    else if (!tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue"))) await app.press("id:fp-queue");
    await new Promise((r) => setTimeout(r, 700));
  }
  await app.waitUntil(() => tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue")), { timeout: 15000 });
  expect(app.shows("id:tin-queue-blocked")).toBe(false);
}, 40_000);

test("desktop: a stale list lets Queue go ahead", async () => {
  const app = await pageAndQueue("stale");
  for (let i = 0; i < 8; i++) {
    if (tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue"))) break;
    if (app.shows("id:queue-continue")) await app.press("id:queue-continue");
    else if (!tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue"))) await app.press("id:fp-queue");
    await new Promise((r) => setTimeout(r, 700));
  }
  await app.waitUntil(() => tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue")), { timeout: 15000 });
  expect(app.shows("id:tin-queue-blocked")).toBe(false);
}, 40_000);

test("desktop: an incomplete list lets Queue go ahead", async () => {
  const app = await pageAndQueue("incomplete");
  for (let i = 0; i < 8; i++) {
    if (tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue"))) break;
    if (app.shows("id:queue-continue")) await app.press("id:queue-continue");
    else if (!tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue"))) await app.press("id:fp-queue");
    await new Promise((r) => setTimeout(r, 700));
  }
  await app.waitUntil(() => tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue")), { timeout: 15000 });
  expect(app.shows("id:tin-queue-blocked")).toBe(false);
}, 40_000);

test("desktop: an unreadable list lets Queue go ahead", async () => {
  const app = await pageAndQueue("error");
  for (let i = 0; i < 8; i++) {
    if (tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue"))) break;
    if (app.shows("id:queue-continue")) await app.press("id:queue-continue");
    else if (!tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue"))) await app.press("id:fp-queue");
    await new Promise((r) => setTimeout(r, 700));
  }
  await app.waitUntil(() => tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue")), { timeout: 15000 });
  expect(app.shows("id:tin-queue-blocked")).toBe(false);
}, 40_000);

test("desktop: a good list then an error at Queue lets it through", async () => {
  // The first Queue read answers fine and stops on the sheet (the list says not_activated). After the sheet is
  // dismissed the server errors: the kept list must not be read as fresh — the gate uses this call's answer, or
  // null when it threw, and null lets Queue go ahead as today.
  const app = await pageAndQueue("not_activated", { tinsErrorAfter: 1 });
  for (let i = 0; i < 6 && !app.shows("id:tin-queue-blocked"); i++) {
    if (app.shows("id:queue-continue")) await app.press("id:queue-continue");
    else await app.press("id:fp-queue");
    await new Promise((r) => setTimeout(r, 700));
  }
  await app.waitFor("id:tin-queue-blocked", { timeout: 10000 });
  await app.press("id:tin-blocked-close");
  await app.waitFor("id:tin-queue-blocked", { state: "gone" });
  for (let i = 0; i < 8; i++) {
    if (tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue"))) break;
    if (app.shows("id:queue-continue")) await app.press("id:queue-continue");
    else await app.press("id:fp-queue");
    await new Promise((r) => setTimeout(r, 700));
  }
  await app.waitUntil(() => tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue")), { timeout: 10000 });
  expect(app.shows("id:tin-queue-blocked")).toBe(false);
}, 40_000);

test("desktop: a double Queue press inside the account read queues once, not twice", async () => {
  const app = await pageAndQueue("activated");
  // Press again at once: the first press's account read is still on the wire, so this press must not start a second queue.
  await app.press("id:fp-queue").catch(() => {});
  await app.press("id:fp-queue").catch(() => {});
  for (let i = 0; i < 8; i++) {
    if (tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue"))) break;
    if (app.shows("id:queue-continue")) await app.press("id:queue-continue");
    await new Promise((r) => setTimeout(r, 700));
  }
  await app.waitUntil(() => tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue")), { timeout: 10000 });
  await new Promise((r) => setTimeout(r, 1500));
  const queued = tinsCalls(app).filter((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue"));
  expect(queued.length).toBe(1);
}, 40_000);

test("the gate key normalizes a legacy 12-digit tin (root plus 3-digit branch) to the row's 5-digit branch", () => {
  expect(tinKeyParts("000123453000")).toEqual(["000123453", "00000"]);
  expect(tinKeyParts("00012345300000")).toEqual(["000123453", "00000"]);
  expect(tinKeyParts("00012345300123")).toEqual(["000123453", "00123"]);
  expect(tinKeyParts("000123453123")).toEqual(["000123453", "00123"]);
});


test("desktop: after the account owner changes a failing read clears the old list and Queue goes ahead", async () => {
  // Owner A's list stopped the first Queue on the sheet; then the owner changes and B's read fails: nothing of A's
  // may answer for B, so the second Queue goes ahead.
  const app = await pageAndQueue("not_activated", { tinsErrorAfter: 1, tinsOwnerFlipAfter: 1 });
  for (let i = 0; i < 6 && !app.shows("id:tin-queue-blocked"); i++) {
    if (app.shows("id:queue-continue")) await app.press("id:queue-continue");
    else await app.press("id:fp-queue");
    await new Promise((r) => setTimeout(r, 700));
  }
  await app.waitFor("id:tin-queue-blocked", { timeout: 10000 });
  await app.press("id:tin-blocked-close");
  await app.waitFor("id:tin-queue-blocked", { state: "gone" });
  for (let i = 0; i < 8; i++) {
    if (tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue"))) break;
    if (app.shows("id:queue-continue")) await app.press("id:queue-continue");
    else await app.press("id:fp-queue");
    await new Promise((r) => setTimeout(r, 700));
  }
  await app.waitUntil(() => tinsCalls(app).some((c) => c.startsWith("queue_form_draft") || c.startsWith("screen_form_queue")), { timeout: 10000 });
  expect(app.shows("id:tin-queue-blocked")).toBe(false);
}, 40_000);
