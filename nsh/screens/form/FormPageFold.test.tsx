// B10 on the page. The built schemas keep exactly one "Additional details" fold — 1602Q's
// Schedule 1 tail — so the fold tests run there; the sixteen line-of-business forms' folds were
// dissolved by the layout overrides and their boxes draw in titled rows (2200S below).
import { expect, open, test } from "nsh/test";

const ANY = "./FormPageAny.tsx";
const PREVIEW = "./FormPagePreview.tsx";

test("phone: a shown box sentence inside 1602Q's fold opens it", async () => {
  const app = await open(ANY, {
    host: { params: { formCode: "1602Q", year: 2025, period: 1, edits: { "frm1602Q:txtSched1Amount15": "abc" } } },
  }); // 390 x 844
  await app.waitFor("text:BIR Form 1602Q");
  await app.press("id:fp-part-6-toggle");
  // The refused edit's sentence shows on the box, so the fold is open and the box is drawn.
  await app.waitFor("id:fp-frm1602Q:txtSched1Amount15", { timeout: 15000 });
  expect(app.shows("id:fp-frm1602Q:txtSched1Amount15")).toBe(true);
});

test("phone: a step to 1602Q's fold row opens it and puts the keyboard in a fold box", async () => {
  const app = await open(ANY, {
    host: { params: { formCode: "1602Q", year: 2025, period: 1, stepRow: "6:0" } },
  });
  await app.waitFor("text:BIR Form 1602Q");
  // jumpTo opened part 6 and stepped to the fold row: the fold opened and its one open box
  // (the other three are locked or computed) took the keyboard — a digit lands in it.
  await app.waitFor("id:fp-frm1602Q:txtSched1Amount15", { timeout: 15000 });
  const box = "id:fp-frm1602Q:txtSched1Amount15";
  for (let i = 0; i < 10 && !(app.value(box) ?? "").includes("7"); i++) {
    await app.keyboard.press("7");
    await app.waitUntil(() => (app.value(box) ?? "").includes("7"), { timeout: 1500 }).catch(() => undefined);
  }
  expect(app.value(box) ?? "").toContain("7");
});

test("2200S at 390 and 1440: the line-of-business box shows and its fact card does not (B5/B7)", async () => {
  for (const look of [{}, { width: 1440, height: 900, platform: "macos" as const }]) {
    const app = await open(PREVIEW, { ...look, host: { params: { form: "2200S", period: 1 } } });
    await app.waitFor("text:BIR Form 2200S");
    await app.press("id:fp-check");
    await app.waitFor("id:fp-pill-count");
    await app.press("id:fp-part-1-toggle");
    for (let i = 0; i < 15 && !app.shows("id:fp-frm2200S:txtLineBus"); i++) await app.scroll("id:form-scroll", 400).catch(() => {});
    await app.waitFor("id:fp-frm2200S:txtLineBus", { timeout: 15000 });
    expect(app.shows("id:fp-frm2200S:txtLineBus")).toBe(true);
    // Q1 P1b: the box is the one input — no separate fact card for a boxed fact.
    expect(app.shows("id:fp-ask-line_of_business")).toBe(false);
  }
}, 120_000);
