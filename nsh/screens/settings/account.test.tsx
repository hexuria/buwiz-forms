// Settings > Account at phone size and at desktop size: the Buwiz account (sign in, sign out, the refresh when it opens, an unreachable
// server), who is signed in, the profile card's pencils, Google, and Security (PIN, authenticator, recovery key) for the owner, a help
// desk's admin and a taxpayer. Against a scratch database with the demo taxpayer (SettingsDemo.tsx).
import { expect, test } from "nsh/test";
import { totpCode } from "../auth/testTotp";
import { calls, expectTexts, openSettings, SIZES, state } from "./testing";

const ready = "id:settings-security";

for (const { name, size } of SIZES) {
  test(`${name}: signed out, Sign in shows the browser steps, then who is signed in; the gate opens`, async () => {
    const app = await openSettings(size, { cloud: "out", solo: true }, "account");
    await app.waitFor(ready);
    expectTexts(app, ["Buwiz", "Server", "Set when this app was built.", "https://buwiz.example.test", "Not signed in", "Sign in"]);
    expect(app.shows("id:settings-account-sign-out")).toBe(false);
    expect(app.shows("id:settings-account-delete-button")).toBe(false);
    await app.press("id:settings-account-sign-in");
    await app.waitFor("text:juana@example.com", { timeout: 15000 });
    expect(calls(app)).toEqual(["cloud_account_begin_sign_in", "cloud_account_finish_sign_in", "cloud_account_session_opened"]);
    expect(app.shows("id:settings-account-sign-out")).toBe(true);
    expect(app.shows("id:settings-account-delete-button")).toBe(true);
    expect(app.shows("id:settings-account-sign-in")).toBe(false);
    expect(state(app)).toContain("cloudLocked=false");
  }, 60_000);

  test(`${name}: signed in, the saved sign-in is checked once as the card opens; Sign out locks the gate`, async () => {
    const app = await openSettings(size, { cloud: "in", solo: true }, "account");
    await app.waitFor(ready);
    await app.waitFor("text:calls: cloud_account_refresh");
    expectTexts(app, ["juana@example.com", "Sign out"]);
    await app.press("id:settings-account-sign-out");
    await app.waitFor("text:/cloudLocked=true/");
    expect(calls(app)).toEqual(["cloud_account_refresh", "cloud_account_sign_out", "cloud_account_session_closed"]);
  }, 60_000);

  test(`${name}: a practice account says its returns are simulated; a real account does not`, async () => {
    const app = await openSettings(size, { cloud: "practice", solo: true }, "account");
    await app.waitFor(ready);
    await app.waitFor("id:settings-account-practice");
    expectTexts(app, ["Practice account", "Returns are simulated and never sent to the BIR"]);
    const real = await openSettings(size, { cloud: "in", solo: true }, "account");
    await real.waitFor(ready);
    await real.waitFor("text:calls: cloud_account_refresh");
    expect(real.shows("id:settings-account-practice")).toBe(false);
  }, 60_000);

  test(`${name}: an unreachable server leaves the card as it was and says why`, async () => {
    const app = await openSettings(size, { cloud: "down", solo: true }, "account");
    await app.waitFor(ready);
    await app.press("id:settings-account-sign-in");
    await app.waitFor("text:Could not reach the Buwiz server. Check the connection and try again.", { timeout: 15000 });
    expect(app.shows("id:settings-account-sign-in")).toBe(true);
    expect(app.enabled("id:settings-account-sign-in")).toBe(true);
    expect(state(app)).toContain("cloudLocked=false");
  }, 60_000);

  test(`${name}: who is signed in, the registered name and email with a pencil each, Sign out and Add account`, async () => {
    const app = await openSettings(size, { solo: true }, "account");
    await app.waitFor(ready);
    expectTexts(app, ["Juana Santos", "juana@example.com", name === "phone" ? "Admin · this device" : "Admin · this computer", "Registered name", "TIN 000-123-453", "Email", "Printed on this profile's returns"]);
    // "Just me" with a taxpayer already: no Add account.
    expect(app.shows("id:settings-add-account")).toBe(false);
    await app.press("id:settings-profile-email-edit");
    await app.waitFor("text:/view=profile/");
    expect(state(app)).toContain('view=profile {"edit":"address"}');
    const again = await openSettings(size, { solo: true }, "account");
    await again.press("id:settings-profile-name-edit");
    await again.waitFor("text:/view=profile/");
    expect(state(again)).toContain('view=profile {"edit":"taxpayer"}');
    // With no PIN there is nothing to sign back in with: Sign out says so and stays.
    const out = await openSettings(size, { solo: true }, "account");
    await out.press("id:settings-sign-out");
    await out.waitFor("text:Set your PIN in Settings first. With no PIN there is nothing to sign back in with.");
    expect(state(out)).toContain("locked=false");
  }, 90_000);

  test(`${name}: a help desk's admin can add an account, and its codes are the Admin PIN and Admin authenticator`, async () => {
    const app = await openSettings(size, { helpdesk: "2468", signin: true }, "account");
    await app.waitFor(ready);
    await app.waitFor("text:Admin PIN");
    expectTexts(app, ["Help desk admin", "TIN 000-000-000", "Admin PIN", "Admin authenticator", "Signs in TIN 000-000-000; opens locked-out taxpayers"]);
    expect(app.shows("id:settings-add-account")).toBe(true);
    await app.press("id:settings-add-account");
    await app.waitFor("text:/view=new_taxpayer/");
  }, 60_000);

  // Gmail is desktop only: its sign-in waits on a local web address a phone cannot answer (taxpayer/Inbox.tsx). On a desktop
  // it also needs a build with a Google client id.
  if (name === "phone") {
    test("phone: Settings > Account has no Google card, so Gmail cannot be connected", async () => {
      const app = await openSettings(size, { google: true, solo: true }, "account");
      await app.waitFor(ready);
      expect(app.shows("id:settings-google")).toBe(false);
      expect(app.shows("id:settings-google-connect")).toBe(false);
      expect(app.texts()).not.toContain("Gmail");
    }, 60_000);
  } else {
    test(`${name}: Gmail connects, tracks receipts and disconnects; with no taxpayer it says to add one first`, async () => {
      const app = await openSettings(size, { google: true, solo: true }, "account");
      await app.waitFor("id:settings-google-connect");
      expectTexts(app, ["Google", "Gmail", "Receipt tracking", "Not connected. Reads BIR confirmation mail for Head office."]);
      expect(app.shows("id:settings-google-disconnect")).toBe(false);
      await app.press("id:settings-google-tracking-switch");
      await app.waitFor("text:Automated receipt tracking on.");
      expect(calls(app)).toContain("set_inbox_tracking true");
      await app.press("id:settings-google-connect");
      await app.waitFor("text:Connected as juana.inbox@example.com", { timeout: 15000 });
      expect(calls(app)).toContain("connect_inbox_oauth");
      expect(app.texts()).toContain("Reconnect");
      expect(app.shows("id:settings-google-disconnect")).toBe(true);
      await app.press("id:settings-google-disconnect");
      await app.waitFor("text:Not connected. Reads BIR confirmation mail for Head office.");
      expect(app.shows("id:settings-google-disconnect")).toBe(false);
      expect(calls(app)).toContain("disconnect_inbox_oauth");

      const none = await openSettings(size, { demo: false }, "account");
      await none.waitFor("id:settings-google-empty");
      expectTexts(none, ["No taxpayer yet", "Add a taxpayer before connecting a Gmail account."]);
    }, 90_000);

    test("desktop: a build with no Google client id offers no Connect, and says to use an app password", async () => {
      const app = await openSettings(size, { google: "unavailable", solo: true }, "account");
      await app.waitFor("id:settings-google");
      expectTexts(app, ["Gmail", "Google sign-in is not available in this version. Use an app password.", "Receipt tracking"]);
      expect(app.shows("id:settings-google-connect")).toBe(false);
      expect(app.texts()).not.toContain("OAuth failed: GOOGLE_CLIENT_ID must be set in .env");
    }, 60_000);
  }
}

