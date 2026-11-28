// A4 (#528): a different Buwiz account starts with no screen state of the last account. The fake host names
// test-room-a.db or test-room-b.db from the sign-in address; no network. The positive control signs in as A twice:
// the same room's kept values are read again.
import { type App, expect, open, test } from "nsh/test";

const ENTRY = "../../AuthDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const PARAMS = { demo: true, second: true, years: true, mode: "solo" as const, firstRun: false, gate: true, signOut: true, rooms: true, readout: true };

async function cloudSignIn(app: App, email: string): Promise<void> {
  await app.waitFor("id:gate-start", { timeout: 10000 });
  await app.press("id:gate-start-continue");
  await app.waitFor("id:gate-email-page");
  await app.clear("id:gate-email");
  await app.type("id:gate-email", email);
  await app.press("id:gate-continue");
  await app.waitFor("id:gate-code-page", { timeout: 10000 });
  await app.type("id:gate-code-boxes", "246810");
  await app.waitFor("id:frame", { timeout: 15000 });
}

async function leaveState(app: App): Promise<void> {
  await app.waitFor("id:home-dashboard", { timeout: 15000 });
  // A draft/form page in the history.
  await app.press("id:home-tp-000123453-next");
  await app.waitFor("id:form-page", { timeout: 30000 });
  // A filter away from Home, with the form page behind it.
  await app.press("id:nav-filings");
  await app.waitFor("id:filings-search", { timeout: 10000 });
  await app.type("id:filings-search", "juana");
  // The side panel open on the profile.
  await app.press("id:context-pill");
  await app.waitFor("id:panel-open-profile", { timeout: 5000 });
  await app.press("id:panel-open-profile");
  await app.waitFor("id:tp-sec-taxpayer-edit", { timeout: 10000 });
  await app.waitUntil(() => Object.keys(app.storage()).some((k) => k.startsWith("redesign.recent@test-room-a.db")), { timeout: 5000 });
}

async function signOut(app: App): Promise<void> {
  await app.press("id:demo-sign-out");
  await app.waitFor("id:gate-start", { timeout: 10000 });
}

test("desktop: a different account signs in to fresh screens; A twice is the positive control", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: PARAMS } });
  await cloudSignIn(app, "juana@example.com");
  await leaveState(app);
  await signOut(app);
  await cloudSignIn(app, "pedro@example.com");

  // B starts on Home: not the form page, not Filings' typed filter, not A's open side panel.
  await app.waitFor("id:home-dashboard", { timeout: 15000 });
  expect(app.shows("id:form-page")).toBe(false);
  expect(app.shows("id:filings")).toBe(false);
  expect(app.shows("id:tp-sec-taxpayer-edit")).toBe(false);
  expect(app.enabled("id:tb-back")).toBe(false);
  expect(app.enabled("id:tb-forward")).toBe(false);
  await app.press("id:nav-filings");
  await app.waitFor("id:filings-search", { timeout: 10000 });
  expect(app.value("id:filings-search")).toBe("");
  const keys = Object.keys(app.storage());
  expect(keys.some((k) => k.startsWith("redesign.recent@test-room-b.db") && app.storage()[k] !== "[]")).toBe(false);

  // Positive control: sign out and sign in as A again; the same room's kept Recent and selection are read again.
  await signOut(app);
  await cloudSignIn(app, "juana@example.com");
  await app.waitFor("id:home-dashboard", { timeout: 15000 });
  const recentKey = Object.keys(app.storage()).find((k) => k.startsWith("redesign.recent@test-room-a.db"));
  expect(recentKey).toBeDefined();
  expect(app.storage()[recentKey!]).toContain("1601C");
  expect(app.texts().some((t) => t.includes("1601C"))).toBe(true);
}, 90_000);


test("desktop: finishUnlock treats an account change while the gate is open as a new session", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: PARAMS } });
  await cloudSignIn(app, "juana@example.com");
  await leaveState(app);
  await app.press("id:demo-switch-account");
  await app.waitFor("id:home-dashboard", { timeout: 15000 });
  expect(app.shows("id:form-page")).toBe(false);
  expect(app.shows("id:filings")).toBe(false);
  expect(app.shows("id:tp-sec-taxpayer-edit")).toBe(false);
  expect(app.enabled("id:tb-back")).toBe(false);
}, 60_000);

test("desktop: the pre-A1 device-wide welcomed_accounts key is ignored; the device store still welcomes once", async () => {
  const app = await open(ENTRY, {
    ...WIDE,
    host: { params: PARAMS },
    storage: { welcomed_accounts: JSON.stringify(["demo"]) },
  });
  await cloudSignIn(app, "juana@example.com");
  await app.waitFor("text:Welcome. Your Buwiz account is ready.", { timeout: 10000 });
  await signOut(app);
  await cloudSignIn(app, "juana@example.com");
  await app.waitFor("id:home-dashboard", { timeout: 15000 });
  expect(app.texts()).not.toContain("Welcome. Your Buwiz account is ready.");
}, 60_000);
