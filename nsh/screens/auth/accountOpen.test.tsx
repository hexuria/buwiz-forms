// A3a's account-open page side: a relaunch or sign-in waits behind Opening/Moving until the host's commit,
// a blocked move offers Try again, and the migration pages really sign the account out. The host calls are
// AuthDemo's deterministic fakes; no network.
import { type App, expect, open, test } from "nsh/test";

const ENTRY = "../../AuthDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const, clock: {} };
const BASE = { demo: true, years: true, mode: "solo", firstRun: false, gate: true, readout: true };

async function run(app: App, ms: number): Promise<void> {
  await (app.clock as unknown as { runFor(ms: number): Promise<void> }).runFor(ms);
}

async function toCode(app: App): Promise<void> {
  await app.waitFor("id:gate-start");
  await app.press("id:gate-start-continue");
  await app.waitFor("id:gate-email-page");
  await app.type("id:gate-email", "juana@example.com");
  await app.press("id:gate-continue");
  await app.waitFor("id:gate-code-page", { timeout: 15000 });
}

for (const kind of ["opening", "moving"] as const) {
  test(`desktop: a relaunch stored sign-in waits on ${kind}; the sign-in page is never shown`, async () => {
    const app = await open(ENTRY, { ...WIDE, host: { params: { ...BASE, claim: kind, commitAfter: 3 } } });
    await run(app, 100);
    expect(app.shows("id:gate-start")).toBe(false);
    await run(app, 3200);
    expect(app.shows("id:frame")).toBe(true);
    expect(app.shows("id:gate-start")).toBe(false);
  }, 30_000);
}

test("desktop: a sign-in waits on Moving and Home loads the kept values only after commit", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { ...BASE, signInClaim: "moving", commitAfter: 3 } } });
  await toCode(app);
  await app.type("id:gate-code-boxes", "246810");
  await run(app, 100);
  expect(app.shows("id:frame")).toBe(false);
  expect(app.shows("id:migration-moving")).toBe(true);
  await run(app, 3200);
  expect(app.shows("id:frame")).toBe(true);
  expect(app.texts()).toContain("returns: 000123453 | deadlines: 000123453 | taxpayers: 000123453");
}, 30_000);

test("desktop: a blocked move shows its page and Try again starts the polling again", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { ...BASE, migrationBlocked: "transient", commitAfter: 3 } } });
  await run(app, 100);
  expect(app.shows("id:migration-transient")).toBe(true);
  expect(app.shows("id:gate-start")).toBe(false);
  const before = app.texts().find((t) => t.startsWith("migration reads:")) ?? "migration reads: 0";
  await app.press("id:migration-try-again");
  await run(app, 100);
  const after = app.texts().find((t) => t.startsWith("migration reads:")) ?? "migration reads: 0";
  expect(after).not.toBe(before);
  await run(app, 3200);
  expect(app.shows("id:frame")).toBe(true);
}, 30_000);

test("desktop: opening and moving do not flash between their two sentences when the host flips once", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { ...BASE, claim: "opening", flip: "moving", commitAfter: 6 } } });
  await run(app, 1200);
  expect(app.texts()).toContain("Opening your account…");
  await run(app, 1200);
  expect(app.texts()).toContain("Opening your account…");
  expect(app.texts()).not.toContain("Moving your returns…");
}, 30_000);

for (const [kind, page, button] of [
  ["transient", "migration-transient", "migration-transient-sign-out"],
  ["moved_not_opened", "migration-moved-not-opened", "migration-moved-sign-out"],
  ["permanent", "migration-permanent", "migration-permanent-sign-out"],
] as const) {
  test(`desktop: ${kind} Sign out really signs out before the gate locks`, async () => {
    const app = await open(ENTRY, { ...WIDE, host: { params: { ...BASE, migrationBlocked: kind, signOut: true } } });
    await run(app, 100);
    expect(app.shows(`id:${page}`)).toBe(true);
    await app.press(`id:${button}`);
    await app.waitFor("id:gate-start", { timeout: 5000 });
    expect(app.shows("id:frame")).toBe(false);
    const calls = app.texts().find((t) => t.startsWith("calls:")) ?? "";
    const signOutAt = calls.indexOf("cloud_account_sign_out");
    const closeAt = calls.indexOf("cloud_account_session_closed");
    expect(signOutAt >= 0).toBe(true);
    expect(closeAt > signOutAt).toBe(true);
  }, 30_000);
}
