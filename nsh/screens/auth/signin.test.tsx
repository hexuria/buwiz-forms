// The sign-in screen, at phone size and at desktop size, over the demo taxpayer (Juana Santos) in a database that has an
// admin code: the TIN and PIN checks, the authenticator, the host's wrong-code backoff, "Forgot PIN?" (the admin opens a
// taxpayer; the admin's recovery key; the Mac login, which a test environment refuses), Lock app and Log out, and "Open
// without signing in" when the admin has no code.
import { expect, open, test } from "nsh/test";
import { generatedSecret, totpCode } from "./testTotp";

const ENTRY = "../../AuthDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const SOLO = { demo: true, mode: "solo", adminPin: "1234", readout: true };

test("phone: the owner's TIN and PIN sign in; wrong ones say why and clear the cells", async () => {
  const app = await open(ENTRY, { host: { params: SOLO } });
  await app.waitFor("id:auth-card");
  expect(app.texts()).toContain("Enter your TIN and your PIN.");
  expect(app.shows("id:frame")).toBe(false);
  await app.press("id:auth-submit");
  await app.waitFor("text:Type all 9 digits of the TIN.");
  await app.type("id:auth-tin", "000111229");
  await app.waitFor("text:Type all 9 digits of the TIN.", { state: "gone" });
  await app.type("id:auth-tp-pin", "1234");
  await app.waitFor("text:No taxpayer has that TIN on this device.");
  await app.clear("id:auth-tin");
  await app.type("id:auth-tin", "000123453");
  await app.press("id:auth-submit");
  await app.waitFor("text:Type all 4 digits of the PIN.");
  // The last digit submits.
  await app.type("id:auth-tp-pin", "9999");
  await app.waitFor("text:Wrong TIN or PIN. Try again.");
  expect(app.value("id:auth-tp-pin")).toBe("");
  await app.type("id:auth-tp-pin", "1234");
  await app.waitFor("id:frame");
  await app.waitFor("text:session: admin");
});

test("desktop: the TIN is nine cells in three groups; the ninth digit moves the keys to the PIN and Backspace in an empty PIN moves them back", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: SOLO } });
  await app.waitFor("id:auth-card");
  // 000-000-000 as cells with a dash between the groups (the old DigitCells), not one text box.
  expect(app.shows("id:auth-tin-dash-1")).toBe(true);
  expect(app.shows("id:auth-tin-dash-2")).toBe(true);
  expect(app.shows("id:auth-tin-dash-3")).toBe(false);
  await app.type("id:auth-tin", "000-123-453");
  expect(app.value("id:auth-tin")).toBe("000123453");
  // The ninth digit handed the keys on: the next digits are the PIN's.
  await app.keyboard.press("1");
  await app.keyboard.press("2");
  expect(app.value("id:auth-tp-pin")).toBe("12");
  await app.keyboard.press("backspace");
  await app.keyboard.press("backspace");
  expect(app.value("id:auth-tp-pin")).toBe("");
  // Backspace in the empty PIN goes back to the TIN: a digit typed now is the TIN's, not the PIN's.
  await app.keyboard.press("backspace");
  await app.keyboard.press("7");
  expect(app.value("id:auth-tp-pin")).toBe("");
  expect(app.value("id:auth-tin")).toBe("000123453");
  // The keys are in the TIN: Backspace takes its last digit, the ninth again moves them on to the PIN.
  await app.keyboard.press("backspace");
  expect(app.value("id:auth-tin")).toBe("00012345");
  await app.keyboard.press("3");
  expect(app.value("id:auth-tin")).toBe("000123453");
  await app.keyboard.press("1");
  expect(app.value("id:auth-tp-pin")).toBe("1");
});

test("desktop: six wrong PINs start the host's wait, which even the right PIN waits out", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: SOLO } });
  await app.waitFor("id:auth-card");
  await app.type("id:auth-tin", "000123453");
  for (let i = 0; i < 5; i++) {
    await app.type("id:auth-tp-pin", "0000");
    await app.waitFor("text:Wrong TIN or PIN. Try again.");
  }
  await app.type("id:auth-tp-pin", "0000");
  await app.waitFor("text:/^Too many wrong tries\. Try again in /");
  await app.type("id:auth-tp-pin", "1234");
  await app.waitFor("text:/^Too many wrong tries\. Try again in /");
  expect(app.shows("id:frame")).toBe(false);
});

