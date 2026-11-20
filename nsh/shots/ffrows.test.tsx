// Row heights of the form page at 420 and 640 pt (the form-fields PRs state the change at both): Form 0605's first part as it
// opens, and after Check with every sentence showing. `just nsh-shots ffrows` saves ~/BuwizNsh/evidence/ffrows/*.png; the rows
// are measured on the pictures (the dividers between them, shots/rowheights.py's rule in pixels.ts). Headless (plain nsh test)
// it takes none and still passes.
import { expect, open, test } from "nsh/test";
import { decodePng, dividerYs, picture } from "./pixels";
import { shoot } from "./shoot";

const WIDTHS = [420, 640];
// Item 5 ("No. of Sheets Attached", row 0:4) as it opens: 109 pt before PR 6's message line went conditional, 87 pt after —
// the 22 pt reserve (mt-1 + min-h-4.5) is gone.
const ITEM5_PT = 87;

for (const width of WIDTHS) {
  test(`0605 rows at ${width} pt: as opened, and after Check`, async () => {
    const app = await open("../FormAnyShot.tsx", {
      width, height: width < 600 ? 2000 : 1400, platform: "macos", colorScheme: "light",
      host: { params: { formCode: "0605", year: 2023, period: 1, mode: "return" } },
    });
    await app.waitFor("id:fp-item-0:3", { timeout: 30_000 });
    await app.wait(800);
    const openPng = await picture(app, `rows-open-${width}`);
    await app.press("id:fp-check");
    // The page checked once Item 4's sentence is on its row (the fp-msg view mounts only with a message now).
    await app.waitFor("text:Item 4 needs a due date.", { timeout: 30_000 });
    await app.waitFor("text:/items? need attention/", { state: "gone", timeout: 15_000 });
    await app.wait(500);
    await shoot(app, `rows-checked-${width}`);
    if (openPng) {
      // Part I's card sits in the picture's top ~2500 px; decode just that and let each heavy step breathe.
      const raster = decodePng(openPng, 2500);
      await app.wait(1);
      const ys = dividerYs(raster, width);
      await app.wait(1);
      // Past the ~220 pt page header the first divider is row 0:0's top; the eight row tops follow, and
      // Item 5 is the fifth row — between row-top dividers four and five of that run.
      const i0 = ys.findIndex((p) => p > 220);
      const h = i0 >= 0 && i0 + 5 < ys.length ? ys[i0 + 5] - ys[i0 + 4] : NaN;
      console.log(`rows-open-${width}: Item 5 row ${h} pt (want ~${ITEM5_PT})`);
      expect(Math.abs(h - ITEM5_PT)).toBeLessThanOrEqual(3);
    }
  }, 120_000);
}
