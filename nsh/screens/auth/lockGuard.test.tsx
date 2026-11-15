// F8-4 and F8-5: the sign-in lock never opens on an answer nobody gave, and nothing of the last session shows on the lock screen.
// F8-4: when the host cannot say whether the admin has a PIN (auth_state throws at boot), the app stays locked: no frame, no sign-in,
// no "Open without signing in", the host's sentence and a Try again. F8-5: on the lock screen another taxpayer's filing events and the
// device-filings note do not toast, and a toast up when the app locks goes with the session. Over AuthDemo.tsx (Juana Santos).
import { expect, open, test } from "nsh/test";

const ENTRY = "../../AuthDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const BROKEN = "The database is busy. Try again in a moment.";

test("F8-4 desktop: auth_state failing at boot keeps the app locked behind the host's sentence; Try again opens the sign-in once it answers", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { demo: true, mode: "solo", adminPin: "1234", authFails: BROKEN } } });
  await app.waitFor("id:access-error-card");
  expect(app.texts()).toContain(BROKEN);
  expect(app.shows("id:frame")).toBe(false);
  expect(app.shows("id:auth-card")).toBe(false);
  // Still failing: Try again keeps it locked.
  await app.press("id:access-error-retry");
  await app.wait(200);
  expect(app.shows("id:access-error-card")).toBe(true);
  expect(app.shows("id:frame")).toBe(false);
  // The host answers: the sign-in screen, never the frame.
  await app.press("id:demo-auth-heal");
  await app.press("id:access-error-retry");
  await app.waitFor("id:auth-card");
  expect(app.shows("id:frame")).toBe(false);
});

test("F8-4 desktop: with no PIN either, a failing read stays locked; once the host says there is no code, the frame opens", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { demo: true, mode: "solo", authFails: BROKEN } } });
  await app.waitFor("id:access-error-card");
  expect(app.shows("id:frame")).toBe(false);
  await app.press("id:demo-auth-heal");
  await app.press("id:access-error-retry");
  await app.waitFor("id:frame");
});

async function signIn(app: Awaited<ReturnType<typeof open>>) {
  await app.waitFor("id:auth-card");
  await app.type("id:auth-tin", "000123453");
  await app.type("id:auth-tp-pin", "1234");
  await app.waitFor("id:frame");
}

test("F8-5 desktop: a lock takes the toasts with it, and on the lock screen no filing event or upload note toasts; signed in, they do", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { demo: true, years: true, mode: "solo", adminPin: "1234", polls: true } } });
  await signIn(app);
  await app.press("id:demo-toast");
  await app.waitFor("text:A note for the person at the keyboard.");

  await app.keyboard.press("mod+l");
  await app.waitFor("id:auth-card");
  await app.waitUntil(() => !app.shows("text:A note for the person at the keyboard."), { timeout: 2000 });

  // The worker files a return and an upload happens while the lock screen is up.
  await app.press("id:demo-event");
  await app.wait(300);
  expect(app.texts().filter((t) => /was submitted|Uploaded a 2551Q/.test(t))).toEqual([]);

  // The owner signs in: the next poll tells them.
  await signIn(app);
  await app.press("id:demo-event");
  await app.waitFor("text:/^1601C for .* was submitted$/");
  await app.waitFor("text:Uploaded a 2551Q an assistant queued.");
});
