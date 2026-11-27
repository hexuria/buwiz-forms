// The TIN list's refresh points (D2): the list is asked at start and never on a timer; a screen's own reads use
// max_age 60; the refresh after Activate and after a return from the account site asks the server (max_age 0). The
// fixture records every host call (`tins account_tins(<max_age>)`), so the test reads exactly what was asked.
import { expect, open, test } from "nsh/test";

const ENTRY = "../../TaxpayerDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params: { start: "view=profile", ...params } } });

interface App {
  texts: () => string[];
  shows: (sel: string) => boolean;
  press: (sel: string) => Promise<void>;
  waitFor: (sel: string, opts?: object) => Promise<void>;
  waitUntil: (fn: () => boolean, opts?: object) => Promise<void>;
  wait: (ms: number) => Promise<void>;
}

function tinsCalls(app: App): string[] {
  return app.texts().filter((t) => t.startsWith("tins ")).map((t) => t.slice("tins ".length));
}

test("desktop: at start the account is asked once with max_age 0, before any screen read", async () => {
  const app = (await open(ENTRY, { ...WIDE, ...asking({ tins: "activated" }) })) as unknown as App;
  await app.waitFor("id:tp-tin-status-state");
  const calls = tinsCalls(app);
  expect(calls[0]).toBe("account_tins(0)");
  // Nothing asked it again on a timer: wait, then read the calls once more — the count stands.
  const standing = calls.length;
  await app.wait(2500);
  expect(tinsCalls(app).filter((c) => c === "account_tins(0)").length).toBe(1);
  expect(tinsCalls(app).length - standing).toBeLessThanOrEqual(1);
}, 30_000);

test("phone: a screen's own reads use max_age 60, never 0 after the start read", async () => {
  const app = (await open(ENTRY, asking({ tins: "activated" }))) as unknown as App;
  await app.waitFor("id:tp-tin-status-state");
  await app.waitUntil(() => tinsCalls(app).length > 0, { timeout: 5000 });
  const calls = tinsCalls(app);
  // The start read is the only max_age 0; every read a screen asked for itself is max_age 60.
  expect(calls.filter((c) => c === "account_tins(0)").length).toBe(1);
  for (const c of calls.slice(1)) {
    expect(c.startsWith("account_tins(60)")).toBe(true);
  }
}, 30_000);

test("desktop: after Activate the account is asked again at the server", async () => {
  const app = (await open(ENTRY, { ...WIDE, ...asking({ tins: "not_activated", activate: "ok" }) })) as unknown as App;
  await app.waitFor("id:tp-tin-status-activate");
  await app.waitUntil(() => tinsCalls(app).filter((c) => c === "account_tins(0)").length === 1, { timeout: 8000 });
  await app.press("id:tp-tin-status-activate");
  await app.waitUntil(() => tinsCalls(app).filter((c) => c === "account_tins(0)").length >= 2, { timeout: 10000 });
  expect(tinsCalls(app).some((c) => c.startsWith("activate_tin(00012345300000)"))).toBe(true);
}, 30_000);

test("desktop: opening the Fix action does not itself re-read the account (nothing has changed yet; the return does)", async () => {
  // The read after a visit to the account site happens when the app becomes active on the filer's return (the shell's
  // listener), never when the link opens: at open nothing on the account has changed yet, so no read is asked here.
  const app = (await open(ENTRY, { ...WIDE, ...asking({ tins: "fix" }) })) as unknown as App;
  await app.waitFor("id:tp-tin-status-action");
  await app.waitUntil(() => tinsCalls(app).filter((c) => c === "account_tins(0)").length === 1, { timeout: 8000 });
  const standing = tinsCalls(app).filter((c) => c === "account_tins(0)").length;
  await app.press("id:tp-tin-status-action");
  await app.wait(1500);
  expect(tinsCalls(app).filter((c) => c === "account_tins(0)").length).toBe(standing);
}, 30_000);
