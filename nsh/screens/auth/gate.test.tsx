// The Buwiz account gate, at phone size and at desktop size. It opens on the first page (the Buwiz Forms logo, one line,
// "Continue with email"), which opens the email page (one page for signing in and creating an account): Continue sends a code
// and the code page opens (six boxes that check by themselves, "Send a new code" with its countdown, the back control that is
// Change email); the right code goes Home (or the first-run setup on a first run), with a welcome line the first time. Nothing
// else, and never a browser. A test never reaches the network: the host's answers are the fixture's (screens/auth/testEmail.ts).
// The dev environment's own bypass (EBIR_TEST_ENV with BIR_AUTH_BYPASS) is what every other test runs on.
import { type App, expect, open, test } from "nsh/test";

const ENTRY = "../../AuthDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const SIZES = [
  { name: "phone", size: {}, device: "this device" },
  { name: "desktop", size: WIDE, device: "this computer" },
] as const;

const EMAIL_SENTENCE = "Enter your email and we will send you a 6-digit code to sign in, or to create your Buwiz account if you are new.";
const EXPIRED_CODE = "This code has expired. Send a new code to try again.";
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

/** The first page's "Continue with email": the email page opens. */
async function toEmail(app: App): Promise<void> {
  await app.waitFor("id:gate-start");
  await app.press("id:gate-start-continue");
  await app.waitFor("id:gate-email-page");
}

/** The code page says where the code went: the sentence, then the address on its own line. */
function sentTo(app: App, address: string): boolean {
  return app.texts().includes("We sent a 6-digit code to") && app.texts().includes(address);
}

/** Opens the email page, types the address and presses Continue; the code page opens. */
async function toCode(app: App, address = "juana@example.com"): Promise<void> {
  await toEmail(app);
  await app.type("id:gate-email", address);
  await app.press("id:gate-continue");
  await app.waitFor("id:gate-code-page", { timeout: 15000 });
}

/**
 * Types the digits as keys only, with no press on the boxes first (app.type would press them to take the keys): they reach the
 * boxes only if the boxes have the focus.
 */
async function typeKeys(app: App, digits: string): Promise<void> {
  for (const digit of digits) await app.keyboard.press(digit);
}

