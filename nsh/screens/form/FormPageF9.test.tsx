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

test("F9-3 desktop: a date in split boxes has a calendar button beside them (2551Q Year Ended), a date box has only its own", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 1 } } });
  await app.waitFor("id:fp-parts-toggle");
  await app.waitFor("id:fp-date-0:1");
  expect(app.count("id:fp-date-0:0")).toBe(0);
  await app.press("id:fp-date-0:1");
  // The popup opens on the month the boxes hold.
  await app.waitFor("text:Su");
  expect(app.shows("text:Dec")).toBe(true);
});

test("F9-3 desktop: the calendar beside a split date with a locked year (2551Q 2025, Fiscal) offers no day outside that year", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 1 } } });
  await app.waitFor("id:fp-parts-toggle");
  await app.waitFor("id:fp-date-0:1");
  // A calendar year ends in December and has only that month to pick (the next test): Fiscal, so the whole year is the filer's.
  await app.press("text:Fiscal");
  expect(app.texts()).toContain("12 - December");
  await app.press("id:fp-date-0:1");
  await app.waitFor("text:Su");
  // The popup opens on December 2025. A day of November 2025 is inside the year: the month box takes it ...
  await app.press("label:Previous month");
  await app.press("text:15");
  await app.waitFor("text:11 - November");
  // ... but a day past the end of the year is out of reach: go on to February 2026 and a press on its 15th does nothing (it used to
  // fill the month list with February and leave the locked year, half a date).
  await app.press("id:fp-date-0:1");
  await app.waitFor("text:Su");
  for (let i = 0; i < 3; i++) await app.press("label:Next month");
  await app.press("text:15").catch(() => {});
  await app.wait(300);
  expect(app.texts()).toContain("11 - November");
  expect(app.texts()).not.toContain("02 - February");
});

test("F9-4 desktop: a locked box's hint is shown whole, in a box that grows to it (0619E Due Date MM and YYYY)", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "0619E", year: 0, period: 0, mode: "sample" } } });
  await app.waitFor("id:fp-parts-toggle");
  // The hint is its own text (not a one-line cut), and the page says it in full.
  expect(app.shows("id:fp-frm0619E:txtDueMonth-hint")).toBe(true);
  expect(app.shows("id:fp-frm0619E:txtDueYear-hint")).toBe(true);
  expect(app.texts().filter((t) => t === "Shows once the period is set").length).toBe(2);
});

test("F14 desktop: a calendar year (Calendar ticked, the default) has only December to pick: the days of another month are out of reach", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 1 } } });
  await app.waitFor("id:fp-parts-toggle");
  await app.waitFor("id:fp-date-0:1");
  await app.press("id:fp-date-0:1");
  await app.waitFor("text:Su");
  // December 2025 is open and its days are there to pick.
  expect(app.texts()).toContain("Dec");
  expect(app.enabled("text:15")).toBe(true);
  // November 2025 is inside the locked year, but not the calendar year's last month: no day of it can be picked.
  await app.press("label:Previous month");
  await app.waitFor("text:Nov");
  expect(app.enabled("text:15")).toBe(false);
  expect(app.texts()).toContain("12 - December");
});

test("F14 desktop: the calendar beside a Year Ended printed MM/20YY (1702Q) opens on the month and year the boxes hold, not on today's", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "1702Q", year: 2024, period: 1 } } });
  await app.waitFor("id:fp-parts-toggle");
  await app.waitFor("id:fp-date-0:1");
  expect(app.texts()).toContain("12 - December");
  expect(app.texts()).toContain("24");
  await app.press("id:fp-date-0:1");
  await app.waitFor("text:Su");
  expect(app.texts()).toContain("Dec");
  expect(app.texts()).toContain("2024");
});
