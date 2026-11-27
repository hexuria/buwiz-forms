// Real-window screenshots of Filings: light and dark, wide (the board) and narrow (the list under status tabs), with the
// filters open, the Filters sheet, the empty states and a navigation preset. `just nsh-shots filings` saves
// ~/BuwizNsh/evidence/filings/*.png. Headless (plain nsh test) it takes no pictures and passes. Demo taxpayer and generated returns only.
import { expect, open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

// The submission worker starts once per process, on the first database an app opens (a real window runs in the host's process,
// and a test file cannot reach the door). The first test below is the empty one, so the worker takes an empty database and the
// seeded queued return stays queued in the windows after it.

const ENTRY = "../FilingsDemo.tsx";
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });

for (const look of LOOKS) {
  test(`filings, no returns, ${look.name}`, async () => {
    const app = await open(ENTRY, { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme, ...asking({ seed: "none" }) });
    await app.waitFor("id:filings-empty", { timeout: 30000 });
    await app.wait(500);
    await shoot(app, `filings-${look.name}-empty`);
    await app.type("id:filings-search", "zzz");
    await app.wait(300);
    await shoot(app, `filings-${look.name}-empty-search`);
    expect(app.shows("id:filings-empty")).toBe(true);
  }, 120_000);
}

for (const look of LOOKS) {
  const wide = look.width > 600;
  test(`filings, ${look.name}`, async () => {
    const app = await open(ENTRY, { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme, ...asking({ generated: true }) });
    await app.waitFor(wide ? "id:filings-board" : "id:filings-tabs", { timeout: 30000 });
    await app.wait(800);
    await shoot(app, `filings-${look.name}-all`);
    if (wide) {
      await app.press("id:filings-statuses-trigger");
      await app.waitFor("id:filings-statuses-opt-Paid");
      await app.wait(300);
      await shoot(app, `filings-${look.name}-statuses`);
      await app.press("id:filings-statuses-clear");
      await app.press("id:filings-statuses-opt-Queued");
      await app.press("id:filings-statuses-opt-Draft");
      await app.wait(300);
      await shoot(app, `filings-${look.name}-statuses-picked`);
    } else {
      await app.press("id:filings-tabs >> label:Draft");
      await app.wait(300);
      await shoot(app, `filings-${look.name}-draft-tab`);
      await app.press("id:filings-tabs >> label:All");
      await app.press("id:filings-filters-open");
      await app.waitFor("id:filings-sheet-done");
      await app.wait(500);
      await shoot(app, `filings-${look.name}-filters`);
      await app.press("id:filings-statuses-trigger");
      await app.waitFor("id:filings-statuses-opt-Paid");
      await app.wait(400);
      await shoot(app, `filings-${look.name}-filters-open`);
    }
    expect(app.shows("id:filings")).toBe(true);
  }, 120_000);

  test(`filings, a navigation preset, ${look.name}`, async () => {
    const d = new Date();
    const month = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
    const app = await open(ENTRY, {
      width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme,
      ...asking({ go: { statuses: ["Submitted", "Confirmed", "Paid"], updatedMonth: month } }),
    });
    await app.waitFor("id:filings-month-chip", { timeout: 30000 });
    await app.wait(800);
    await shoot(app, `filings-${look.name}-preset`);
    expect(app.shows("id:filings")).toBe(true);
  }, 120_000);
}

// A window between the two: 900 points wide with the sidebar docked leaves a page under 720, so the list and a Filters drawer.
for (const look of LOOKS.filter((l) => l.width > 600)) {
  test(`filings, 900 points, ${look.name}`, async () => {
    const app = await open(ENTRY, { width: 900, height: 800, platform: look.platform, colorScheme: look.colorScheme, ...asking({ generated: true }) });
    await app.waitFor("id:filings-tabs", { timeout: 30000 });
    await app.wait(800);
    await shoot(app, `filings-${look.name}-900`);
    await app.press("id:filings-filters-open");
    await app.waitFor("id:filings-sheet-done");
    await app.wait(500);
    await shoot(app, `filings-${look.name}-900-filters`);
  }, 120_000);
}