for (const { name, size, device } of SIZES) {
  const at = (params: Record<string, unknown> = {}) => open(ENTRY, { ...size, host: { params: { gate: true, readout: true, ...params } } });

  test(`${name}: the gate opens on the first page: the logo, one line and one button, nothing else`, async () => {
    const app = await at();
    await app.waitFor("id:gate-start");
    expect(app.shows("id:brand-logo")).toBe(true);
    expect(app.count("id:brand-logo")).toBe(1);
    for (const t of ["Prepare, file and print your BIR returns.", "Continue with email"]) expect(app.texts()).toContain(t);
    // No name line (the logo carries it), no other way to sign in, no field yet.
    for (const t of ["eBIRForms", "Sign in with Apple", "Sign in with Google", "Ways to sign in"]) expect(app.texts()).not.toContain(t);
    expect(app.shows("id:gate-email")).toBe(false);
    expect(app.shows("id:gate-back")).toBe(false);
    expect(app.shows("id:frame")).toBe(false);
    expectClean(app.texts());
    expect(calls(app)).toBe("calls: none");
  }, 60_000);

  test(`${name}: Continue with email opens the email page: the logo, its title, one sentence, the field and one button`, async () => {
    const app = await at();
    await toEmail(app);
    expect(app.shows("id:brand-logo")).toBe(true);
    for (const t of ["Enter your email", EMAIL_SENTENCE, "Email", "Continue", "Back"]) expect(app.texts()).toContain(t);
    expect(app.shows("id:gate-start")).toBe(false);
    expect(app.shows("id:frame")).toBe(false);
    expect(app.shows("id:setup-card")).toBe(false);
    expect(app.shows("id:gate-code-page")).toBe(false);
    expect(app.enabled("id:gate-continue")).toBe(true);
    expect(app.texts()).not.toContain("or");
    expectClean(app.texts());
    expect(calls(app)).toBe("calls: none");
  }, 60_000);

  test(`${name}: the logo is on the first page, the email page and the code page`, async () => {
    const app = await at();
    await app.waitFor("id:gate-start");
    expect(app.shows("id:brand-logo")).toBe(true);
    await toEmail(app);
    expect(app.shows("id:brand-logo")).toBe(true);
    await app.type("id:gate-email", "juana@example.com");
    await app.press("id:gate-continue");
    await app.waitFor("id:gate-code-page", { timeout: 15000 });
    expect(app.shows("id:brand-logo")).toBe(true);
    expect(app.count("id:brand-logo")).toBe(1);
  }, 60_000);

  test(`${name}: the email page's back control goes to the first page; Continue with email comes back with the address kept`, async () => {
    const app = await at();
    await toEmail(app);
    await app.type("id:gate-email", "juana@example");
    await app.press("id:gate-continue");
    await app.waitFor("text:That email address does not look right. Check it and try again.");
    await app.press("id:gate-back");
    await app.waitFor("id:gate-start");
    expect(app.shows("id:gate-email-page")).toBe(false);
    await app.press("id:gate-start-continue");
    await app.waitFor("id:gate-email-page");
    expect(app.value("id:gate-email")).toBe("juana@example");
    // The sentence went with the page.
    expect(app.shows("id:gate-email-said")).toBe(false);
    expect(calls(app)).toBe("calls: none");
  }, 60_000);

  test(`${name}: the code page's back control (Change email) returns to the email page with the address kept`, async () => {
    const app = await at();
    await toCode(app);
    expect(app.texts()).toContain("Change email");
    await app.press("id:gate-change-email");
    await app.waitFor("id:gate-email-page");
    expect(app.shows("id:gate-code-page")).toBe(false);
    expect(app.value("id:gate-email")).toBe("juana@example.com");
    expect(calls(app)).toBe("calls: cloud_account_email_start");
  }, 60_000);

  test(`${name}: the welcome line shows on the first page after the first sign-in, and not after the second`, async () => {
    const app = await at({ demo: true, mode: "solo", firstRun: false, signOut: true });
    await toCode(app);
    await app.type("id:gate-code-boxes", "246810");
    await app.waitFor("id:frame", { timeout: 15000 });
    await app.waitFor("id:welcome-note");
    expect(app.texts()).toContain("Welcome. Your Buwiz account is ready.");
    // Signed out and in again on this device: no welcome.
    await app.press("id:demo-sign-out");
    await app.waitFor("id:gate-start", { timeout: 15000 });
    await toEmail(app);
    await app.clear("id:gate-email");
    await app.type("id:gate-email", "juana@example.com");
    await app.press("id:gate-continue");
    await app.waitFor("id:gate-code-page", { timeout: 15000 });
    await app.type("id:gate-code-boxes", "246810");
    await app.waitFor("id:frame", { timeout: 15000 });
    await app.wait(500);
    expect(app.shows("id:welcome-note")).toBe(false);
  }, 90_000);

  test(`${name}: an empty or bad address is said under the field without a request`, async () => {
    const app = await at();
    await toEmail(app);
    await app.press("id:gate-continue");
    await app.waitFor("text:Enter your email address.");
    await app.type("id:gate-email", "juana@example");
    await app.waitFor("text:Enter your email address.", { state: "gone" });
    await app.press("id:gate-continue");
    await app.waitFor("text:That email address does not look right. Check it and try again.");
    expect(app.shows("id:gate-email-page")).toBe(true);
    expect(calls(app)).toBe("calls: none");
  }, 60_000);

  test(`${name}: Continue is busy only while it sends; a refusal is said and the page stays`, async () => {
    const app = await at({ email: "refuse" });
    await toEmail(app);
    await app.type("id:gate-email", "juana@example.com");
    await app.press("id:gate-continue");
    await app.waitFor("label:Continue…");
    expect(app.texts()).not.toContain("Waiting…");
    await app.waitFor("text:A sign-in code was already sent to this address. You can ask for a new one in 4 minutes.");
    expect(app.shows("label:Continue…")).toBe(false);
    expect(app.shows("id:gate-email-page")).toBe(true);
    expect(app.enabled("id:gate-continue")).toBe(true);
    expect(calls(app)).toBe("calls: cloud_account_email_start");
  }, 60_000);

  for (const [kind, sentence] of [
    ["unavailable", "Sign-in is not available right now. Try again in a few minutes."],
    ["offline", "No connection. Check your internet and try again."],
    ["throttled", "Too many tries. Wait a moment and try again."],
  ] as const) {
    test(`${name}: ${kind}: the email page says so and stays; nothing opens`, async () => {
      const app = await at({ email: kind });
      await toEmail(app);
      await app.type("id:gate-email", "juana@example.com");
      await app.press("id:gate-continue");
      await app.waitFor(`text:${sentence}`);
      await app.wait(500);
      expect(app.shows("id:gate-email-page")).toBe(true);
      expect(calls(app)).toBe("calls: cloud_account_email_start");
      expect(app.openedUrls()).toEqual([]);
      expectClean(app.texts());
    }, 60_000);
  }

  test(`${name}: the code page: where the code went, Send a new code locked with its countdown, Change email, and no promise about the mail's button yet`, async () => {
    const app = await at();
    await toCode(app);
    expect(sentTo(app, "juana@example.com")).toBe(true);
    for (const t of ["Check your email", "Send a new code in 5:00", "Change email"]) {
      expect(app.texts()).toContain(t);
    }
    // The button in the mail is not handled by the app yet: the page must not promise it.
    expect(app.texts().join(" ")).not.toContain("button in the email");
    expect(app.enabled("id:gate-resend")).toBe(false);
    expect(app.shows("id:gate-continue")).toBe(false);
    expectClean(app.texts());
    await app.wait(1200);
    expect(app.texts()).toContain("Send a new code in 4:59");
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
    // One check per paste, never a burst.
    expect(calls(app).split(",").filter((c) => c.startsWith("cloud_account_email_verify")).length).toBe(4);
  }, 90_000);

  test(`${name}: an expired code says so and Resend unlocks; after too many tries Send a new code starts again at once`, async () => {
    const app = await at();
    await toCode(app);
    await app.type("id:gate-code-boxes", "111111");
    await app.waitFor(`text:${EXPIRED_CODE}`);
    await app.waitUntil(() => app.enabled("id:gate-resend"));
    expect(app.texts()).toContain("Send a new code");
    const again = await at();
    await toCode(again);
    await again.type("id:gate-code-boxes", "999999");
    await again.waitFor(`text:${TOO_MANY}`);
    // The boxes stay where they were, spent; the dead code holds nothing back: the same control, now "Send a new code", starts
    // a new request for the same address at once. Change email stays.
    expect(again.shows("id:gate-code-boxes")).toBe(true);
    expect(again.enabled("id:gate-code-boxes")).toBe(false);
    expect(again.enabled("id:gate-resend")).toBe(true);
    expect(again.shows("id:gate-change-email")).toBe(true);
    expect(again.texts()).toContain("Send a new code");
    await again.press("id:gate-resend");
    await again.waitFor("text:We sent a new code to juana@example.com.");
    expect(again.shows("id:gate-code-boxes")).toBe(true);
    expect(again.texts()).toContain("Send a new code in 5:00");
    expect(again.enabled("id:gate-resend")).toBe(false);
    expect(calls(again)).toBe("calls: cloud_account_email_start,cloud_account_email_verify 999999,cloud_account_email_start");
  }, 90_000);

  test(`${name}: Resend unlocks when the code expires, sends a new one and locks again`, async () => {
    const app = await at({ expiresIn: 3 });
    await toCode(app);
    expect(app.texts()).toContain("Send a new code in 0:03");
    expect(app.enabled("id:gate-resend")).toBe(false);
    await app.waitFor(`text:${EXPIRED_CODE}`, { timeout: 8000 });
    expect(app.texts()).toContain("Send a new code");
    expect(app.enabled("id:gate-resend")).toBe(true);
    await app.press("id:gate-resend");
    await app.waitFor("text:We sent a new code to juana@example.com.");
    expect(app.texts()).toContain("Send a new code in 0:03");
    expect(app.texts()).not.toContain(EXPIRED_CODE);
    expect(calls(app)).toBe("calls: cloud_account_email_start,cloud_account_email_resend");
  }, 60_000);

  test(`${name}: Resend refused as too early waits as the server says`, async () => {
    const app = await at({ expiresIn: 1, resend: "early" });
    await toCode(app);
    await app.waitUntil(() => app.enabled("id:gate-resend"), { timeout: 5000 });
    await app.press("id:gate-resend");
    await app.waitFor("text:A new code can be sent once this one has expired.");
    expect(app.texts().some((t) => /^Send a new code in (1:00|0:59)$/.test(t))).toBe(true);
    expect(app.enabled("id:gate-resend")).toBe(false);
  }, 60_000);

  for (const [blocked, sentence] of [
    ["html", "Sign-in is not available right now. Try again in a few minutes."],
    ["throttled", "Too many tries. Wait a moment and try again."],
  ] as const) {
    test(`${name}: ${blocked}: the check and Resend say so, keep what was typed, and stay on the code page`, async () => {
      const app = await at({ blocked, expiresIn: 1 });
      await toCode(app);
      await app.type("id:gate-code-boxes", "123456");
      await app.waitFor(`text:${sentence}`);
      expect(app.value("id:gate-code-boxes")).toBe("123456");
      expect(app.shows("id:gate-code-page")).toBe(true);
      await app.waitUntil(() => app.enabled("id:gate-resend"), { timeout: 5000 });
      await app.press("id:gate-resend");
      await app.waitUntil(() => calls(app).includes("cloud_account_email_resend"));
      await app.waitFor(`text:${sentence}`);
      expect(app.shows("id:gate-code-page")).toBe(true);
      expect(calls(app)).toBe("calls: cloud_account_email_start,cloud_account_email_verify 123456,cloud_account_email_resend");
    }, 60_000);
  }

  test(`${name}: no bursts: the boxes check once while a check runs; Resend and Send a new code send once however often pressed`, async () => {
    const app = await at({ slow: true, expiresIn: 1 });
    await toCode(app);
    await app.type("id:gate-code-boxes", "123456");
    await app.waitFor("text:Checking…");
    // The boxes are locked while the check runs: more keys do nothing.
    await app.type("id:gate-code-boxes", "7").catch(() => undefined);
    await app.waitFor("text:That code is not right. 4 tries left.", { timeout: 10000 });
    const verifies = () => calls(app).split(",").filter((c) => c.startsWith("cloud_account_email_verify")).length;
    expect(verifies()).toBe(1);
    await app.waitUntil(() => app.enabled("id:gate-resend"), { timeout: 5000 });
    await app.press("id:gate-resend");
    await app.press("id:gate-resend");
    await app.press("id:gate-resend");
    await app.waitFor("text:We sent a new code to juana@example.com.", { timeout: 10000 });
    expect(calls(app).split(",").filter((c) => c === "cloud_account_email_resend").length).toBe(1);
    const many = await at({ slow: true });
    await toCode(many);
    await many.type("id:gate-code-boxes", "999999");
    await many.waitFor(`text:${TOO_MANY}`, { timeout: 10000 });
    await many.press("id:gate-resend");
    await many.press("id:gate-resend");
    await many.waitFor("text:We sent a new code to juana@example.com.", { timeout: 10000 });
    expect(calls(many).replace("calls: ", "").split(",").filter((c) => c === "cloud_account_email_start").length).toBe(2);
  }, 90_000);

  test(`${name}: Change email keeps the request; the same address goes straight back to the code page, nothing sent, the countdown going on`, async () => {
    const app = await at();
    await toCode(app);
    await app.wait(2200);
    await app.press("id:gate-change-email");
    await app.waitFor("id:gate-email-page");
    // The address it was sent to, editable.
    expect(app.value("id:gate-email")).toBe("juana@example.com");
    expect(app.enabled("id:gate-email")).toBe(true);
    await app.press("id:gate-continue");
    await app.waitFor("id:gate-code-page");
    expect(app.texts().some((t) => /^Send a new code in 4:5\d$/.test(t))).toBe(true);
    expect(app.texts()).not.toContain("Send a new code in 5:00");
    // The same address typed again in another case, with spaces: the same request.
    await app.press("id:gate-change-email");
    await app.clear("id:gate-email");
    await app.type("id:gate-email", "  Juana@Example.COM ");
    await app.press("id:gate-continue");
    await app.waitFor("id:gate-code-page");
    expect(sentTo(app, "juana@example.com")).toBe(true);
    // One start, never a cancel: no second mail, nothing thrown away.
    expect(calls(app)).toBe("calls: cloud_account_email_start");
    // The kept code still signs in.
    await app.type("id:gate-code-boxes", "246810");
    await app.waitUntil(() => calls(app).includes("cloud_account_session_opened"), { timeout: 15000 });
  }, 60_000);

  test(`${name}: Change email, then a different address starts a new request; back to the first address starts another, never the old one`, async () => {
    const app = await at();
    await toCode(app);
    await app.press("id:gate-change-email");
    await app.clear("id:gate-email");
    await app.type("id:gate-email", "pedro@example.com");
    await app.press("id:gate-continue");
    await app.waitFor("id:gate-code-page", { timeout: 15000 });
    expect(sentTo(app, "pedro@example.com")).toBe(true);
    expect(app.texts()).toContain("Send a new code in 5:00");
    expect(calls(app)).toBe("calls: cloud_account_email_start,cloud_account_email_start");
    await app.press("id:gate-change-email");
    await app.clear("id:gate-email");
    await app.type("id:gate-email", "juana@example.com");
    await app.press("id:gate-continue");
    await app.waitFor("id:gate-code-page", { timeout: 15000 });
    expect(sentTo(app, "juana@example.com")).toBe(true);
    expect(app.texts()).toContain("Send a new code in 5:00");
    expect(calls(app)).toBe("calls: cloud_account_email_start,cloud_account_email_start,cloud_account_email_start");
  }, 60_000);

  test(`${name}: code_already_sent with no request kept here: the sentence with its time, the field still editable, Continue still there`, async () => {
    const app = await at({ email: "refuse" });
    await toEmail(app);
    await app.type("id:gate-email", "juana@example.com");
    await app.press("id:gate-continue");
    await app.waitFor("text:A sign-in code was already sent to this address. You can ask for a new one in 4 minutes.");
    expect(app.enabled("id:gate-email")).toBe(true);
    expect(app.enabled("id:gate-continue")).toBe(true);
    await app.clear("id:gate-email");
    await app.type("id:gate-email", "pedro@example.com");
    expect(app.value("id:gate-email")).toBe("pedro@example.com");
    // Typing clears the sentence; nothing was sent but the one start.
    await app.waitFor("text:A sign-in code was already sent to this address. You can ask for a new one in 4 minutes.", { state: "gone" });
    expect(calls(app)).toBe("calls: cloud_account_email_start");
  }, 60_000);

  test(`${name}: the line under the field and under the boxes keeps its room before and after a sentence`, async () => {
    const app = await at();
    await toEmail(app);
    expect(app.shows("id:gate-email-slot")).toBe(true);
    expect(app.shows("id:gate-email-said")).toBe(false);
    await app.press("id:gate-continue");
    await app.waitFor("id:gate-email-said");
    expect(app.shows("id:gate-email-slot")).toBe(true);
    await app.type("id:gate-email", "juana@example.com");
    await app.press("id:gate-continue");
    await app.waitFor("id:gate-code-page", { timeout: 15000 });
    expect(app.shows("id:gate-code-slot")).toBe(true);
    expect(app.shows("id:gate-code-said")).toBe(false);
    await app.type("id:gate-code-boxes", "123456");
    await app.waitFor("text:That code is not right. 4 tries left.");
    expect(app.shows("id:gate-code-slot")).toBe(true);
    expect(app.shows("id:gate-code-said")).toBe(true);
    // Every page of the gate is in a scroll view (what NSH pads and scrolls when the keyboard covers a field).
    expect(app.shows("id:auth-scroll")).toBe(true);
  }, 60_000);

  test(`${name}: one control gets a new code in every state, in the same place: locked, unlocked, after too many tries`, async () => {
    const app = await at({ expiresIn: 2 });
    await toCode(app);
    expect(app.count("id:gate-resend")).toBe(1);
    expect(app.texts()).toContain("Send a new code in 0:02");
    expect(app.enabled("id:gate-resend")).toBe(false);
    await app.waitFor("text:Send a new code", { timeout: 8000 });
    expect(app.count("id:gate-resend")).toBe(1);
    expect(app.enabled("id:gate-resend")).toBe(true);
    const many = await at();
    await toCode(many);
    await many.type("id:gate-code-boxes", "999999");
    await many.waitFor(`text:${TOO_MANY}`);
    expect(many.count("id:gate-resend")).toBe(1);
    expect(many.texts()).toContain("Send a new code");
    expect(many.enabled("id:gate-resend")).toBe(true);
    expect(many.shows("id:gate-new-code")).toBe(false);
    for (const page of [app, many]) {
      expect(page.shows("id:gate-change-email")).toBe(true);
    }
  }, 60_000);

  test(`${name}: the gate made anew asks the host for the kept request and opens the code page with the countdown going on`, async () => {
    const app = await at({ drop: true });
    await toCode(app);
    await app.wait(2200);
    const reads = () => Number((app.texts().find((t) => t.startsWith("pending reads: ")) ?? "pending reads: -1").slice("pending reads: ".length));
    const before = reads();
    // The flow's state in memory is gone (as in an engine made anew): the gate asks the host, once, and the code page is back.
    await app.press("id:demo-drop-sign-in");
    await app.waitFor("id:gate-code-page");
    expect(reads()).toBe(before + 1);
    expect(app.texts().some((t) => /^Send a new code in 4:5\d$/.test(t))).toBe(true);
    expect(sentTo(app, "juana@example.com")).toBe(true);
    // Nothing reached the server again.
    expect(calls(app)).toBe("calls: cloud_account_email_start");
  }, 60_000);

  test(`${name}: an ended request goes back to the email page with its sentence`, async () => {
    const app = await at();
    await toCode(app);
    await app.type("id:gate-code-boxes", "555555");
    await app.waitFor("text:This sign-in request has ended. Start again.");
    expect(app.shows("id:gate-email-page")).toBe(true);
  }, 60_000);

  test(`${name}: after a relaunch the gate opens straight on the code page with what is left of the countdown`, async () => {
    const app = await at({ pending: 125, demo: true, mode: "solo", firstRun: false });
    await app.waitFor("id:gate-code-page");
    expect(app.texts()).toContain("pending reads: 1");
    expect(calls(app)).toBe("calls: none");
    expect(sentTo(app, "juana@example.com")).toBe(true);
    expect(app.texts()).toContain("Send a new code in 2:05");
    expect(app.shows("id:gate-email-page")).toBe(false);
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

  test(`${name}: signing in puts the keyboard down, on the way Home and on the way to the first-run question`, async () => {
    for (const params of [{ demo: true, mode: "solo", firstRun: false }, {}]) {
      // Keyboard.dismiss is the framework's (nsh); its stand-in counts the calls.
      let dismissed = 0;
      const app = await open(ENTRY, {
        ...size,
        host: { params: { gate: true, readout: true, ...params } },
        modules: { nsh: { "Keyboard.dismiss": () => void dismissed++ } },
      });
      await toCode(app);
      await app.type("id:gate-code-boxes", "12345");
      expect(dismissed).toBe(0);
      await app.type("id:gate-code-boxes", "6");
      await app.waitFor("text:That code is not right. 4 tries left.");
      expect(dismissed).toBe(0);
      await app.clear("id:gate-code-boxes");
      await app.type("id:gate-code-boxes", "246810");
      await app.waitFor(params.firstRun === false ? "id:frame" : "id:setup-card", { timeout: 15000 });
      expect(dismissed).toBe(1);
    }
  }, 120_000);

  test(`${name}: back from Change email the address field has the focus (its address selected, so typing replaces it)`, async () => {
    const app = await at();
    await toCode(app);
    await app.press("id:gate-change-email");
    await app.waitFor("id:gate-email-page");
    expect(app.value("id:gate-email")).toBe("juana@example.com");
    // A key goes to the focused input, and the field was not pressed: it has the focus. Headless inputs keep no selection
    // (nsh test docs, app.type), so here the key is added at the end; on a device it replaces the selected address.
    await app.keyboard.press("x");
    expect(app.value("id:gate-email")).toBe("juana@example.comx");
  }, 60_000);

  test(`${name}: the code page opens with the keys in the boxes: six digits typed with no tap sign in and go Home`, async () => {
    const app = await at({ demo: true, mode: "solo", firstRun: false });
    await toCode(app);
    // The first key lands in the boxes, though nothing pressed them: they have the focus.
    await typeKeys(app, "2");
    expect(app.value("id:gate-code-boxes")).toBe("2");
    await typeKeys(app, "46810");
    await app.waitFor("id:frame", { timeout: 15000 });
    expect(calls(app)).toBe("calls: cloud_account_email_start,cloud_account_email_verify 246810,cloud_account_session_opened");
  }, 60_000);

  test(`${name}: after a wrong code empties the boxes, six digits typed with no tap sign in`, async () => {
    const app = await at({ demo: true, mode: "solo", firstRun: false });
    await toCode(app);
    await app.type("id:gate-code-boxes", "123456");
    await app.waitFor("text:That code is not right. 4 tries left.", { timeout: 10000 });
    expect(app.value("id:gate-code-boxes") ?? "").toBe("");
    await typeKeys(app, "246810");
    await app.waitFor("id:frame", { timeout: 15000 });
  }, 60_000);

  test(`${name}: after Resend, six digits typed with no tap sign in`, async () => {
    const app = await at({ demo: true, mode: "solo", firstRun: false, expiresIn: 1 });
    await toCode(app);
    await app.waitUntil(() => app.enabled("id:gate-resend"), { timeout: 5000 });
    await app.press("id:gate-resend");
    await app.waitFor("text:We sent a new code to juana@example.com.");
    // The boxes take the focus one tick after the new code is out (focusWhenOpen): a person reads the line before typing, so the
    // test lets the app run that tick (no tap, no press on the boxes) rather than typing in the same instant.
    await app.waitUntil(() => app.enabled("id:gate-code-boxes"));
    await app.wait(50);
    await typeKeys(app, "246810");
    await app.waitFor("id:frame", { timeout: 15000 });
  }, 60_000);

  test(`${name}: after too many tries and Send a new code, six digits typed with no tap sign in`, async () => {
    const app = await at({ demo: true, mode: "solo", firstRun: false });
    await toCode(app);
    await app.type("id:gate-code-boxes", "999999");
    await app.waitFor(`text:${TOO_MANY}`);
    await app.press("id:gate-resend");
    await app.waitFor("text:We sent a new code to juana@example.com.");
    // The boxes take the focus one tick after the new code is out (focusWhenOpen): a person reads the line before typing, so the
    // test lets the app run that tick (no tap, no press on the boxes) rather than typing in the same instant.
    await app.waitUntil(() => app.enabled("id:gate-code-boxes"));
    await app.wait(50);
    await typeKeys(app, "246810");
    await app.waitFor("id:frame", { timeout: 15000 });
  }, 60_000);

  test(`${name}: spent boxes kept through Change email take the focus once Send a new code opens them: no tap`, async () => {
    const app = await at({ demo: true, mode: "solo", firstRun: false });
    await toCode(app);
    await app.type("id:gate-code-boxes", "999999");
    await app.waitFor(`text:${TOO_MANY}`);
    await app.press("id:gate-change-email");
    await app.waitFor("id:gate-email-page");
    await app.press("id:gate-continue");
    await app.waitFor(`text:${TOO_MANY}`);
    // The page opened spent: nothing took the focus then.
    expect(app.enabled("id:gate-code-boxes")).toBe(false);
    await app.press("id:gate-resend");
    await app.waitFor("text:We sent a new code to juana@example.com.");
    // The boxes take the focus one tick after the new code is out (focusWhenOpen): a person reads the line before typing, so the
    // test lets the app run that tick (no tap, no press on the boxes) rather than typing in the same instant.
    await app.waitUntil(() => app.enabled("id:gate-code-boxes"));
    await app.wait(50);
    await typeKeys(app, "246810");
    await app.waitFor("id:frame", { timeout: 15000 });
  }, 60_000);

  test(`${name}: the network's limit on code checks and a busy server are said with no try spent, and the same request signs in after`, async () => {
    const app = await at({ demo: true, mode: "solo", firstRun: false });
    await toCode(app);
    await app.type("id:gate-code-boxes", "777777");
    await app.waitFor("text:Too many sign-in codes are being checked right now. Try again in 3 minutes.");
    expect(app.texts().some((t) => t.includes("tries left"))).toBe(false);
    expect(app.value("id:gate-code-boxes")).toBe("");
    await app.type("id:gate-code-boxes", "888888");
    await app.waitFor("text:Sign-in is busy right now. Try again in a moment.");
    expect(app.value("id:gate-code-boxes")).toBe("");
    await app.type("id:gate-code-boxes", "246810");
    await app.waitFor("id:frame", { timeout: 15000 });
    expect(calls(app)).toBe(
      "calls: cloud_account_email_start,cloud_account_email_verify 777777,cloud_account_email_verify 888888,cloud_account_email_verify 246810,cloud_account_session_opened",
    );
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

test("desktop: signed out as the menu's Sign Out does, the gate opens on its first page; Continue, then Back, returns to it", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { gate: true, demo: true, mode: "solo", firstRun: false, signOut: true, readout: true } } });
  await toCode(app);
  await app.type("id:gate-code-boxes", "246810");
  await app.waitFor("id:frame", { timeout: 15000 });
  // The host's sign-out, then app.lockCloud: nothing else resets the sign-in (shell/desktop/actions.ts signOutOfTheAccount).
  await app.press("id:demo-sign-out");
  await app.waitFor("id:gate-start", { timeout: 15000 });
  expect(app.shows("id:gate-email-page")).toBe(false);
  await app.press("id:gate-start-continue");
  await app.waitFor("id:gate-email-page");
  await app.press("id:gate-back");
  await app.waitFor("id:gate-start");
}, 60_000);
