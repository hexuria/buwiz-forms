// Real-window screenshots of the base screen: wide and narrow, light and dark, top of the page and scrolled to the end.
// `just nsh-shots base` runs it with --target macos and saves ~/BuwizNsh/evidence/base/*.png.
import { expect, open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

for (const look of LOOKS) {
  test(`base screen, ${look.name}`, async () => {
    const app = await open("../BasePreview.tsx", {
      width: look.width,
      height: look.height,
      platform: look.platform,
      colorScheme: look.colorScheme,
    });
    expect(app.texts()).toContain("Native module says: pong");
    await shoot(app, `base-${look.name}-top`);
    await app.scroll("id:page-scroll", 5000);
    await shoot(app, `base-${look.name}-end`);
  });
}
