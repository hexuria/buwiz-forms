// Real-window screenshots of the frame: light and dark, wide and narrow, with the demo taxpayer; the panels, the palette,
// the start-a-return dialog, the floating sidebar under 860 points and the narrow layout's tab bar and sheet.
// `just nsh-shots shell` saves ~/BuwizNsh/evidence/shell/*.png. Headless (plain nsh test) it takes no pictures and passes.
import { expect, open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

const ENTRY = "../ShellDemo.tsx";
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });

for (const look of LOOKS) {
  const wide = look.width > 600;
  test(`frame, ${look.name}`, async () => {
    const app = await open(ENTRY, { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme, ...asking({ branch: true, second: true }) });
    await app.waitFor("id:main-surface");
    await app.wait(500);
    await shoot(app, `shell-${look.name}-home`);
    if (wide) {
      await app.press("id:context-pill");
      await app.waitFor("id:panel-profile");
      await app.wait(300);
      await shoot(app, `shell-${look.name}-profile-panel`);
      await app.press("id:tb-notifications");
      await app.waitFor("id:panel-notifications");
      await app.wait(300);
      await shoot(app, `shell-${look.name}-notifications`);
      await app.press("id:tb-panel");
      await app.press("id:account-row");
      await app.waitFor("id:menu-lock");
      await app.wait(300);
      await shoot(app, `shell-${look.name}-account-menu`);
      await app.keyboard.press("escape");
      await app.keyboard.press("mod+k");
      await app.waitFor("id:palette-input");
      await app.wait(300);
      await shoot(app, `shell-${look.name}-palette`);
      await app.type("id:palette-input", "2551");
      await app.waitFor("text:2551Q · Quarterly Percentage Tax Return");
      await app.wait(300);
      await shoot(app, `shell-${look.name}-palette-forms`);
      await app.submit("id:palette-input");
      await app.waitFor("id:new-return");
      await app.press("id:new-return-2551Q");
      await app.wait(400);
      await shoot(app, `shell-${look.name}-new-return`);
      await app.press("id:new-return-cancel");
      await app.press("id:nav-library");
      await app.waitFor("id:library");
      await app.wait(300);
      await shoot(app, `shell-${look.name}-library`);
    } else {
      await app.press("id:nb-avatar");
      await app.waitFor("id:sheet-extras");
      await app.wait(500);
      await shoot(app, `shell-${look.name}-account-sheet`);
      await app.back();
      await app.press("id:nb-notifications");
      await app.waitFor("id:notifications-list");
      await app.wait(500);
      await shoot(app, `shell-${look.name}-notifications`);
      await app.back();
      await app.press("id:nb-search");
      await app.waitFor("id:palette-input");
      await app.wait(300);
      await shoot(app, `shell-${look.name}-palette`);
      await app.keyboard.press("escape");
      await app.press("id:tab-library");
      await app.waitFor("id:library");
      await app.wait(300);
      await shoot(app, `shell-${look.name}-library`);
    }
    expect(app.shows("id:shell")).toBe(true);
  }, 120_000);
}

for (const look of LOOKS.filter((l) => l.width > 600)) {
  test(`frame under 860 points, ${look.name}`, async () => {
    const app = await open(ENTRY, { width: 760, height: 800, platform: look.platform, colorScheme: look.colorScheme, ...asking() });
    await app.waitFor("id:main-surface");
    await app.press("id:tb-sidebar");
    await app.waitFor("id:sidebar-overlay");
    await app.wait(400);
    await shoot(app, `shell-${look.name}-floating-sidebar`);
    await app.press("id:overlay-scrim");
    await app.waitFor("id:sidebar-overlay", { state: "gone" }); // it slides away, and covers the pill until it has
    await app.press("id:context-pill");
    await app.waitFor("id:panel-overlay");
    await app.wait(400);
    await shoot(app, `shell-${look.name}-floating-panel`);
  }, 120_000);
}

// Esc closes the top layer only: the dialog over a panel closes first, the panel stays. Real windows route keys through the
// platform, so this is also the proof that one press is not heard twice (once by the dialog, once by the frame).
test("Esc closes the dialog and leaves the panel behind it", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", ...asking() });
  await app.waitFor("id:main-surface");
  await app.press("id:context-pill");
  await app.waitFor("id:panel-profile");
  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  await app.keyboard.press("escape");
  await app.waitFor("id:palette-input", { state: "gone" });
  await app.wait(300);
  expect(app.shows("id:panel-profile")).toBe(true);
  await app.keyboard.press("escape");
  await app.waitFor("id:panel-profile", { state: "gone" });
}, 60_000);

// A return open in the frame: the form page fills the main surface (wide: beside the sidebar; narrow: above the tab bar),
// the Recent row marks it, and the toaster sits above the form's pill.
for (const look of LOOKS) {
  test(`form page in the frame, ${look.name}`, async () => {
    const app = await open(ENTRY, { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme, ...asking() });
    await app.waitFor("id:main-surface");
    if (look.width > 600) await app.press("id:sb-new");
    else {
      await app.press("id:nb-search");
      await app.waitFor("id:palette-input");
      await app.type("id:palette-input", "1601");
      await app.waitFor("text:1601C · Monthly Remittance Return of Income Taxes Withheld on Compensation");
      await app.submit("id:palette-input");
    }
    await app.waitFor("id:new-return");
    await app.press("id:new-return-1601C");
    await app.press("id:new-return-open");
    await app.waitFor("id:form-page", { timeout: 20000 });
    await app.waitFor("text:BIR Form 1601C");
    await app.wait(1200);
    await shoot(app, `shell-${look.name}-form`);
  }, 120_000);
}
