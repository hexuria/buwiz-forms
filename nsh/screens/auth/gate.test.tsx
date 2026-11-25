// The Buwiz account gate, at phone size and at desktop size. It opens on the email page (one page for signing in and creating an
// account): "Continue with email" sends a code and the code page opens (six boxes that check by themselves, Resend with its
// countdown, Change email); the right code goes Home. Nothing else, and never a browser. A test never reaches the network: the
// host's answers are the fixture's (screens/auth/testEmail.ts). The dev environment's own bypass (EBIR_TEST_ENV with
// BIR_AUTH_BYPASS) is what every other test runs on.
import { type App, expect, open, test } from "nsh/test";

const ENTRY = "../../AuthDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const SIZES = [
  { name: "phone", size: {}, device: "this device" },
  { name: "desktop", size: WIDE, device: "this computer" },
] as const;

const EMAIL_SENTENCE = "Enter your email and we will send you a 6-digit code to sign in, or to create your Buwiz account if you are new.";
const EXPIRED_CODE = "This code has expired. Press Resend for a new one.";
const TOO_MANY = "Too many wrong codes. Send a new code to try again.";

function calls(app: App): string {
  return app.texts().find((t) => t.startsWith("calls:")) ?? "";
}

/** Nothing of the old start card or the browser path, no address, no "Waiting…" anywhere. */
function expectClean(texts: string[]): void {
  const gone = ["Create account", "Welcome to eBIRForms", "Waiting…", "Passwordless", "Sign in with Buwiz", "Continue in your browser", "Open the sign-in page"];
  for (const text of texts) {
    for (const word of gone) expect(text.includes(word)).toBe(false);
    expect(/device code|use the browser|https?:\/\/|user_code/i.test(text)).toBe(false);
  }
}

/** Types the address and presses Continue with email; the code page opens. */
async function toCode(app: App, address = "juana@example.com"): Promise<void> {
  await app.waitFor("id:gate-card");
  await app.type("id:gate-email", address);
  await app.press("id:gate-continue");
  await app.waitFor("id:gate-code-card", { timeout: 15000 });
}

