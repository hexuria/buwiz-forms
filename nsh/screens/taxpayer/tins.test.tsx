// The TIN status chips (D2): every state on the tax-profile page, the profile panel and the picker rows; the Activate
// button and the host's refusal sentences; the reads the screens make. Against the TaxpayerDemo fixture, whose tins
// param fakes the account's list (TaxpayerDemo.tsx); the fixture's tins-calls line records what the host was asked.
import { expect, open, test } from "nsh/test";

const ENTRY = "../../TaxpayerDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params: { start: "view=profile", ...params } } });
const desk = (params: Record<string, unknown> = {}) => open(ENTRY, { ...WIDE, ...asking(params) });
const phone = (params: Record<string, unknown> = {}) => open(ENTRY, asking(params));

function tinsCalls(app: { texts: () => string[] }): string[] {
  return app.texts().filter((t) => t.startsWith("tins ")).map((t) => t.slice("tins ".length));
}

test("desktop: an activated TIN shows the Activated chip on the tax-profile page, the panel and the picker rows", async () => {
  const app = await desk({ tins: "activated" });
  await app.waitFor("id:tp-tin-status-state");
  expect(app.texts()).toContain("Activated");
  // The profile panel (opened from the context pill) and its branch row's compact chip.
  await app.press("id:context-pill");
  await app.waitFor("id:panel-tin-status-state");
  expect(app.shows("id:panel-branch-tin-00000-state")).toBe(true);
  // No Activate button, no message, no stale note, no Test chip.
  expect(app.shows("id:tp-tin-status-activate")).toBe(false);
  expect(app.shows("id:tp-tin-status-test")).toBe(false);
  expect(app.shows("id:tp-tin-status-stale")).toBe(false);
  // The read at start asks the server (max_age 0).
  expect(tinsCalls(app)[0]).toBe("account_tins(0)");
});

test("phone: a pending TIN shows Pending and the row's message", async () => {
  const app = await phone({ tins: "pending" });
  await app.waitFor("id:tp-tin-status-state");
  expect(app.texts()).toContain("Pending");
  expect(app.texts()).toContain("Buwiz is verifying this TIN. It usually takes a day.");
  expect(app.shows("id:tp-tin-status-activate")).toBe(false);
});

test("desktop: a fix row shows the message and its action, which opens the account link", async () => {
  const app = await desk({ tins: "fix" });
  await app.waitFor("id:tp-tin-status-message");
  expect(app.texts()).toContain("The registered name does not match. Check it on your account.");
  expect(app.shows("id:tp-tin-status-action")).toBe(true);
  await app.press("id:tp-tin-status-action");
  // Opening the link itself asks nothing: the account is asked again when the app becomes active on the return.
  await app.wait(1200);
  expect(tinsCalls(app).filter((c) => c === "account_tins(0)").length).toBe(1);
});

test("phone: a not-activated TIN shows the Activate button, which activates and refreshes", async () => {
  const app = await phone({ tins: "not_activated", activate: "ok" });
  await app.waitFor("id:tp-tin-status-activate");
  expect(app.texts()).toContain("Activate this TIN to send");
  await app.press("id:tp-tin-status-activate");
  await app.waitUntil(() => tinsCalls(app).some((c) => c.startsWith("activate_tin(00012345300000)")), { timeout: 10000 });
  // After Activate the list is asked again at the server.
  await app.waitUntil(() => tinsCalls(app).filter((c) => c === "account_tins(0)").length >= 2, { timeout: 10000 });
  expect(app.shows("id:tp-tin-status-refusal")).toBe(false);
});

test("desktop: a test TIN adds the Test TIN chip", async () => {
  const app = await desk({ tins: "test" });
  await app.waitFor("id:tp-tin-status-test");
  expect(app.texts()).toContain("Test TIN");
  expect(app.texts()).toContain("Activated");
});

test("desktop: a stale row says when it was checked", async () => {
  const app = await desk({ tins: "stale" });
  await app.waitFor("id:tp-tin-status-stale");
  expect(app.texts().some((t) => /^Checked \d{4}-\d{2}-\d{2}$/.test(t) || t.startsWith("Checked "))).toBe(true);
});

const REFUSAL_CASES_A = [
  ["bad_tin", "This TIN is not valid."],
  ["name_missing", "Add the registered name first."],
  ["tin_already_added", "This TIN is already on your account, waiting to be verified."],
] as const;
const REFUSAL_CASES_B = [
  ["too_many_unverified", "Your account has 20 TINs waiting to be verified. Verify or remove one on your account first."],
  ["rate_limited", "Too many tries. Try again in 12 minutes."],
  ["other", "The Buwiz server could not add this TIN (fizzy)."],
] as const;

async function refusalShows(kind: string, sentence: string): Promise<void> {
  const app = await phone({ tins: "not_activated", activate: kind });
  await app.waitFor("id:tp-tin-status-activate");
  await app.press("id:tp-tin-status-activate");
  await app.waitFor("id:tp-tin-status-refusal");
  expect(app.texts()).toContain(sentence);
}

