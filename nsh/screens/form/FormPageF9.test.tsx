// F9 fixes of the side-by-side comparison and the 43-form sweep (FormPageAny.tsx, the demo taxpayer, a scratch database).
import { expect, open, test } from "nsh/test";
import { warmForms } from "../../bir/testing";

// The first form call of a process builds every form's schema; a new app has 5 s to render on the headless runner.
warmForms();

const ENTRY = "./FormPageAny.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };
const LONG = "Schedule 1 · Computation of Tax (Attach additional sheet/s, if necessary)";

test("F9-2 desktop: Compact form sections off (the default): the side list names each part in full", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 1 } } });
  await app.waitFor("id:fp-parts-toggle");
  expect(app.shows(`id:fp-nav-4 >> text:${LONG}`)).toBe(true);
  expect(app.count("id:fp-nav-4 >> text:/^Sch\\. 1/")).toBe(0);
});

test("F9-2 desktop: Compact form sections on: one shortened line per part (old compactPartName)", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 1, compact: true } } });
  await app.waitFor("id:fp-parts-toggle");
  expect(app.shows("id:fp-nav-4 >> text:Sch. 1 · Computation of Tax")).toBe(true);
  expect(app.count(`id:fp-nav-4 >> text:${LONG}`)).toBe(0);
});

test("F9-3 desktop: the Year Ended pair is one period field; its month grid opens on the held year with the stepper still (2551Q)", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 1 } } });
  await app.waitFor("id:fp-parts-toggle");
  await app.waitFor("id:fp-frm2551Qv2018:rtnMonth#period");
  // The pair's own boxes never draw on their own.
  expect(app.shows("id:fp-frm2551Qv2018:rtnMonth")).toBe(false);
  expect(app.shows("id:fp-frm2551Qv2018:txtYear")).toBe(false);
  const cal = "frm2551Qv2018:rtnMonth#period-calendar";
  await app.press(`id:${cal}`);
  await app.waitFor(`id:${cal}-m12`);
  // The grid stands on the held year 2025; the year is the return's, so the stepper has nowhere to go.
  expect(app.texts()).toContain("2025");
  expect(app.enabled(`id:${cal}-prev`)).toBe(false);
  expect(app.enabled(`id:${cal}-next`)).toBe(false);
});

test("F9-3 desktop: Fiscal frees every month of the held year for the period's month box (2551Q)", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 1 } } });
  await app.waitFor("id:fp-parts-toggle");
  await app.waitFor("id:fp-frm2551Qv2018:rtnMonth#period");
  // A calendar year ends in December and has only that month to pick (the next test): Fiscal, so the whole year is the filer's.
  await app.press("text:Fiscal");
  const cal = "frm2551Qv2018:rtnMonth#period-calendar";
  await app.press(`id:${cal}`);
  await app.waitFor(`id:${cal}-m11`);
  expect(app.enabled(`id:${cal}-m11`)).toBe(true);
  expect(app.enabled(`id:${cal}-m1`)).toBe(true);
  await app.press(`id:${cal}-m11`);
  // The month box takes the pick; the locked year stays the return's.
  expect(await app.value("id:fp-frm2551Qv2018:rtnMonth#period")).toBe("11");
  expect(app.texts()).toContain("2025");
});

test("F9-4 desktop: a partially locked date shows its period hint whole (0619E Due Date)", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "0619E", year: 0, period: 0, mode: "sample" } } });
  await app.waitFor("id:fp-parts-toggle");
  expect(app.shows("id:fp-frm0619E:txtDueMonth#date")).toBe(true);
  expect(app.shows("id:fp-frm0619E:txtDueMonth#date-hint")).toBe(true);
  expect(app.texts().filter((t) => t === "Shows once the period is set").length).toBe(1);
});

test("F14 desktop: a calendar year offers only December in the period's month grid (2551Q)", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 1 } } });
  await app.waitFor("id:fp-parts-toggle");
  await app.waitFor("id:fp-frm2551Qv2018:rtnMonth#period");
  const cal = "frm2551Qv2018:rtnMonth#period-calendar";
  await app.press(`id:${cal}`);
  await app.waitFor(`id:${cal}-m12`);
  // December 2025 is open; every other month of the calendar year is out of reach.
  expect(app.enabled(`id:${cal}-m12`)).toBe(true);
  expect(app.enabled(`id:${cal}-m11`)).toBe(false);
  expect(app.enabled(`id:${cal}-m1`)).toBe(false);
});

test("F14 desktop: the month grid beside a Year Ended printed MM/20YY (1702Q) opens on the held year, December the only pick", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "1702Q", year: 2024, period: 1 } } });
  await app.waitFor("id:fp-parts-toggle");
  await app.waitFor("id:fp-frm1702q:rbYrEndMonth#period");
  const cal = "frm1702q:rbYrEndMonth#period-calendar";
  await app.press(`id:${cal}`);
  await app.waitFor(`id:${cal}-m12`);
  // The grid stands on the year the boxes hold ("24" is 2024), not today's.
  expect(app.texts()).toContain("2024");
  expect(app.enabled(`id:${cal}-m12`)).toBe(true);
  expect(app.enabled(`id:${cal}-m11`)).toBe(false);
});
