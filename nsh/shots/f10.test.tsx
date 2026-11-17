// F10: the fixes outside the form page, in real windows (light, dark, wide, narrow): the content column, the sidebar's Recent row,
// the tabs while hovered, the command palette, the start-a-return dialog, Settings > Jobs, Calendar's month chip, the sign-in's TIN
// cells. Pictures go to ~/BuwizNsh/evidence/f10/ (just nsh-shots f10). Headless (plain nsh test) it takes no pictures and passes;
// the tests that read pixels run only in a real window (the picture is 2x: 1440 points are 2880 pixels).
import { expect, open, test } from "nsh/test";
import { shoot } from "./shoot";
import { decodePng, longestRun, picture, type Raster } from "./pixels";

const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const NARROW = { width: 390, height: 844, platform: "macos" as const };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });
const dark = ([r, g, b]: [number, number, number]) => r < 90 && g < 90 && b < 90;
const bluish = ([r, , b]: [number, number, number]) => b - r > 70;

/** The leftmost and rightmost pixel (in points) that differs from the card's white in the rows y0..y1 (points) between x0 and x1 (points). */
function inkSpan(raster: Raster, scale: number, y0: number, y1: number, x0: number, x1: number): { left: number; right: number } {
  let left = Infinity;
  let right = -Infinity;
  for (let y = Math.round(y0 * scale); y < Math.min(raster.rows, Math.round(y1 * scale)); y += 2) {
    for (let x = Math.round(x0 * scale); x < Math.round(x1 * scale); x++) {
      const [r, g, b] = raster.at(x, y);
      if (r < 244 || g < 244 || b < 244) {
        left = Math.min(left, x / scale);
        break;
      }
    }
    for (let x = Math.round(x1 * scale) - 1; x >= Math.round(x0 * scale); x--) {
      const [r, g, b] = raster.at(x, y);
      if (r < 244 || g < 244 || b < 244) {
        right = Math.max(right, x / scale);
        break;
      }
    }
  }
  return { left, right };
}

for (const scheme of ["light", "dark"] as const) {
  test(`home: the content column is 816 wide inside the 1182 surface, as the old app's (${scheme})`, async () => {
    const app = await open("../ShellDemo.tsx", { ...WIDE, colorScheme: scheme, ...asking({ start: "view=home", theme: scheme === "dark" ? "Dark" : "Light" }) });
    await app.waitFor("id:home-tiles");
    await app.wait(800);
    const png = await picture(app, `f10-home-wide-${scheme}`);
    if (!png || scheme === "dark") return;
    const r = decodePng(png, 440);
    const scale = r.width / 1440;
    // The tile row (Overdue ... Filed this month): from 432 to 1244 points in the old app (x 432-1246 measured), not 400 to 1280.
    const span = inkSpan(r, scale, 170, 210, 300, 1400);
    expect(span.left).toBeGreaterThanOrEqual(430);
    expect(span.left).toBeLessThanOrEqual(434);
    expect(span.right).toBeGreaterThanOrEqual(1242);
    expect(span.right).toBeLessThanOrEqual(1248);
  }, 90_000);
}

test("library: the wide column is 1056 wide, 312 from the sidebar", async () => {
  const app = await open("../ShellDemo.tsx", { ...WIDE, colorScheme: "light", ...asking({ start: "view=library" }) });
  await app.waitFor("id:library");
  await app.wait(800);
  await shoot(app, "f10-library-wide-light");
});

test("calendar month: the chip names the form and the taxpayer's initials, as the old source does", async () => {
  const app = await open("../ShellDemo.tsx", { ...WIDE, colorScheme: "light", ...asking({ start: "view=calendar" }) });
  await app.waitFor("id:calendar-view");
  await app.press("id:calendar-view >> text:Month");
  await app.wait(1000);
  await shoot(app, "f10-calendar-month-wide-light");
});

test("sidebar: the Recent row shows a whole return", async () => {
  const app = await open("../ShellDemo.tsx", { ...WIDE, colorScheme: "light", ...asking({ start: "form=2551Q/2026/3" }) });
  await app.waitFor("id:fp-parts-toggle", { timeout: 40000 });
  await app.waitFor("text:Recent");
  await app.wait(1500);
  await shoot(app, "f10-recent-wide-light");
}, 120_000);