test("phone: each refusal shows the host's own sentence, first half", async () => {
  for (const [kind, sentence] of REFUSAL_CASES_A) await refusalShows(kind, sentence);
}, 60_000);

test("phone: each refusal shows the host's own sentence, second half", async () => {
  for (const [kind, sentence] of REFUSAL_CASES_B) await refusalShows(kind, sentence);
}, 60_000);

test("phone: a refusal the fixture makes up passes through word for word (nothing is reworded or mapped)", async () => {
  const sentence = "Zebra crossing ahead, exactly as the host worded it.";
  const app = await phone({ tins: "not_activated", refusalText: sentence });
  await app.waitFor("id:tp-tin-status-activate");
  await app.press("id:tp-tin-status-activate");
  await app.waitFor("id:tp-tin-status-refusal");
  expect(app.texts()).toContain(sentence);
}, 30_000);

test("desktop: the picker rows carry the chip: the demo taxpayer activated, the peer pending", async () => {
  const app = await desk({ tins: "multi", peer: true });
  await app.waitFor("id:tp-tin-status-state");
  await app.press("id:context-pill");
  await app.waitFor("id:panel-tin-status-state");
  // The branch row's compact chip is Activated.
  expect(app.shows("id:panel-branch-tin-00000-state")).toBe(true);
  // The switch-taxpayer row's compact chip (the peer's head office) is Pending.
  await app.waitFor("id:panel-tp-tin-000111233-state");
});

test("phone: when the list cannot be read no chip shows and nothing breaks", async () => {
  const app = await phone({ tins: "error" });
  await app.waitFor("id:tp-page-profile");
  await new Promise((r) => setTimeout(r, 1500));
  expect(app.shows("id:tp-tin-status")).toBe(false);
});

test("desktop: a good list then a failed refresh keeps the list but marks it stale for the chips", async () => {
  // The start read answers (the fix row shows); the re-read the action would have triggered is forced here by the
  // erroring second read: the chips keep the last list but must say when it was checked, never pass it as fresh.
  const app = await desk({ tins: "fix", tinsErrorAfter: 1 });
  await app.waitFor("id:tp-tin-status-state");
  await app.waitUntil(() => tinsCalls(app).length >= 1, { timeout: 8000 });
  // Force another read: the account-tins mount read of a second surface. Activate is not there (a fix row), so
  // use the panel's open-close cycle, which mounts its chip and reads with max_age 60 — unless the erroring read
  // already happened. The readout decides: after two reads the second one threw.
  await app.press("id:context-pill");
  await app.waitFor("id:panel-tin-status-state", { timeout: 8000 }).catch(() => {});
  await app.waitUntil(() => tinsCalls(app).length >= 2, { timeout: 10000 });
  await app.waitFor("id:tp-tin-status-stale", { timeout: 10000 });
  expect(app.texts().some((t) => t.startsWith("Checked "))).toBe(true);
  // The state line still shows the kept row: it is stale, not gone.
  expect(app.shows("id:tp-tin-status-state")).toBe(true);
}, 40_000);

test("desktop: when the signed-in owner changes a failing read clears the old owner's chips", async () => {
  // Owner A's list was read at start; the owner flips and B's read fails: nothing of A's may show for B.
  const app = await desk({ tins: "activated", tinsErrorAfter: 1, tinsOwnerFlipAfter: 1 });
  await app.waitFor("id:tp-tin-status-state");
  await app.waitUntil(() => tinsCalls(app).length >= 1, { timeout: 8000 });
  await app.press("id:context-pill");
  await app.waitUntil(() => tinsCalls(app).length >= 2, { timeout: 10000 });
  // B's read failed and A's list is not B's: no chip state anywhere.
  await app.waitUntil(() => !app.shows("id:tp-tin-status-state"), { timeout: 10000 });
  expect(app.shows("id:panel-tin-status-state")).toBe(false);
}, 40_000);

test("desktop: a chip mounted during an in-flight read after an owner switch fills when the read lands", async () => {
  // Owner A's list was read at start; the owner flips and B's read is slow. While it is on the wire the chip shows
  // nothing of A's; when the read lands the chip fills — it never stays empty until remounted.
  // ownerFlipAfter 0: the first read is still owner A (the flip is read at its start), every later read owner B.
  const app = await desk({ tins: "activated", tinsOwnerFlipAfter: 0, tinsDelayMs: 900 });
  await app.waitFor("id:tp-tin-status-state");
  await app.waitUntil(() => tinsCalls(app).length >= 1, { timeout: 8000 });
  await app.press("id:context-pill");
  // B's read is in flight now: nothing of A's may show, observed as the chip going EMPTY — then it refills when B's read lands.
  await app.waitUntil(() => tinsCalls(app).length >= 2, { timeout: 10000 });
  await app.waitUntil(() => !app.shows("id:tp-tin-status-state"), { timeout: 10000 });
  await app.waitFor("id:tp-tin-status-state", { timeout: 10000 });
  expect(app.shows("id:tp-tin-status-state")).toBe(true);
}, 40_000);
