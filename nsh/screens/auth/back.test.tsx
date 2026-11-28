// Android's back button on the pages before the app (`app.back()` presses it: the app's listeners hear it first). On the email
// and code pages it steps back a page, as the back control does (Gate.tsx). On the first page, the PIN sign-in and the
// first-run steps nothing of the app's hidden history is walked: the press is left to the system, which leaves the app
// (Shell.tsx answers false while the frame is hidden). A cloud sign-out starts the next room on Home; a taxpayer lock still returns to the page the person was on.
import { type App, expect, open, test } from "nsh/test";

const ENTRY = "../../AuthDemo.tsx";
const ANDROID = { width: 390, height: 844, platform: "android" as const };

async function toEmail(app: App): Promise<void> {
  await app.waitFor("id:gate-start");
  await app.press("id:gate-start-continue");
  await app.waitFor("id:gate-email-page");
}

async function signInByEmail(app: App): Promise<void> {
  await toEmail(app);
  await app.clear("id:gate-email");
  await app.type("id:gate-email", "juana@example.com");
  await app.press("id:gate-continue");
  await app.waitFor("id:gate-code-page", { timeout: 15000 });
  await app.type("id:gate-code-boxes", "246810");
  await app.waitFor("id:frame", { timeout: 15000 });
}

test("android: Back on the code page goes to the email page (the address kept), and on the email page to the first page", async () => {
  const app = await open(ENTRY, { ...ANDROID, host: { params: { gate: true, readout: true } } });
  await toEmail(app);
  await app.type("id:gate-email", "juana@example.com");
  await app.press("id:gate-continue");
  await app.waitFor("id:gate-code-page", { timeout: 15000 });
  await app.back();
  await app.waitFor("id:gate-email-page");
  expect(app.value("id:gate-email")).toBe("juana@example.com");
  await app.back();
  await app.waitFor("id:gate-start");
}, 60_000);

test("android: Back on the first page after a sign-out walks nothing of the hidden app: signed in again, Home shows", async () => {
  const app = await open(ENTRY, { ...ANDROID, host: { params: { gate: true, demo: true, mode: "solo", firstRun: false, signOut: true, readout: true } } });
  await signInByEmail(app);
  await app.press("id:tab-calendar");
  await app.waitFor("id:calendar-narrow");
  await app.press("id:demo-sign-out");
  await app.waitFor("id:gate-start", { timeout: 15000 });
  await app.back();
  await app.wait(200);
  expect(app.shows("id:gate-start")).toBe(true);
  await signInByEmail(app);
  // A4: closing the cloud room resets the hidden history and route; the next room starts on Home.
  await app.waitFor("id:home");
  expect(app.shows("id:calendar-narrow")).toBe(false);
}, 90_000);

test("android: Back on the PIN sign-in walks nothing of the hidden app: signed in again, the same page shows", async () => {
  const app = await open(ENTRY, { ...ANDROID, host: { params: { demo: true, mode: "solo", adminPin: "1234", readout: true } } });
  await app.waitFor("id:auth-card");
  await app.type("id:auth-tin", "000123453");
  await app.type("id:auth-tp-pin", "1234");
  await app.waitFor("id:frame");
  await app.press("id:tab-calendar");
  await app.waitFor("id:calendar-narrow");
  await app.keyboard.press("mod+l");
  await app.waitFor("id:auth-card");
  await app.back();
  await app.wait(200);
  expect(app.shows("id:auth-card")).toBe(true);
  await app.clear("id:auth-tin");
  await app.type("id:auth-tin", "000123453");
  await app.type("id:auth-tp-pin", "1234");
  await app.waitFor("id:frame");
  await app.waitFor("id:calendar-narrow");
  expect(app.shows("id:home")).toBe(false);
}, 90_000);

test("android: Back on a first-run step stays on it (nothing hidden is walked)", async () => {
  const app = await open(ENTRY, { ...ANDROID, host: { params: { readout: true } } });
  await app.waitFor("id:setup-card");
  await app.press("id:setup-helpdesk");
  await app.waitFor("id:setup-pin-card");
  await app.back();
  await app.wait(200);
  expect(app.shows("id:setup-pin-card")).toBe(true);
}, 60_000);
