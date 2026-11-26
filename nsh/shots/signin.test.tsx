// Real-window screenshots of every page of the Buwiz sign-in (screens/auth/signInFlow.ts): on the account gate the email page (empty,
// a bad address, Continue busy, a refusal, not available, no connection) and the code page (just opened, typing, checking, a wrong
// code with the tries left, expired with Resend unlocked, too many tries, restored after a relaunch, back on it after Change
// email and the same address); Settings > Account's Buwiz account (signed in, its Sign out question, signed out). Light and dark,
// wide and narrow (the narrow looks are the macOS build at phone width: `nsh test --target` draws no phone without a Simulator).
// `just nsh-shots signin` saves ~/BuwizNsh/evidence/signin/*.png. Headless (plain nsh test) it takes no pictures and passes.
import { type App, open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

const GATE = "../AuthDemo.tsx";
const SETTINGS = "../SettingsDemo.tsx";

async function toCode(app: App): Promise<void> {
  await app.type("id:gate-email", "juana@example.com");
  await app.press("id:gate-continue");
  await app.waitFor("id:gate-code-card", { timeout: 15000 });
}

for (const look of LOOKS) {
  const size = { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme };
  const gate = (params: Record<string, unknown> = {}) => open(GATE, { ...size, host: { params: { gate: true, ...params } } });
  const name = (step: string) => `signin-${look.name}-${step}`;

  test(`gate: the email page, empty, a bad address, busy, ${look.name}`, async () => {
    const app = await gate({ slow: true });
    await app.waitFor("id:gate-card");
    await app.wait(400);
    await shoot(app, name("email"));
    await app.type("id:gate-email", "juana@example");
    await app.press("id:gate-continue");
    await app.waitFor("text:That email address does not look right. Check it and try again.");
    await app.wait(300);
    await shoot(app, name("email-bad-address"));
    await app.type("id:gate-email", ".com");
    await app.press("id:gate-continue");
    await app.waitFor("label:Continue with email…");
    await app.wait(300);
    await shoot(app, name("email-busy"));
  }, 120_000);

  test(`gate: the email page refused, not available, no connection, ${look.name}`, async () => {
    for (const [step, email, sentence] of [
      ["email-refused", "refuse", "A sign-in code was already sent to this address. You can ask for a new one in 4 minutes."],
      ["email-not-available", "unavailable", "Sign-in is not available right now. Try again in a few minutes."],
      ["email-no-connection", "offline", "No connection. Check your internet and try again."],
    ] as const) {
      const app = await gate({ email });
      await app.waitFor("id:gate-card");
      await app.type("id:gate-email", "juana@example.com");
      await app.press("id:gate-continue");
      await app.waitFor(`text:${sentence}`);
      await app.wait(300);
      await shoot(app, name(step));
    }
  }, 120_000);

  test(`gate: the code page, typing, wrong, ${look.name}`, async () => {
    const app = await gate();
    await app.waitFor("id:gate-card");
    await toCode(app);
    await app.wait(400);
    await shoot(app, name("code"));
    await app.type("id:gate-code-boxes", "246");
    await app.wait(300);
    await shoot(app, name("code-typing"));
    await app.type("id:gate-code-boxes", "999");
    await app.waitFor("text:That code is not right. 4 tries left.");
    await app.wait(300);
    await shoot(app, name("code-wrong"));
  }, 120_000);

  test(`gate: the code page checking, expired, too many, restored, ${look.name}`, async () => {
    const slow = await gate({ slow: true });
    await slow.waitFor("id:gate-card");
    await toCode(slow);
    await slow.type("id:gate-code-boxes", "123456");
    await slow.waitFor("text:Checking…");
    await slow.wait(300);
    await shoot(slow, name("code-checking"));
    const expired = await gate({ expiresIn: 2 });
    await expired.waitFor("id:gate-card");
    await toCode(expired);
    await expired.waitFor("text:This code has expired. Press Resend for a new one.", { timeout: 8000 });
    await expired.wait(300);
    await shoot(expired, name("code-expired-resend"));
    const many = await gate();
    await many.waitFor("id:gate-card");
    await toCode(many);
    await many.type("id:gate-code-boxes", "999999");
    await many.waitFor("text:Too many wrong codes. Send a new code to try again.");
    await many.wait(300);
    await shoot(many, name("code-too-many"));
    const kept = await gate({ pending: 125 });
    await kept.waitFor("id:gate-code-card");
    await kept.wait(400);
    await shoot(kept, name("code-restored"));
  }, 180_000);

  test(`settings > account: the Buwiz account signed in, its Sign out question, signed out, ${look.name}`, async () => {
    const shot = async (cloud: "in" | "out") => {
      const app = await open(SETTINGS, { ...size, host: { params: { cloud, solo: true, start: "section=account" } } });
      await app.waitFor("id:settings-security", { timeout: 30000 });
      await app.wait(600);
      return app;
    };
    const signedIn = await shot("in");
    await shoot(signedIn, name("settings-account-signed-in"));
    await signedIn.press("id:settings-account-sign-out");
    await signedIn.waitFor("id:settings-sign-out-yes");
    await signedIn.wait(400);
    await shoot(signedIn, name("settings-account-sign-out-question"));
    const signedOut = await shot("out");
    await shoot(signedOut, name("settings-account-signed-out"));
  }, 120_000);

  test(`gate: Change email, then the same address: back on the code page, the countdown going on, ${look.name}`, async () => {
    const app = await gate();
    await app.waitFor("id:gate-card");
    await toCode(app);
    await app.wait(3200);
    await app.press("id:gate-change-email");
    await app.waitFor("id:gate-card");
    await app.wait(300);
    await shoot(app, name("email-after-change"));
    await app.press("id:gate-continue");
    await app.waitFor("id:gate-code-card");
    await app.wait(300);
    await shoot(app, name("code-after-change"));
  }, 120_000);
}
