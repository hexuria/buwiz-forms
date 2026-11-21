// AFTER pictures for PR 6 (Q5/P5a): a row's message line only while it has a message — 0605 Page 1 as
// opened and after Check, the Part II money rows opened, 1601C Part I; the Item 5 row's pixel height; and
// the scroll anchor (a message line appearing above the viewport must not move what is on screen).
// `just nsh-shots ffrowline` saves ~/BuwizNsh/evidence/ffrowline/*.png; headless it takes none and still passes.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";
import { shoot } from "./shoot";
import { decodePng, dividerYs, picture } from "./pixels";

const LOOKS = [
  { name: "420-light", width: 420, height: 900, platform: "macos" as const, colorScheme: "light" as const },
  { name: "420-dark", width: 420, height: 900, platform: "macos" as const, colorScheme: "dark" as const },
  { name: "1280-light", width: 1280, height: 900, platform: "macos" as const, colorScheme: "light" as const },
  { name: "1280-dark", width: 1280, height: 900, platform: "macos" as const, colorScheme: "dark" as const },
];

const ANY = "../FormAnyShot.tsx";
const SETTLE = 800;

async function openForm(params: Record<string, unknown>, look: (typeof LOOKS)[number]): Promise<App> {
  const app = await open(ANY, {
    width: look.width,
    height: look.height,
    platform: look.platform,
    colorScheme: look.colorScheme,
    host: { params },
  });
  await app.waitFor("id:fp-check", { timeout: 60_000 });
  await app.waitFor("text:Opening…", { state: "gone", timeout: 60_000 });
  await app.wait(SETTLE);
  return app;
}

/** The pill's sections list jumps to a part (opens it, folds the rest, scrolls its card to the top). */
async function jumpToPart(app: App, pi: number) {
  await app.press("id:fp-pill-open");
  await app.waitFor(`id:fp-section-${pi}`, { timeout: 15_000 });
  await app.press(`id:fp-section-${pi}`);
  await app.wait(600);
}

test("0605 Page 1: as opened, and after Check with its message lines", async () => {
  for (const look of LOOKS) {
    const app = await openForm({ formCode: "0605", year: 2023, period: 1, mode: "return" }, look);
    await app.waitFor("id:fp-item-0:3", { timeout: 30_000 });
    await app.wait(SETTLE);
    await shoot(app, `0605-page1-open-${look.name}`);
    await app.press("id:fp-check");
    // Check's sentences land on the rows (Item 4's is the row's own line); the toast comes and goes.
    await app.waitFor("text:Item 4 needs a due date.", { timeout: 30_000 });
    await app.waitFor("text:/items? need attention/", { state: "gone", timeout: 15_000 });
    await app.wait(500);
    await shoot(app, `0605-page1-checked-${look.name}`);
    // Items 4-8 carry the messages: scroll them into the middle of the page for the second look.
    await app.scroll("id:form-scroll", look.width < 600 ? 430 : 260);
    await app.wait(600);
    await shoot(app, `0605-page1-checked-rows-${look.name}`);
  }
}, 300_000);

test("0605 Part II: the money rows opened", async () => {
  for (const look of LOOKS) {
    const app = await openForm({ formCode: "0605", year: 2023, period: 1, mode: "return" }, look);
    await app.waitFor("id:fp-item-0:3", { timeout: 30_000 });
    await jumpToPart(app, 2);
    await app.waitFor("id:fp-item-2:0", { timeout: 15_000 });
    await app.wait(SETTLE);
    await shoot(app, `0605-part2-open-${look.name}`);
  }
}, 300_000);

test("1601C Part I opened", async () => {
  for (const look of LOOKS) {
    const app = await openForm({ formCode: "1601C", year: 2025, period: 3, mode: "return" }, look);
    await jumpToPart(app, 1);
    await app.wait(SETTLE);
    await shoot(app, `1601c-part1-open-${look.name}`);
  }
}, 300_000);

test("anchor: Check's message lines above the viewport do not move what is on screen", async () => {
  const look = LOOKS[0]; // 420x900 light: the proof needs one real window, not the look matrix.
  // FormAnchorShot keeps the session and puts its Check outside the page's Scroll: pressing the page's own
  // fp-check would first scroll the header into view and destroy the position this test measures.
  const app = await open("../FormAnchorShot.tsx", {
    width: look.width,
    height: look.height,
    platform: look.platform,
    colorScheme: look.colorScheme,
    host: { params: { formCode: "0605", year: 2023, period: 1, mode: "return" } },
  });
  await app.waitFor("id:fp-check", { timeout: 60_000 });
  await app.waitFor("text:Opening…", { state: "gone", timeout: 60_000 });
  await app.waitFor("id:fp-item-0:3", { timeout: 30_000 });
  // Part I opens under Page 1; scroll to the bottom of the page. Page 1's messaged rows then sit wholly
  // above the viewport (the engine must move the page by exactly their growth), while what is shown —
  // Part I's fold and its clean rows — carries no new line, so the picture must not change.
  await app.press("id:fp-part-1-toggle");
  await app.wait(600);
  await app.scroll("id:form-scroll", 4000);
  await app.waitFor("id:fp-item-1:2", { timeout: 15_000 });
  await app.wait(500);
  const before = await picture(app, "anchor-before");
  await shoot(app, "anchor-before");
  await app.press("id:fx-check");
  // Check's toast proves validation is back and every message is on its row; gone means the page settled.
  await app.waitFor("text:/needs? attention/", { timeout: 30_000 });
  await app.waitFor("text:/needs? attention/", { state: "gone", timeout: 15_000 });
  await app.wait(800);
  const after = await picture(app, "anchor-after");
  await shoot(app, "anchor-after");
  if (before && after) {
    // A full picture's decode and scan are heavy for one JS turn: each step gets its own.
    const a = decodePng(before);
    await app.wait(50);
    const b = decodePng(after);
    await app.wait(50);
    const ya = dividerYs(a, look.width);
    await app.wait(50);
    const yb = dividerYs(b, look.width);
    // Every divider above the first row that grew on screen keeps its place: the lines wholly above the
    // viewport moved the page by as much, so the top of what is shown is the same before and after.
    let same = 0;
    while (same < ya.length && same < yb.length && Math.abs(ya[same] - yb[same]) <= 3) same++;
    expect(same).toBeGreaterThanOrEqual(2);
    console.log(`anchor: ${same} leading dividers held (${ya.length} -> ${yb.length})`);
  }
}, 300_000);
