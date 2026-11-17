// F18: real-window screenshots of Settings > Import / Export > Import from BIR eBIRForms, light and dark, wide and narrow: the card
// idle (with the standard folder a Windows computer offers), the review after a scan, the result after Import, and a wrong folder.
// The host is SettingsDemo's `ebir` fake (a test cannot press the native folder panel). `just nsh-shots f18` saves
// ~/BuwizNsh/evidence/f18/*.png. Headless (plain nsh test) it takes no pictures and passes.
import { type App, open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

const ENTRY = "../SettingsDemo.tsx";

async function openTransfer(look: (typeof LOOKS)[number], ebir: string): Promise<App> {
  const app = await open(ENTRY, {
    width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme,
    host: { params: { ebir, start: "section=transfer" } },
  });
  await app.waitFor("id:settings-ebir-choose", { timeout: 30000 });
  await app.wait(800);
  return app;
}

/** Scrolls the section so `id` is in view (narrow pages are taller than the window). */
async function reveal(app: App, by: number): Promise<void> {
  await app.scroll("id:settings-scroll", by).catch(() => {});
  await app.wait(400);
}

for (const look of LOOKS) {
  test(`f18, ${look.name}`, async () => {
    const narrow = look.width < 600;
    const app = await openTransfer(look, "standard");
    await shoot(app, `f18-${look.name}-idle`);

    await app.press("id:settings-ebir-scan-0");
    await app.waitFor("id:settings-ebir-review", { timeout: 10000 });
    await app.wait(400);
    await shoot(app, `f18-${look.name}-review`);
    if (narrow) {
      await reveal(app, 500);
      await shoot(app, `f18-${look.name}-review-2`);
    }

    await app.press("id:settings-ebir-import");
    await app.waitFor("id:settings-ebir-result", { timeout: 10000 });
    await app.wait(400);
    await shoot(app, `f18-${look.name}-result`);
    if (narrow) {
      await reveal(app, 500);
      await shoot(app, `f18-${look.name}-result-2`);
    }

    const wrong = await openTransfer(look, "wrong");
    await wrong.press("id:settings-ebir-choose");
    await wrong.waitFor("id:settings-ebir-problem", { timeout: 10000 });
    await wrong.wait(400);
    await shoot(wrong, `f18-${look.name}-wrong`);
  }, 120_000);
}