test("desktop: an authenticator code signs a taxpayer in; Lock app and Log out bring the sign-in back", async () => {
  const secret = generatedSecret();
  const app = await open(ENTRY, { ...WIDE, host: { params: { demo: true, second: true, mode: "helpdesk", adminPin: "1234", totps: { "000123453": secret }, readout: true } } });
  await app.waitFor("id:auth-card");
  expect(app.texts()).toContain("Enter your TIN and your PIN. The help desk admin signs in with TIN 000-000-000.");
  await app.press("id:auth-method");
  await app.waitFor("id:auth-tp-totp");
  expect(app.texts()).toContain("Use the PIN instead");
  await app.type("id:auth-tin", "000123453");
  await app.type("id:auth-tp-totp", "000000");
  await app.waitFor("text:Wrong TIN or code. Try again.");
  await app.type("id:auth-tp-totp", totpCode(secret));
  await app.waitFor("id:frame");
  await app.waitFor("text:session: taxpayer 000123453");
  expect(app.texts()).toContain("TIN 000-123-453");
  // Lock app (Cmd+L): the sign-in again, with the PIN cells first.
  await app.keyboard.press("mod+l");
  await app.waitFor("id:auth-card");
  expect(app.shows("id:auth-tp-pin")).toBe(true);
  expect(app.texts()).toContain("session: open");
  // A taxpayer with no PIN cannot sign in with one.
  await app.type("id:auth-tin", "000111229");
  await app.type("id:auth-tp-pin", "1111");
  await app.waitFor("text:That taxpayer has no PIN yet. Ask the admin to open it.");
  // The help desk admin.
  await app.clear("id:auth-tin");
  await app.type("id:auth-tin", "000000000");
  await app.type("id:auth-tp-pin", "1234");
  await app.waitFor("text:session: admin");
  await app.press("id:account-row");
  await app.press("id:menu-logout");
  await app.waitFor("id:auth-card");
  await app.waitFor("text:Signed out.");
});

test("desktop: Forgot PIN? lets the admin open a taxpayer, and the admin in with the recovery key", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { demo: true, mode: "helpdesk", adminPin: "1234", pins: { "000123453": "4321" }, readout: true } } });
  await app.waitFor("id:auth-card");
  await app.press("id:auth-forgot");
  await app.waitFor("text:The admin can open your account with their PIN.");
  expect(app.shows("id:auth-method")).toBe(false);
  expect(app.texts()).toContain("I remember my PIN");
  await app.type("id:auth-tin", "000123453");
  await app.type("id:auth-admin-pin", "9999");
  await app.waitFor("text:Wrong admin PIN or code. Try again.");
  await app.type("id:auth-admin-pin", "1234");
  await app.waitFor("text:session: taxpayer 000123453");
  await app.waitFor("text:Opened by the admin. Set a new PIN in Settings.");
  await app.keyboard.press("mod+l");
  await app.waitFor("id:auth-card");
  // The admin's own TIN: the recovery key (and the Mac login where there is one).
  await app.press("id:auth-forgot");
  await app.type("id:auth-tin", "000000000");
  await app.waitFor("id:auth-recovery");
  expect(app.texts()).toContain("Use your recovery key, or your Mac login.");
  await app.press("id:auth-recovery-submit");
  await app.waitFor("text:Type the recovery key you wrote down when the admin PIN was set.");
  await app.type("id:auth-recovery", "AAAA-BBBB-CCCC-DDDD");
  await app.press("id:auth-recovery-submit");
  await app.waitFor("text:That recovery key does not match. Check it and try again.");
  if (app.shows("id:auth-mac-login")) {
    // A test environment never asks the OS: the host answers no.
    await app.press("id:auth-mac-login");
    await app.waitFor("text:Mac login was cancelled or did not match.");
  }
  const key = app.texts().find((t) => t.startsWith("key: "))!.slice(5);
  await app.type("id:auth-recovery", key);
  await app.press("id:auth-recovery-submit");
  await app.waitFor("text:session: admin");
  await app.waitFor("text:Recovery key used. It works only once: set a new admin PIN and make a new key.");
});

test("phone: with no admin code the sign-in offers Open without signing in, and Lock app says to set a PIN first", async () => {
  const app = await open(ENTRY, { host: { params: { demo: true, mode: "solo", lock: true, readout: true } } });
  await app.waitFor("id:auth-card");
  await app.press("id:auth-open");
  await app.waitFor("id:frame");
  expect(app.texts()).toContain("session: open");
  await app.keyboard.press("mod+l");
  await app.waitFor("text:Set your PIN in Settings first. With no PIN there is nothing to sign back in with.");
  expect(app.shows("id:auth-card")).toBe(false);
});
