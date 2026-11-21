// The Buwiz account gate, at phone size and at desktop size: the welcome card, the device flow up to the host call (a test
// never reaches the network: the host's answer is the fixture's), a refusal shown and the buttons back, and an approval
// that opens the app. The dev environment's own bypass (EBIR_TEST_ENV with BIR_AUTH_BYPASS) is what every other test runs on.
import { expect, open, test } from "nsh/test";

const ENTRY = "../../AuthDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };

test("phone: Sign in reaches the host; its refusal shows on the card and the buttons come back", async () => {
  const app = await open(ENTRY, { host: { params: { gate: true, begin: "refuse", readout: true } } });
  await app.waitFor("id:gate-card");
  expect(app.texts()).toContain("Welcome to eBIRForms");
  expect(app.texts()).toContain("Sign in with your Buwiz account to file from this device.");
  expect(app.shows("id:frame")).toBe(false);
  expect(app.shows("id:setup-card")).toBe(false);
  await app.press("id:gate-sign-in");
  await app.waitFor("text:Could not reach the Buwiz server at http://localhost:3008. Check the connection and try again.");
  expect(app.texts()).toContain("calls: cloud_account_begin_sign_in");
  expect(app.enabled("id:gate-sign-in")).toBe(true);
  expect(app.texts()).toContain("Sign in");
  await app.press("id:gate-create-account");
  await app.waitFor("text:calls: cloud_account_begin_sign_in,cloud_account_begin_sign_up");
});

test("desktop: Create account's approval opens the session, then first launch", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { gate: true, begin: "code", readout: true } } });
  await app.waitFor("id:gate-card");
  await app.press("id:gate-create-account");
  await app.waitFor("id:setup-card");
  expect(app.texts()).toContain("calls: cloud_account_begin_sign_up,cloud_account_finish_sign_in,cloud_account_session_opened");
});

test("desktop: an approved owner with a PIN gets the sign-in next", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { gate: true, begin: "code", demo: true, mode: "solo", adminPin: "1234", readout: true } } });
  await app.waitFor("id:gate-card");
  await app.press("id:gate-sign-in");
  await app.waitFor("id:auth-card");
  expect(app.shows("id:frame")).toBe(false);
});