for (const { name, size } of SIZES) {
  test(`${name}: a first PIN signs the owner in as the admin, shows a recovery key once, and Change and Remove follow`, async () => {
    const app = await openSettings(size, { solo: true }, "account");
    await app.waitFor(ready);
    expectTexts(app, ["My PIN", name === "phone" ? "Not set. Anyone using this device can open this account" : "Not set. Anyone using this computer can open this account", "Set PIN", "Recovery key", "Made when you set a PIN or authenticator"]);
    expect(app.shows("id:settings-pin-clear")).toBe(false);

    // Four digits are needed.
    await app.type("id:settings-pin-cells", "12");
    await app.press("id:settings-pin-save");
    await app.waitFor("text:Type all 4 digits of the new PIN first.");

    await app.type("id:settings-pin-cells", "34");
    await app.press("id:settings-pin-save");
    await app.waitFor("text:PIN saved. Sign-in asks for it from now on.");
    // The owner is the admin now: still in Settings, on Account, with the key to write down.
    await app.waitFor("id:settings-recovery-key", { timeout: 15000 });
    expect(state(app)).toContain("session=admin");
    expect(state(app)).toContain("view=settings");
    expect(app.texts()).toContain("Write it down now. It is shown once and works once.");
    const key = app.texts().find((t) => /^[A-Z0-9]{4}(-[A-Z0-9]{4}){3}$/.test(t));
    expect(key).toBeTruthy();
    expect(app.texts()).toContain("Change");
    expect(app.shows("id:settings-pin-clear")).toBe(true);
    expect(app.shows("id:settings-recovery-btn")).toBe(false);

    // The PIN is the owner's: Change takes four new digits.
    await app.type("id:settings-pin-cells", "5678");
    await app.press("id:settings-pin-save");
    await app.waitFor("text:PIN changed.");
    expect(app.value("id:settings-pin-cells")).toBe("");
    await app.press("id:settings-pin-clear");
    await app.waitFor("text:PIN removed.");
    expect(app.texts()).toContain("Set PIN");
  }, 90_000);

  test(`${name}: an authenticator is added with the key and a code it shows, cancelled, refused when wrong, and removed`, async () => {
    const app = await openSettings(size, { solo: true, pin: "1234", signin: true }, "account");
    await app.waitFor(ready);
    expect(app.texts()).toContain("Use a 6-digit code from an authenticator app instead of the PIN");
    expect(app.shows("id:settings-totp-cells")).toBe(false);
    await app.press("id:settings-totp-btn");
    await app.waitFor("id:settings-totp-cells");
    const hint = () => app.texts().find((t) => t.startsWith("Add this key to your authenticator app, then type the code it shows: ")) ?? "";
    const secret = hint().split(": ")[1];
    expect(secret).toMatch(/^[A-Z2-7]{16,}$/);
    // Cancel forgets the key.
    await app.press("id:settings-totp-cancel");
    await app.waitFor("id:settings-totp-btn");
    expect(app.shows("id:settings-totp-cells")).toBe(false);

    await app.press("id:settings-totp-btn");
    await app.waitFor("id:settings-totp-cells");
    const second = hint().split(": ")[1];
    expect(second).not.toBe(secret);
    // A wrong code is refused and the key stays on screen.
    await app.type("id:settings-totp-cells", "000000");
    await app.press("id:settings-totp-confirm");
    await app.waitFor("id:settings-totp-cells");
    expect(app.texts()).not.toContain("Authenticator added.");
    expect(app.shows("id:settings-totp-btn")).toBe(false);
    await app.clear("id:settings-totp-cells");
    await app.type("id:settings-totp-cells", totpCode(second));
    await app.press("id:settings-totp-confirm");
    await app.waitFor("text:Authenticator added.");
    expect(app.texts()).toContain("A code from your authenticator app also signs you in");
    await app.press("id:settings-totp-btn");
    await app.waitFor("text:Authenticator removed.");
    expect(app.texts()).toContain("Add");
  }, 90_000);

  test(`${name}: the owner makes a new recovery key, and the old one is replaced`, async () => {
    const app = await openSettings(size, { solo: true, pin: "1234", signin: true }, "account");
    await app.waitFor(ready);
    expect(app.texts()).toContain("None yet. Without one, only your Mac login can reset a forgotten PIN");
    await app.press("id:settings-recovery-btn");
    await app.waitFor("id:settings-recovery-key");
    expect(app.texts()).toContain("Write it down now. It is shown once and works once.");
    const first = app.texts().find((t) => /^[A-Z0-9]{4}(-[A-Z0-9]{4}){3}$/.test(t));
    expect(first).toBeTruthy();
    // Leaving Settings forgets the key; back in, it is a button for a new one.
    await app.press(name === "phone" ? "id:tab-home" : "id:nav-home");
    await app.waitFor("id:home-dashboard");
    if (name === "phone") {
      // Home is a tab: Settings is reached again from the badge's sheet, and opens on its list.
      await app.press("id:nb-avatar");
      await app.press("id:nb-menu-profile");
      await app.waitFor("id:sheet-extras");
      await app.press("id:sheet-settings");
      // The sheet closes as the page moves: until it is gone it still covers the list (a press on a row reaches the avatar in it).
      await app.waitFor("id:sheet-extras", { state: "gone" });
      await app.waitFor("id:settings-nav-account");
      await app.press("id:settings-nav-account");
    } else {
      await app.press("id:tb-back");
    }
    await app.waitFor("id:settings-recovery-btn");
    expect(app.shows("id:settings-recovery-key")).toBe(false);
    expect(app.texts()).toContain("Make a new key");
    expect(app.texts()).toContain("Signs you in if you forget the PIN. Making a new one cancels the old one");
  }, 90_000);

  test(`${name}: a taxpayer who signed in edits their own PIN and authenticator, and has no recovery key`, async () => {
    const app = await openSettings(size, { solo: true, signin: "taxpayer" }, "account");
    await app.waitFor(ready);
    expectTexts(app, ["My PIN", "My authenticator", name === "phone" ? "Not set. Anyone using this device can open this account" : "Not set. Anyone using this computer can open this account"]);
    expect(app.shows("id:settings-recovery")).toBe(false);
    await app.type("id:settings-pin-cells", "4321");
    await app.press("id:settings-pin-save");
    await app.waitFor("text:PIN saved. Sign-in asks for it from now on.");
    expect(app.texts()).toContain("Asked when you sign in with your TIN");
    expect(app.shows("id:settings-pin-clear")).toBe(true);
    expect(app.shows("id:settings-recovery-key")).toBe(false);
  }, 60_000);
}
