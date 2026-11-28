// Real-window screenshots of the TIN status chips and the Queue sheet (D2): phone, tablet and desktop, light and dark.
// The chips on the tax-profile page in their richest states (Fix with its action, Not activated with the Activate
// button) and the sheet Queue stops on. `just nsh-shots tins` saves ~/BuwizNsh/evidence/tins/*.png. Headless it passes
// without pictures.
import { expect, open, test } from "nsh/test";
import { shoot } from "./shoot";

// No warmForms() here: a test file's own imports run in the CLI's runner, which on `--target macos` has none of
// the app's native modules — `bir/testing` (ebirforms/native) fails the bundle there. The fakes install inside
// each real host from the params.

const ENTRY = "../TaxpayerDemo.tsx";
const FORM_ENTRY = "../FormAnyShot.tsx";

/** Phone, tablet and desktop, light and dark. */
const TIN_LOOKS = [
  { name: "phone-light", width: 390, height: 844, platform: "macos", colorScheme: "light" },
  { name: "phone-dark", width: 390, height: 844, platform: "macos", colorScheme: "dark" },
  { name: "tablet-light", width: 834, height: 1112, platform: "macos", colorScheme: "light" },
  { name: "tablet-dark", width: 834, height: 1112, platform: "macos", colorScheme: "dark" },
  { name: "desktop-light", width: 1440, height: 900, platform: "macos", colorScheme: "light" },
  { name: "desktop-dark", width: 1440, height: 900, platform: "macos", colorScheme: "dark" },
] as const;

for (const look of TIN_LOOKS) {
  const size = { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme };

  test(`tin chips fix and not-activated, ${look.name}`, async () => {
    const fix = await open(ENTRY, { ...size, host: { params: { start: "view=profile", tins: "fix" } } });
    await fix.waitFor("id:tp-tin-status-action", { timeout: 15000 });
    await fix.wait(500);
    await shoot(fix, `tins-${look.name}-fix`);

    const notYet = await open(ENTRY, { ...size, host: { params: { start: "view=profile", tins: "not_activated" } } });
    await notYet.waitFor("id:tp-tin-status-activate", { timeout: 15000 });
    await notYet.wait(500);
    await shoot(notYet, `tins-${look.name}-not-activated`);
    expect(notYet.shows("id:tp-tin-status-activate")).toBe(true);
  }, 120_000);

  test(`queue sheet, ${look.name}`, async () => {
    const app = await open(FORM_ENTRY, { ...size, host: { params: { formCode: "2551Q", year: 2025, period: 1, tins: "fix", facts: { eopt_tier: "Micro", tax_classification: "SelfEmployed" } } } });
    await app.waitFor("text:BIR Form 2551Q", { timeout: 15000 });
    await app.press("id:fp-queue");
    // Wide shows the blocker as a dialog, narrow as the phone sheet: either answers the press.
    const blockedShown = () => app.shows("id:tin-queue-blocked") || app.shows("id:tin-queue-blocked-sheet");
    // Press past the page's own stops, waiting on the transitions rather than counting a fixed beat.
    for (let i = 0; i < 8 && !blockedShown(); i++) {
      if (app.shows("id:queue-continue")) await app.press("id:queue-continue");
      else await app.press("id:fp-queue");
      await app.waitUntil(() => blockedShown() || app.shows("id:queue-continue"), { timeout: 2000 }).catch(() => {});
    }
    await app.waitUntil(blockedShown, { timeout: 10000 });
    await app.wait(400);
    await shoot(app, `tins-${look.name}-queue-sheet`);
    expect(blockedShown()).toBe(true);
  }, 120_000);
}
