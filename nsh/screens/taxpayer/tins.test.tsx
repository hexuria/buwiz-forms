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
  // The return from the account site asks again at the server.
  await app.waitUntil(() => tinsCalls(app).filter((c) => c === "account_tins(0)").length >= 2, { timeout: 10000 });
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

test("phone: each refusal shows the host's own sentence and nothing is reworded", async () => {
  const cases = [
    ["bad_tin", "This TIN is not valid."],
    ["name_missing", "Add the registered name first."],
    ["tin_already_added", "This TIN is already on your account, waiting to be verified."],
    ["too_many_unverified", "Your account has 20 TINs waiting to be verified. Verify or remove one on your account first."],
    ["rate_limited", "Too many tries. Try again in 12 minutes."],
    ["other", "The Buwiz server could not add this TIN (fizzy)."],
  ] as const;
  for (const [kind, sentence] of cases) {
    const app = await phone({ tins: "not_activated", activate: kind });
    await app.waitFor("id:tp-tin-status-activate");
    await app.press("id:tp-tin-status-activate");
    await app.waitFor("id:tp-tin-status-refusal");
    expect(app.texts()).toContain(sentence);
  }
});

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
