// F9-1: the form page says what needs the filer at once, as the old page did (app/shell/form_page.js todo(), partErrors,
// drawPill). Same world as the side-by-side pictures (~/BuwizNsh/evidence/parity): the demo taxpayer with tax year 2026
// (1601C, 2551Q) and four saved drafts (FormPageAny.tsx param seed). The old app shows on the 2551Q Q3 2026 draft:
// "2 parts need attention · next ↓", red badges 1 on Part I and 1 on Part II, the pill "2 to do · Page 1" with a red 2,
// and the host's message under Item 13 once Part I is open. Counts never wait for a box to be left; the red text of a
// stored return's opening shows at once; on a return never saved it waits until its row is left or Check.
import { expect, open, test } from "nsh/test";

const ENTRY = "./FormPageAny.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };

async function seeded(formCode: string, period: number) {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode, year: 2026, period, seed: true } } });
  await app.waitFor("id:fp-parts-toggle", { timeout: 15000 });
  return app;
}

test("F9-1 desktop: the saved 2551Q Q3 2026 draft opens with 2 to-dos, two part badges and the Item 13 message", async () => {
  const app = await seeded("2551Q", 3);
  await app.waitFor("text:2 parts need attention · next ↓");
  expect(app.shows("id:fp-nav-1 >> text:1")).toBe(true);
  expect(app.shows("id:fp-nav-2 >> text:1")).toBe(true);
  expect(app.count("id:fp-nav-0 >> text:1")).toBe(0);
  expect(app.texts()).toContain("2 to do · Page 1");
  expect(app.shows("id:fp-pill-count >> text:2")).toBe(true);
  expect(app.shows("label:2 items need attention")).toBe(true);
  // The pill walks them in form order: Item 13 (Part I), then Item 20 (Part II).
  await app.press("id:fp-pill-next");
  await app.waitFor("text:/^Item 13 · /");
  expect(app.texts()).toContain("1 of 2 to do · Part I");
  // The host's message from the opening shows under Item 13 without a box being left.
  await app.waitFor("text:/^This later-quarter Section 116 return needs/");
  await app.press("id:fp-pill-next");
  await app.waitFor("text:/^Item 20 · /");
  expect(app.texts()).toContain("2 of 2 to do · Part II");
});

// 1601C: Item 3 (Return details), Item 11 (Part I) and the EOPT tier at Item 32 (Part II); 2551Q Q1: Item 13 and Item 20.
for (const [code, period, pill, parts] of [
  ["1601C", 9, "3 to do · Page 1", "3 parts need attention · next ↓"],
  ["1601C", 10, "3 to do · Page 1", "3 parts need attention · next ↓"],
  ["2551Q", 1, "2 to do · Page 1", "2 parts need attention · next ↓"],
] as const) {
  test(`F9-1 desktop: the seeded ${code} draft of period ${period} counts its rows with a host message`, async () => {
    const app = await seeded(code, period);
    await app.waitFor(`text:${parts}`);
    expect(app.texts()).toContain(pill);
  });
}

test("F9-1 desktop: a return never saved counts its to-dos at once but shows no red text until Check", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 1 } } });
  await app.waitFor("id:fp-parts-toggle");
  await app.waitFor("id:fp-pill-count");
  expect(app.texts().some((t) => / to do · /.test(t))).toBe(true);
  await app.press("id:fp-part-1-toggle");
  await app.waitFor("text:/^Item 13|Only for individual/");
  expect(app.texts().some((t) => /^Choose the applicable Item 13/.test(t))).toBe(false);
  await app.press("id:fp-check");
  await app.waitFor("text:/^Choose the applicable Item 13/", { timeout: 5000 });
});