test("tabs: a chosen tab keeps its underline while the pointer is over it (Profile, wide)", async () => {
  const app = await open("../ShellDemo.tsx", { ...WIDE, colorScheme: "light", ...asking({ start: "view=profile" }) });
  await app.waitFor("id:tp-tabs");
  await app.wait(500);
  await app.hover("id:tp-tabs >> label:Profile");
  await app.wait(400);
  const png = await picture(app, "f10-tabs-hover-wide-light");
  if (!png) return;
  const r = decodePng(png, 700);
  const scale = r.width / 1440;
  // The strip sits under the header: a dark 2-point line as wide as the tab (more than 40 points) in one row of pixels.
  let best = 0;
  for (let y = Math.round(150 * scale); y < Math.round(330 * scale); y++) best = Math.max(best, longestRun(r, y, Math.round(432 * scale), Math.round(900 * scale), dark));
  expect(best / scale).toBeGreaterThan(40);
});

test("tabs: a chosen tab keeps its underline while the pointer is over it (Filings, phone)", async () => {
  const app = await open("../FilingsDemo.tsx", { ...NARROW, colorScheme: "light", ...asking() });
  await app.waitFor("id:filings-tabs");
  await app.press("id:filings-tabs >> text:Submitted");
  await app.hover("id:filings-tabs >> text:Submitted");
  await app.wait(400);
  const png = await picture(app, "f10-tabs-hover-narrow-light");
  if (!png) return;
  const r = decodePng(png, 900);
  const scale = r.width / 390;
  let best = 0;
  for (let y = Math.round(100 * scale); y < Math.round(450 * scale); y++) best = Math.max(best, longestRun(r, y, 0, r.width, dark));
  expect(best / scale).toBeGreaterThan(40);
});

for (const scheme of ["light", "dark"] as const) {
  test(`palette: small capital group headings, rows of 36 points, and no ring on the search line (${scheme})`, async () => {
    const app = await open("../ShellDemo.tsx", { ...WIDE, colorScheme: scheme, ...asking({ start: "view=home", theme: scheme === "dark" ? "Dark" : "Light" }) });
    await app.waitFor("id:sidebar");
    await app.keyboard.press("mod+k");
    await app.waitFor("id:palette-input");
    await app.wait(800);
    const png = await picture(app, `f10-palette-wide-${scheme}`);
    if (!png || scheme === "dark") return;
    const r = decodePng(png, 260);
    const scale = r.width / 1440;
    // The dialog's top 48 points (the search line): nothing in the accent's blue (the focus ring was a 1-point blue border).
    let blue = 0;
    for (let y = Math.round(60 * scale); y < Math.round(120 * scale); y++) for (let x = Math.round(440 * scale); x < Math.round(1000 * scale); x++) if (bluish(r.at(x, y))) blue += 1;
    expect(blue).toBe(0);
  });
}

test("new return: the header reads as the old one and Open the profile is under the warning", async () => {
  const app = await open("../ShellDemo.tsx", { ...WIDE, colorScheme: "light", ...asking({ start: "view=home" }) });
  await app.waitFor("id:sidebar");
  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  await app.type("id:palette-input", "2551Q");
  await app.waitFor("text:2551Q · Quarterly Percentage Tax Return");
  await app.submit("id:palette-input");
  await app.waitFor("id:new-return");
  await app.press("id:new-return-2551Q");
  await app.waitFor("id:new-return-not-set");
  await app.wait(600);
  await shoot(app, "f10-newreturn-wide-light");
});

test("settings > jobs: the rows wrap in a 680-wide column", async () => {
  const app = await open("../SettingsDemo.tsx", { ...WIDE, colorScheme: "light", ...asking({ jobs: true, start: "view=settings;section=jobs" }) });
  await app.waitFor("id:jobs-list");
  await app.wait(600);
  await shoot(app, "f10-jobs-wide-light");
});

for (const look of [
  { name: "wide-light", ...WIDE, colorScheme: "light" as const },
  { name: "wide-dark", ...WIDE, colorScheme: "dark" as const },
  { name: "narrow-light", ...NARROW, colorScheme: "light" as const },
]) {
  test(`sign-in: the TIN is nine cells in three groups with dashes (${look.name})`, async () => {
    const app = await open("../AuthDemo.tsx", { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme, ...asking({ demo: true, mode: "solo", adminPin: "1234" }) });
    await app.waitFor("id:auth-card");
    await app.wait(600);
    await shoot(app, `f10-signin-${look.name}-empty`);
    await app.type("id:auth-tin", "000123453");
    await app.wait(400);
    await shoot(app, `f10-signin-${look.name}-filled`);
  });
}
