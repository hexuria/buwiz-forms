// Real-window screenshots of Home: light and dark, wide and narrow. The dashboard with drafts, a filed return, a queued one,
// an as-needed return and a second taxpayer; a tile picked; a day of the strip picked; the welcome of a fresh install.
// `just nsh-shots home` saves ~/BuwizNsh/evidence/home/*.png. Headless (plain nsh test) it takes no pictures and passes.
import { expect, open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

const ENTRY = "../HomeDemo.tsx";
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });
const RICH = { drafts: 2, filed: 1, queued: true, undated: true, second: true, branch: true };

for (const look of LOOKS) {
  test(`home, ${look.name}`, async () => {
    const app = await open(ENTRY, { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme, ...asking(RICH) });
    await app.waitFor("id:home-dashboard", { timeout: 30000 });
    await app.wait(800);
    await shoot(app, `home-${look.name}-top`);
    await app.scroll("id:main-scroll", 700);
    await app.wait(300);
    await shoot(app, `home-${look.name}-middle`);
    await app.scroll("id:main-scroll", 2000).catch(() => {});
    await app.wait(300);
    await shoot(app, `home-${look.name}-bottom`);
    await app.scroll("id:main-scroll", -5000).catch(() => {});
    await app.press("id:home-tile-overdue");
    await app.waitFor("id:home-due-show-all");
    await app.wait(300);
    await shoot(app, `home-${look.name}-overdue`);
    expect(app.shows("id:home-dashboard")).toBe(true);
  }, 120_000);

  test(`home welcome, ${look.name}`, async () => {
    const app = await open(ENTRY, { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme, ...asking({ demo: false }) });
    await app.waitFor("id:home-welcome", { timeout: 30000 });
    await app.wait(500);
    await shoot(app, `home-${look.name}-welcome`);
    expect(app.shows("id:welcome-add-taxpayer")).toBe(true);
  }, 120_000);
}

// A window between the phone layout and the full one: three tiles across, wide rows in a 618-point column.
for (const look of LOOKS.filter((l) => l.width > 600)) {
  test(`home at 700 points, ${look.name}`, async () => {
    const app = await open(ENTRY, { width: 700, height: 900, platform: look.platform, colorScheme: look.colorScheme, ...asking(RICH) });
    await app.waitFor("id:home-dashboard", { timeout: 30000 });
    await app.wait(800);
    await shoot(app, `home-mid-${look.colorScheme}-top`);
    await app.scroll("id:main-scroll", 3000).catch(() => {});
    await app.wait(300);
    await shoot(app, `home-mid-${look.colorScheme}-bottom`);
    expect(app.shows("id:home-dashboard")).toBe(true);
  }, 120_000);
}
