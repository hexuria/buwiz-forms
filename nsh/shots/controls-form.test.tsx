// Real-window screenshots of the field controls inside the form page (B1's FormShot, a 2551Q of the demo taxpayer): a look at the
// controls as the page lays them out. `just nsh-shots controls-form` saves ~/BuwizNsh/evidence/controls-form/*.png.
import { open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

for (const look of LOOKS.filter((l) => l.name === "wide-light" || l.name === "narrow-dark")) {
  test(`controls in the form page, ${look.name}`, async () => {
    const app = await open("../FormShot.tsx", { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme });
    await app.waitFor("text:BIR Form 2551Q");
    await app.wait(800);
    await shoot(app, `cf-${look.name}-top`);
    for (let i = 1; i <= 3; i++) {
      try {
        await app.scroll("id:form-scroll", look.height - 160);
      } catch {
        break;
      }
      await app.wait(300);
      await shoot(app, `cf-${look.name}-${i}`);
    }
  }, 120_000);
}
