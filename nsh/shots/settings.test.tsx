// Real-window screenshots of Settings: light and dark, wide and narrow. Every section: General, Account (with a PIN and an
// authenticator key being added), Import / Export, Jobs, Logs, Computer (with the setup change open), and the search.
// `just nsh-shots settings` saves ~/BuwizNsh/evidence/settings/*.png. Headless (plain nsh test) it takes no pictures and passes.
import { type App, expect, open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

const ENTRY = "../SettingsDemo.tsx";
const RICH = { solo: true, second: true, jobs: true, log: "mixed", cloud: "out", update: true, start: "view=settings" };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });

/** A phone: from a section's page back to the list, by the header's Back. */
async function toList(app: App): Promise<void> {
  await app.press("id:nb-back");
  await app.waitFor("id:settings-list-scroll", { timeout: 15000 });
}

/** The section's page: wide, press the list's row; narrow, press the row in the list. */
async function section(app: App, id: string, ready: string): Promise<void> {
  await app.press(`id:settings-nav-${id}`);
  await app.waitFor(ready, { timeout: 30000 });
  await app.wait(400);
}

for (const look of LOOKS) {
  test(`settings, ${look.name}`, async () => {
    const app = await open(ENTRY, { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme, ...asking(RICH) });
    await app.waitFor("id:settings", { timeout: 30000 });
    await app.wait(800);
    const narrow = look.width < 600;
    await shoot(app, `settings-${look.name}-start`);

    await section(app, "general", "id:settings-appearance");
    await shoot(app, `settings-${look.name}-general`);

    if (narrow) await toList(app);
    await section(app, "account", "id:settings-security");
    await shoot(app, `settings-${look.name}-account`);
    await app.scroll("id:settings-scroll", 520).catch(() => {});
    await app.wait(300);
    await shoot(app, `settings-${look.name}-account-2`);
    await app.press("id:settings-totp-btn");
    await app.waitFor("id:settings-totp-cells");
    await app.wait(300);
    await shoot(app, `settings-${look.name}-account-totp`);

    if (narrow) await toList(app);
    await section(app, "transfer", "id:settings-data");
    await shoot(app, `settings-${look.name}-transfer`);

    if (narrow) await toList(app);
    await section(app, "jobs", "id:jobs-tab");
    await shoot(app, `settings-${look.name}-jobs`);
    await app.press("id:jobs-tabs >> text:Logs");
    await app.waitFor("id:logs-tab");
    await app.wait(300);
    await shoot(app, `settings-${look.name}-logs`);

    if (narrow) await toList(app);
    await section(app, "computer", "id:settings-files");
    await shoot(app, `settings-${look.name}-computer`);
    await app.scroll("id:settings-scroll", 700).catch(() => {});
    await app.wait(300);
    await shoot(app, `settings-${look.name}-computer-2`);
    await app.press("id:setup-change-btn");
    await app.press("id:setup-change-owner");
    await app.waitFor("id:setup-change-owner-head");
    await app.wait(300);
    await shoot(app, `settings-${look.name}-setup-owner`);
    expect(app.shows("id:settings")).toBe(true);
  }, 180_000);

  if (look.width > 600) {
    // A window between the phone layout and a roomy one: the section list takes 220 of 640, so the cards are narrow.
    test(`settings at 640 points, ${look.name}`, async () => {
      const app = await open(ENTRY, { width: 640, height: 900, platform: look.platform, colorScheme: look.colorScheme, ...asking(RICH) });
      await app.waitFor("id:settings", { timeout: 30000 });
      await section(app, "account", "id:settings-security");
      await shoot(app, `settings-mid-${look.colorScheme}-account`);
      await app.scroll("id:settings-scroll", 900).catch(() => {});
      await app.wait(300);
      await shoot(app, `settings-mid-${look.colorScheme}-account-2`);
      await section(app, "jobs", "id:jobs-tab");
      await shoot(app, `settings-mid-${look.colorScheme}-jobs`);
      expect(app.shows("id:settings")).toBe(true);
    }, 120_000);
  }

  test(`settings search, ${look.name}`, async () => {
    const app = await open(ENTRY, { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme, ...asking(RICH) });
    await app.waitFor("id:settings", { timeout: 30000 });
    await app.type("id:settings-search", "pin");
    await app.wait(500);
    await shoot(app, `settings-${look.name}-search-pin`);
    await app.type("id:settings-search", "zzzz");
    await app.wait(500);
    await shoot(app, `settings-${look.name}-search-none`);
    expect(app.shows("id:settings")).toBe(true);
  }, 120_000);
}
