// Real-window screenshots of "New tax profile" always there: the Home empty state's button, the picker's add row, the
// panel entry and the page itself — phone, tablet and desktop, light and dark. `just nsh-shots newprofile` saves
// ~/BuwizNsh/evidence/newprofile/*.png. Headless (plain nsh test) it takes no pictures and passes.
import { expect, open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

const ENTRY = "../ShellDemo.tsx";
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });

for (const look of LOOKS) {
  const size = { width: look.width, height: look.height, platform: look.platform as "macos", colorScheme: look.colorScheme };

  test(`new tax profile: Home empty and the page, ${look.name}`, async () => {
    const app = await open(ENTRY, { ...size, ...asking({ demo: false }) });
    await app.waitFor("id:home-welcome", { timeout: 30000 });
    await app.wait(500);
    await shoot(app, `newprofile-${look.name}-home-empty`);
    await app.press("id:welcome-add-taxpayer");
    await app.waitFor("id:new-tp");
    await app.wait(400);
    await shoot(app, `newprofile-${look.name}-page`);
    expect(app.texts()).toContain("New tax profile");
  }, 120_000);

  test(`new tax profile: the entry with profiles present, ${look.name}`, async () => {
    const app = await open(ENTRY, { ...size, ...asking({ second: true }) });
    if (look.width < 600) {
      // A phone: the picker's add row.
      await app.waitFor("id:tab-bar");
      await app.press("id:nb-search");
      await app.waitFor("id:palette-input");
      await app.wait(400);
      await shoot(app, `newprofile-${look.name}-picker`);
      expect(app.texts()).toContain("New tax profile");
    } else {
      // Tablet and desktop: the panel's entry beside the taxpayer list.
      await app.waitFor("id:frame");
      await app.press("id:context-pill");
      await app.waitFor("id:panel-profile");
      await app.wait(400);
      await shoot(app, `newprofile-${look.name}-panel`);
      expect(app.shows("id:panel-new-tp")).toBe(true);
    }
  }, 120_000);
}

// A tablet's own shape: the tax-profile sheet from the header carries the entry, and it opens the page.
for (const scheme of ["light", "dark"] as const) {
  test(`new tax profile: the tablet entry, ${scheme}`, async () => {
    const app = await open(ENTRY, { width: 1376, height: 1032, platform: "ios", colorScheme: scheme, ...asking({ second: true }) });
    await app.waitFor("id:nb-taxpayer");
    await app.press("id:nb-taxpayer");
    await app.waitFor("id:sheet-extras");
    await app.wait(400);
    expect(app.shows("id:panel-new-tp")).toBe(true);
    await shoot(app, `newprofile-tablet-${scheme}-sheet`);
    await app.press("id:panel-new-tp");
    await app.waitFor("id:new-tp");
    await app.wait(400);
    await shoot(app, `newprofile-tablet-${scheme}-page`);
    expect(app.texts()).toContain("New tax profile");
  }, 120_000);
}
