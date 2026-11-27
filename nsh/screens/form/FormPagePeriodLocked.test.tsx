// Period pills and period fields on a saved return draw the same refusal the session applies: every choice is locked,
// with one sentence saying why. New and sample returns keep their period choices open. Through the page fixture.
import { expect, open, test } from "nsh/test";
import { warmForms } from "../../bir/testing";

warmForms();

const ENTRY = "./FormPageAny.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };
const Q1 = "id:fp-frm2551Qv2018:qtr_1";
const Q2 = "id:fp-frm2551Qv2018:qtr_2";
const Q3 = "id:fp-frm2551Qv2018:qtr_3";
const Q4 = "id:fp-frm2551Qv2018:qtr_4";
const QUARTER_SENTENCE = "The quarter is set when the return is saved. To file another quarter, start a new return.";
const MONTH_SENTENCE = "The month is set when the return is saved. To file another month, start a new return.";

for (const [name, view] of [["phone", {}], ["desktop", DESKTOP]] as const) {
  test(`${name}: a saved 2551Q locks every quarter pill, says why once, and a press changes nothing`, async () => {
    const app = await open(ENTRY, {
      ...view,
      host: { params: { formCode: "2551Q", year: 2026, period: 1, seed: true, probeKey: "frm2551Qv2018:qtr_2", setup: [{ pick: "frm2551Qv2018:qtr_2" }] } },
    });
    await app.waitFor(Q1);
    for (const q of [Q1, Q2, Q3, Q4]) expect(app.enabled(q)).toBe(false);
    expect(app.texts()).toContain(QUARTER_SENTENCE);
    expect(app.texts().filter((t) => t === QUARTER_SENTENCE).length).toBe(1);
    await app.waitFor("text:probe false", { timeout: 5000 });
    await expect(app.press(Q2)).rejects.toMatchObject({ code: "ERR_NSH_TEST_DISABLED" });
    expect(app.texts()).toContain("probe false");
  });
}

test("desktop: a new 2551Q return leaves the quarter pills pressable", async () => {
  const app = await open(ENTRY, {
    ...DESKTOP,
    host: { params: { formCode: "2551Q", year: 2025, period: 1, probeKey: "frm2551Qv2018:qtr_2" } },
  });
  await app.waitFor(Q1);
  for (const q of [Q1, Q2, Q3, Q4]) expect(app.enabled(q)).toBe(true);
  expect(app.texts()).not.toContain(QUARTER_SENTENCE);
  await app.press(Q2);
  await app.waitFor(`text:${QUARTER_SENTENCE}`, { timeout: 8000 });
  for (const q of [Q1, Q2, Q3, Q4]) expect(app.enabled(q)).toBe(false);
});

test("phone: a 2551Q sample keeps quarter pills pressable before and after its period is set", async () => {
  const app = await open(ENTRY, {
    host: { params: { formCode: "2551Q", year: 0, period: 0, mode: "sample", probeKey: "frm2551Qv2018:qtr_3" } },
  });
  await app.waitFor(Q1);
  for (const q of [Q1, Q2, Q3, Q4]) expect(app.enabled(q)).toBe(true);
  await app.type("id:fp-frm2551Qv2018:rtnMonth#period", "12/2025");
  await app.waitUntil(() => app.enabled(Q3) === true, { timeout: 10000 });
  await app.press(Q3);
  await app.waitFor("text:probe true", { timeout: 8000 });
  expect(app.texts()).not.toContain(QUARTER_SENTENCE);
});

for (const [name, view] of [["phone", {}], ["desktop", DESKTOP]] as const) {
  test(`${name}: a saved 1601C locks its filing period and says why`, async () => {
    const app = await open(ENTRY, {
      ...view,
      host: { params: { formCode: "1601C", year: 2026, period: 9, seed: true } },
    });
    await app.waitFor("id:fp-frm1601c:txtMonth#period");
    expect(app.enabled("id:fp-frm1601c:txtMonth#period")).toBe(false);
    expect(app.texts()).toContain(MONTH_SENTENCE);
    expect(app.texts().filter((t) => t === MONTH_SENTENCE).length).toBe(1);
  });
}
