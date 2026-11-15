// Real-window screenshots of the form page (a 2551Q of the demo taxpayer): wide and narrow, light and dark.
// `just nsh-shots form` saves ~/BuwizNsh/evidence/form/*.png.
import { open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

for (const look of LOOKS) {
  test(`form page, ${look.name}`, async () => {
    const app = await open("../FormShot.tsx", {
      width: look.width,
      height: look.height,
      platform: look.platform,
      colorScheme: look.colorScheme,
    });
    await app.waitFor("text:BIR Form 2551Q");
    await app.wait(600);
    await shoot(app, `form-${look.name}-top`);
    await app.press("id:fp-check");
    await app.waitFor("id:fp-pill-count");
    // The fixture's Toaster sits above the pill (bottom 96), as in the frame: the pill is pressable under a toast.
    await app.press("id:fp-pill-next");
    await app.wait(900);
    await shoot(app, `form-${look.name}-step`);
    await app.press("id:fp-pill-open");
    await app.wait(500);
    await shoot(app, `form-${look.name}-sections`);
    await app.press("id:fp-section-4");
    await app.wait(500);
    // Headless (plain nsh test) the page may already show its end: scrolling is only for the picture.
    await app.scroll("id:form-scroll", 260).catch(() => {});
    await app.wait(400);
    await shoot(app, `form-${look.name}-schedule`);
  }, 120_000);
}