for (const { name, size, device } of SIZES) {
  const at = (params: Record<string, unknown> = {}) => open(ENTRY, { ...size, host: { params: { gate: true, readout: true, ...params } } });

  test(`${name}: the gate opens on the email page: its title, one sentence, the field and one button, nothing else`, async () => {
    const app = await at();
    await app.waitFor("id:gate-card");
    for (const t of ["Sign in to eBIRForms", EMAIL_SENTENCE, "Email", "Continue with email"]) expect(app.texts()).toContain(t);
    expect(app.shows("id:frame")).toBe(false);
    expect(app.shows("id:setup-card")).toBe(false);
    expect(app.shows("id:gate-code-card")).toBe(false);
    expect(app.enabled("id:gate-continue")).toBe(true);
    expect(app.texts()).not.toContain("or");
    expectClean(app.texts());
    expect(calls(app)).toBe("calls: none");
  }, 60_000);

  test(`${name}: an empty or bad address is said under the field without a request`, async () => {
    const app = await at();
    await app.waitFor("id:gate-card");
    await app.press("id:gate-continue");
    await app.waitFor("text:Enter your email address.");
    await app.type("id:gate-email", "juana@example");
    await app.waitFor("text:Enter your email address.", { state: "gone" });
    await app.press("id:gate-continue");
    await app.waitFor("text:That email address does not look right. Check it and try again.");
    expect(app.shows("id:gate-card")).toBe(true);
    expect(calls(app)).toBe("calls: none");
  }, 60_000);

  test(`${name}: Continue is busy only while it sends; a refusal is said and the page stays`, async () => {
    const app = await at({ email: "refuse" });
    await app.waitFor("id:gate-card");
    await app.type("id:gate-email", "juana@example.com");
    await app.press("id:gate-continue");
    await app.waitFor("label:Continue with email…");
    expect(app.texts()).not.toContain("Waiting…");
    await app.waitFor("text:A sign-in code was already sent to this address. Use it, or try again in 4 minutes.");
    expect(app.shows("label:Continue with email…")).toBe(false);
    expect(app.shows("id:gate-card")).toBe(true);
    expect(app.enabled("id:gate-continue")).toBe(true);
    expect(calls(app)).toBe("calls: cloud_account_email_start");
  }, 60_000);

  for (const [kind, sentence] of [
    ["unavailable", "Sign-in is not available right now. Try again in a few minutes."],
    ["offline", "No connection. Check your internet and try again."],
  ] as const) {
    test(`${name}: ${kind}: the email page says so and stays; nothing opens`, async () => {
      const app = await at({ email: kind });
      await app.waitFor("id:gate-card");
      await app.type("id:gate-email", "juana@example.com");
      await app.press("id:gate-continue");
      await app.waitFor(`text:${sentence}`);
      await app.wait(500);
      expect(app.shows("id:gate-card")).toBe(true);
      expect(calls(app)).toBe("calls: cloud_account_email_start");
      expect(app.openedUrls()).toEqual([]);
      expectClean(app.texts());
    }, 60_000);
  }

  test(`${name}: the code page: where the code went, Resend locked with its countdown, Change email, the mail's button`, async () => {
    const app = await at();
    await toCode(app);
    for (const t of ["Enter the code", "We sent a 6-digit code to juana@example.com.", "Resend in 5:00", "Change email", `The button in the email also signs you in on ${device}.`]) {
      expect(app.texts()).toContain(t);
    }
    expect(app.enabled("id:gate-resend")).toBe(false);
    expect(app.shows("id:gate-continue")).toBe(false);
    expectClean(app.texts());
    await app.wait(1200);
    expect(app.texts()).toContain("Resend in 4:59");
  }, 60_000);

  test(`${name}: a wrong code checks by itself on the sixth digit, says the tries left, and the boxes empty`, async () => {
    const app = await at({ slow: true });
    await toCode(app);
    await app.type("id:gate-code-boxes", "12345");
    await app.wait(300);
    expect(calls(app)).toBe("calls: cloud_account_email_start");
    await app.type("id:gate-code-boxes", "6");
    await app.waitFor("text:Checking…");
    expect(app.enabled("id:gate-code-boxes")).toBe(false);
    await app.waitFor("text:That code is not right. 4 tries left.", { timeout: 10000 });
    expect(app.texts()).not.toContain("Checking…");
    expect(app.value("id:gate-code-boxes") ?? "").toBe("");
    expect(calls(app)).toBe("calls: cloud_account_email_start,cloud_account_email_verify 123456");
    // Typing again clears the sentence.
    await app.type("id:gate-code-boxes", "1");
    await app.waitFor("text:That code is not right. 4 tries left.", { state: "gone" });
  }, 60_000);

  test(`${name}: a pasted code in every shape fills the boxes and is checked`, async () => {
    const app = await at();
    await toCode(app);
    const shapes = ["123456", "123 457", "123-458", "Your eBIRForms sign-in code is 123 459. It works for 5 minutes."];
    const sent = ["123456", "123457", "123458", "123459"];
    for (const [i, text] of shapes.entries()) {
      await app.paste("id:gate-code-boxes", { text });
      await app.waitUntil(() => calls(app).includes(`cloud_account_email_verify ${sent[i]}`), { timeout: 10000 });
      const left = 4 - i;
      await app.waitFor(`text:That code is not right. ${left} ${left === 1 ? "try" : "tries"} left.`);
    }
  }, 90_000);

  test(`${name}: an expired code says so and Resend unlocks; after too many tries Send a new code starts again at once`, async () => {
    const app = await at();
    await toCode(app);
    await app.type("id:gate-code-boxes", "111111");
    await app.waitFor(`text:${EXPIRED_CODE}`);
    await app.waitUntil(() => app.enabled("id:gate-resend"));
    expect(app.texts()).toContain("Resend");
    const again = await at();
    await toCode(again);
    await again.type("id:gate-code-boxes", "999999");
    await again.waitFor(`text:${TOO_MANY}`);
    expect(again.shows("id:gate-code-boxes")).toBe(false);
    // The dead code holds nothing back: one action, a new request for the same address at once.
    expect(again.shows("id:gate-resend")).toBe(false);
    expect(again.shows("id:gate-change-email")).toBe(false);
    expect(again.texts()).toContain("Send a new code");
    await again.press("id:gate-new-code");
    await again.waitFor("text:We sent a new code to juana@example.com.");
    expect(again.shows("id:gate-code-boxes")).toBe(true);
    expect(again.texts()).toContain("Resend in 5:00");
    expect(again.enabled("id:gate-resend")).toBe(false);
    expect(calls(again)).toBe("calls: cloud_account_email_start,cloud_account_email_verify 999999,cloud_account_email_start");
  }, 90_000);

  test(`${name}: Resend unlocks when the code expires, sends a new one and locks again`, async () => {
    const app = await at({ expiresIn: 3 });
    await toCode(app);
    expect(app.texts()).toContain("Resend in 0:03");
    expect(app.enabled("id:gate-resend")).toBe(false);
    await app.waitFor(`text:${EXPIRED_CODE}`, { timeout: 8000 });
    expect(app.texts()).toContain("Resend");
    expect(app.enabled("id:gate-resend")).toBe(true);
    await app.press("id:gate-resend");
    await app.waitFor("text:We sent a new code to juana@example.com.");
    expect(app.texts()).toContain("Resend in 0:03");
    expect(app.texts()).not.toContain(EXPIRED_CODE);
    expect(calls(app)).toBe("calls: cloud_account_email_start,cloud_account_email_resend");
  }, 60_000);

  test(`${name}: Resend refused as too early waits as the server says`, async () => {
    const app = await at({ expiresIn: 1, resend: "early" });
    await toCode(app);
    await app.waitUntil(() => app.enabled("id:gate-resend"), { timeout: 5000 });
    await app.press("id:gate-resend");
    await app.waitFor("text:A new code can be sent once this one has expired.");
    expect(app.texts().some((t) => /^Resend in (1:00|0:59)$/.test(t))).toBe(true);
    expect(app.enabled("id:gate-resend")).toBe(false);
  }, 60_000);

  test(`${name}: Change email ends the request and the email page comes back with the address`, async () => {
    const app = await at();
    await toCode(app);
    await app.press("id:gate-change-email");
    await app.waitFor("id:gate-card");
    expect(app.value("id:gate-email")).toBe("juana@example.com");
    expect(calls(app)).toBe("calls: cloud_account_email_start,cloud_account_email_cancel");
    await app.press("id:gate-continue");
    await app.waitFor("id:gate-code-card", { timeout: 15000 });
  }, 60_000);

  test(`${name}: an ended request goes back to the email page with its sentence`, async () => {
    const app = await at();
    await toCode(app);
    await app.type("id:gate-code-boxes", "555555");
    await app.waitFor("text:This sign-in request has ended. Start again.");
    expect(app.shows("id:gate-card")).toBe(true);
  }, 60_000);

  test(`${name}: after a relaunch the gate opens straight on the code page with what is left of the countdown`, async () => {
    const app = await at({ pending: 125, demo: true, mode: "solo", firstRun: false });
    await app.waitFor("id:gate-code-card");
    expect(app.texts()).toContain("We sent a 6-digit code to juana@example.com.");
    expect(app.texts()).toContain("Resend in 2:05");
    expect(app.shows("id:gate-card")).toBe(false);
    await app.type("id:gate-code-boxes", "246810");
    await app.waitFor("id:frame", { timeout: 15000 });
  }, 60_000);

  test(`${name}: the right code goes Home at once`, async () => {
    const app = await at({ demo: true, mode: "solo", firstRun: false });
    await toCode(app);
    await app.type("id:gate-code-boxes", "246810");
    await app.waitFor("id:frame", { timeout: 15000 });
    expect(app.shows("id:gate-root")).toBe(false);
    expect(calls(app)).toBe("calls: cloud_account_email_start,cloud_account_email_verify 246810,cloud_account_session_opened");
  }, 60_000);
}

test("desktop: a new account's code opens the session, then first launch", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { gate: true, readout: true } } });
  await toCode(app);
  await app.type("id:gate-code-boxes", "246810");
  await app.waitFor("id:setup-card", { timeout: 15000 });
}, 60_000);

test("desktop: a signed-in owner with a PIN gets the PIN sign-in next", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { gate: true, demo: true, mode: "solo", adminPin: "1234" } } });
  await toCode(app);
  await app.type("id:gate-code-boxes", "246810");
  await app.waitFor("id:auth-card", { timeout: 15000 });
  expect(app.shows("id:frame")).toBe(false);
}, 60_000);
