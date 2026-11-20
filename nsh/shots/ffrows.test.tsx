// Row heights of the form page at 420 and 640 pt (the form-fields PRs state the change at both): Form 0605's first part as it
// opens, and after Check with every sentence showing. `just nsh-shots ffrows` saves ~/BuwizNsh/evidence/ffrows/*.png; the rows
// are measured on the pictures (the dividers between them). Headless (plain nsh test) it takes none and still passes.
import { open, test } from "nsh/test";
import { shoot } from "./shoot";

const WIDTHS = [420, 640];

for (const width of WIDTHS) {
  test(`0605 rows at ${width} pt: as opened, and after Check`, async () => {
    const app = await open("../FormAnyShot.tsx", {
      width, height: width < 600 ? 2000 : 1400, platform: "macos", colorScheme: "light",
      host: { params: { formCode: "0605", year: 2023, period: 1, mode: "return" } },
    });
    await app.waitFor("id:fp-item-0:3", { timeout: 30_000 });
    await app.wait(800);
    await shoot(app, `rows-open-${width}`);
    await app.press("id:fp-check");
    await app.waitFor("id:fp-msg-0:3", { timeout: 30_000 });
    await app.waitFor("text:/items? need attention/", { state: "gone", timeout: 15_000 });
    await app.wait(500);
    await shoot(app, `rows-checked-${width}`);
  }, 120_000);
}
