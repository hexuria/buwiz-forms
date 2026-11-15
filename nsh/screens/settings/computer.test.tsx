// Settings > Computer at phone size and at desktop size: the local files, log retention and the global hotkey (stored through the real host),
// how closing behaves, the version, and the "who uses this computer" change (an owner or a help desk) with its recovery key and the sign-out
// that follows. Against a scratch database with the demo taxpayer (SettingsDemo.tsx).
import { expect, test } from "nsh/test";
import { totpCode } from "../auth/testTotp";
import { expectTexts, openSettings, SIZES, state } from "./testing";

const SECRET = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
const KEY = /^[A-Z0-9]{4}(-[A-Z0-9]{4}){3}$/;

for (const { name, size } of SIZES) {
  test(`${name}: the local files, the version and how closing works are told; retention and the hotkey are stored by the host`, async () => {
    const app = await openSettings(size, { solo: true }, "computer");
    await app.waitFor("id:settings-files");
    expectTexts(app, [
      "Computer", "Local files", "Database", "Data folder", "Logs folder", "Keep logs for", "Older daily log files are deleted", "7 days",
      "Shortcuts", "Global show hotkey", "Shows or hides eBIRForms from any app", "Off", "Window", "Close window",
      "Closing hides eBIRForms to the menu bar when a tray icon exists. With no tray, closing quits. Quit is always in the application menu.",
      "Computer setup", "eBIRForms 0.2.0",
    ]);
    expect(app.texts().some((t) => t.includes("ebirforms-door") && t.endsWith("bir_data.db"))).toBe(true);
    expect(app.texts().some((t) => t.endsWith("dev.goldcoders.bir.nsh/logs"))).toBe(true);

    // The longest retention loses nothing newer than 90 days of the NSH app's own logs.
    await app.press("id:settings-retention-trigger");
    await app.waitFor("text:90 days");
    await app.press("text:90 days");
    await app.waitFor("text:90 days");
    expect(app.texts()).not.toContain("7 days");

    await app.press("id:settings-hotkey-select-trigger");
    await app.waitFor("text:Ctrl+Option+E");
    await app.press("text:Ctrl+Option+E");
    await app.waitFor("text:Ctrl+Option+E");
    expect(app.texts()).not.toContain("Off");
    await app.press("id:settings-hotkey-select-trigger");
    await app.waitFor("text:Off");
    await app.press("text:Off");
    await app.waitFor("text:Off");
    expect(app.texts()).not.toContain("Ctrl+Option+E");
  }, 90_000);

  test(`${name}: a hotkey the host refuses is said in its words and the choice stays as it was`, async () => {
    const app = await openSettings(size, { solo: true, hotkey: "refuse" }, "computer");
    await app.waitFor("id:settings-hotkey");
    await app.press("id:settings-hotkey-select-trigger");
    await app.waitFor("text:Ctrl+Option+B");
    await app.press("text:Ctrl+Option+B");
    await app.waitFor("text:That shortcut is not one this computer can register. Use a letter, digit, or F-key with modifiers, like super+shift+E.");
    expect(app.texts()).toContain("calls: set_global_hotkey control+alt+B");
    expect(app.texts()).not.toContain("Ctrl+Option+B");
  }, 60_000);

  test(`${name}: the owner changes: another taxpayer is chosen, the admin PIN confirms it, a new recovery key shows once, then the computer signs out`, async () => {
    const app = await openSettings(size, { solo: true, second: true, pin: "1234", signin: true }, "computer");
    await app.waitFor("id:setup-change-btn");
    expect(app.texts()).toContain("Just me or my business. Owner: Juana Santos, TIN 000-123-453");
    await app.press("id:setup-change-btn");
    await app.waitFor("id:setup-change-choose");
    expect(app.texts()).toContain("Hand the computer to another taxpayer, or use it to help other taxpayers.");
    await app.press("id:setup-change-owner");
    await app.waitFor("id:setup-change-owner-head");
    expectTexts(app, ["New owner", "Sample Trading Corp", "TIN 000-111-229"]);
    // The owner herself is not a candidate.
    expect(app.shows("id:setup-change-cand-000123453")).toBe(false);
    await app.press("id:setup-change-pick-000111229");
    await app.waitFor("id:setup-change-what");
    expect(app.texts()).toContain("Make Sample Trading Corp (TIN 000-111-229) the owner and admin");
    expect(app.texts()).toContain("Your current admin PIN");
    // The admin has no authenticator: no Use code.
    expect(app.shows("id:setup-change-toggle")).toBe(false);

    // Four digits, then a wrong PIN: the host's refusal is said, and the cells clear.
    await app.type("id:setup-change-pin-cells", "12");
    await app.press("id:setup-change-confirm-btn");
    await app.waitFor("text:Type all 4 digits of your current admin PIN.");
    await app.type("id:setup-change-pin-cells", "99");
    await app.press("id:setup-change-confirm-btn");
    await app.waitFor("text:/PIN/", { timeout: 5000 });
    expect(app.shows("id:setup-change-what")).toBe(true);
    expect(app.value("id:setup-change-pin-cells")).toBe("");

    await app.type("id:setup-change-pin-cells", "1234");
    await app.press("id:setup-change-confirm-btn");
    await app.waitFor("id:setup-change-key", { timeout: 15000 });
    expectTexts(app, ["New recovery key", "Write it down now. It is shown once and works once; the old key no longer works.", "Setup changed", "I wrote it down"]);
    expect(app.texts().some((t) => KEY.test(t))).toBe(true);
    // Only the key shows now: no section list, and the title is Computer.
    expect(app.shows("id:settings-nav-general")).toBe(false);
    expect(state(app)).toContain("admin=000111229");

    await app.press("id:setup-change-done");
    await app.waitFor("text:/locked=true/");
    expect(state(app)).toContain("session=open");
    expect(app.shows("id:settings")).toBe(false);
  }, 90_000);

  test(`${name}: leaving Settings while the new recovery key shows signs the computer out all the same`, async () => {
    const app = await openSettings(size, { solo: true, second: true, pin: "1234", signin: true }, "computer");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-owner");
    await app.press("id:setup-change-pick-000111229");
    await app.type("id:setup-change-pin-cells", "1234");
    await app.press("id:setup-change-confirm-btn");
    await app.waitFor("id:setup-change-done", { timeout: 15000 });
    await app.press(name === "phone" ? "id:tab-home" : "id:nav-home");
    await app.waitFor("text:/locked=true/");
    expect(state(app)).toContain("admin=000111229");
  }, 90_000);

  test(`${name}: Cancel closes the change at each step and forgets what was typed`, async () => {
    const app = await openSettings(size, { solo: true, second: true, pin: "1234", signin: true }, "computer");
    await app.waitFor("id:setup-change-btn");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-cancel");
    await app.waitFor("id:setup-change-btn");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-owner");
    await app.waitFor("id:setup-change-owner-head");
    await app.press("id:setup-change-cancel");
    await app.waitFor("id:setup-change-btn");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-helpdesk");
    await app.waitFor("id:setup-change-newpin");
    await app.type("id:setup-change-newpin-cells", "2468");
    await app.press("id:setup-change-cancel");
    await app.waitFor("id:setup-change-btn");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-helpdesk");
    await app.waitFor("id:setup-change-newpin");
    expect(app.value("id:setup-change-newpin-cells")).toBe("");
    await app.press("id:setup-change-cancel");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-owner");
    await app.press("id:setup-change-pick-000111229");
    await app.waitFor("id:setup-change-what");
    await app.press("id:setup-change-cancel");
    await app.waitFor("id:setup-change-btn");
    expect(state(app)).toContain("admin=000123453");
  }, 90_000);

  test(`${name}: a taxpayer with no PIN yet cannot be chosen as the owner`, async () => {
    const app = await openSettings(size, { solo: true, second: "nopin", pin: "1234", signin: true }, "computer");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-owner");
    await app.waitFor("id:setup-change-owner-head");
    expect(app.texts()).toContain("TIN 000-111-229. No PIN yet: they set one in Settings after signing in");
    expect(app.shows("id:setup-change-pick-000111229")).toBe(false);
  }, 60_000);

  test(`${name}: with no other taxpayer, Another owner says to add one first`, async () => {
    const app = await openSettings(size, { solo: true, pin: "1234", signin: true }, "computer");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-owner");
    await app.waitFor("id:setup-change-none");
    expectTexts(app, ["No other taxpayers", "Add the taxpayer first, then come back here."]);
  }, 60_000);

  test(`${name}: switching to a help desk takes a four-digit admin PIN, the current PIN, and ends in the help desk's own sign-in`, async () => {
    const app = await openSettings(size, { solo: true, pin: "1234", signin: true }, "computer");
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-helpdesk");
    await app.waitFor("id:setup-change-newpin");
    expectTexts(app, ["Switch to a help desk", "New admin PIN", "4 digits, for TIN 000-000-000"]);
    await app.press("id:setup-change-newpin-next");
    await app.waitFor("text:Type all 4 digits of the new admin PIN.");
    await app.type("id:setup-change-newpin-cells", "2468");
    await app.press("id:setup-change-newpin-next");
    await app.waitFor("id:setup-change-what");
    expect(app.texts()).toContain("Switch to a help desk with the new admin PIN");
    await app.type("id:setup-change-pin-cells", "1234");
    await app.press("id:setup-change-confirm-btn");
    await app.waitFor("id:setup-change-key", { timeout: 15000 });
    expect(app.texts().some((t) => KEY.test(t))).toBe(true);
    expect(state(app)).toContain("mode=helpdesk");
    await app.press("id:setup-change-done");
    await app.waitFor("text:/locked=true/");
  }, 90_000);

  test(`${name}: on a help desk the choice is An owner, and the authenticator code can confirm it instead of the PIN`, async () => {
    const app = await openSettings(size, { helpdesk: "2468", second: true, totpSecret: SECRET, signin: true }, "computer");
    await app.waitFor("id:setup-change-btn");
    expect(app.texts()).toContain("A help desk. The admin signs in with TIN 000-000-000");
    await app.press("id:setup-change-btn");
    await app.waitFor("id:setup-change-choose");
    expect(app.texts()).toContain("Make one of the taxpayers the owner. They become the admin and sign in with their own PIN.");
    expect(app.shows("id:setup-change-helpdesk")).toBe(false);
    expect(app.texts()).toContain("An owner");
    await app.press("id:setup-change-owner");
    await app.waitFor("id:setup-change-owner-head");
    // Juana has no PIN, so only Sample Trading Corp can be chosen.
    expect(app.shows("id:setup-change-pick-000123453")).toBe(false);
    await app.press("id:setup-change-pick-000111229");
    await app.waitFor("id:setup-change-what");
    expect(app.shows("id:setup-change-pin-cells")).toBe(true);
    await app.press("id:setup-change-toggle");
    await app.waitFor("id:setup-change-code-cells");
    expect(app.texts()).toContain("Your admin authenticator code");
    await app.type("id:setup-change-code-cells", "12");
    await app.press("id:setup-change-confirm-btn");
    await app.waitFor("text:Type all 6 digits of the code.");
    await app.clear("id:setup-change-code-cells");
    await app.type("id:setup-change-code-cells", totpCode(SECRET));
    await app.press("id:setup-change-confirm-btn");
    await app.waitFor("id:setup-change-key", { timeout: 15000 });
    expect(state(app)).toContain("admin=000111229");
    expect(state(app)).toContain("mode=solo");
  }, 90_000);
}
