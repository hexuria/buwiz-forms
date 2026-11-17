// F20: real-window screenshots of Filings with "Filed in BIR eBIRForms": light and dark, wide (the board) and narrow (the list under
// tabs). The host's answer is FilingsDemo's `legacy: "rows"` (made-up returns of the demo taxpayer and her Cebu branch: an import needs
// the host's own folder dialog). Also one window with nothing imported, which draws no group and no empty box.
// `just nsh-shots f20` saves ~/BuwizNsh/evidence/f20/*.png. Headless (plain nsh test) it takes no pictures and passes.
import { expect, open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

const ENTRY = "../FilingsDemo.tsx";
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });

for (const look of LOOKS) {
  const wide = look.width > 600;
  test(`f20, the group, ${look.name}`, async () => {
    const app = await open(ENTRY, { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme, ...asking({ legacy: "rows" }) });
    await app.waitFor("id:filings-legacy", { timeout: 30000 });
    await app.wait(800);
    await shoot(app, `f20-${look.name}-top`);
    await app.scroll("id:main-scroll", wide ? 600 : 900).catch(() => {});
    await app.wait(500);
    await shoot(app, `f20-${look.name}-group`);
    await app.scroll("id:main-scroll", 3000).catch(() => {});
    await app.wait(500);
    await shoot(app, `f20-${look.name}-end`);
    expect(app.shows("id:filings-legacy")).toBe(true);
  }, 120_000);

  test(`f20, nothing imported, ${look.name}`, async () => {
    const app = await open(ENTRY, { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme, ...asking() });
    await app.waitFor(wide ? "id:filings-board" : "id:filings-tabs", { timeout: 30000 });
    await app.wait(800);
    await app.scroll("id:main-scroll", 3000).catch(() => {});
    await app.wait(500);
    await shoot(app, `f20-${look.name}-none-end`);
    expect(app.shows("id:filings-legacy")).toBe(false);
  }, 120_000);
}

// Another taxpayer in view: her own line only.
test("f20, the second taxpayer's group, wide-light", async () => {
  const look = LOOKS[0];
  const app = await open(ENTRY, {
    width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme,
    ...asking({ legacy: "rows", start: "view=filings;tp=000111229" }),
  });
  await app.waitFor("id:filings-legacy", { timeout: 30000 });
  await app.wait(800);
  await app.scroll("id:main-scroll", 600).catch(() => {});
  await app.wait(500);
  await shoot(app, "f20-wide-light-second");
  expect(app.shows("id:legacy-8")).toBe(true);
}, 120_000);
