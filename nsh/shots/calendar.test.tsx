// Real-window screenshots of the Calendar (C3): light and dark, wide and narrow, over generated data (the demo taxpayer Juana Santos,
// a branch of hers and Sample Trading Corp). `just nsh-shots calendar` saves ~/BuwizNsh/evidence/calendar/*.png; headless (plain
// nsh test) it takes no pictures and passes.
import { expect, open, test } from "nsh/test";
import { isoDate } from "../lib/dates";
import { LOOKS, shoot } from "./shoot";

const ENTRY = "../CalendarDemo.tsx";
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });
const DATA = { branch: true, second: true, drafts: true, filed: true };

// The 10th of this month, moved off a weekend: where the monthly returns of the month before fall due (a real-window test file
// cannot call the host, so the day is not read from the data).
const tenth = new Date(new Date().getFullYear(), new Date().getMonth(), 10);
if (tenth.getDay() === 6) tenth.setDate(12);
if (tenth.getDay() === 0) tenth.setDate(11);
const day = isoDate(tenth);

for (const look of LOOKS) {
  const wide = look.width > 600;
  test(`calendar, ${look.name}`, async () => {
    const app = await open(ENTRY, { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme, ...asking(DATA) });
    await app.waitFor(wide ? "id:calendar-list" : "id:calendar-narrow");
    await app.waitFor(wide ? "text:Draft" : "text:/ · Draft$/");
    await app.wait(500);
    if (wide) {
      await shoot(app, `calendar-${look.name}-list`);
      await app.scroll("id:main-scroll", 900);
      await app.wait(300);
      await shoot(app, `calendar-${look.name}-list-scrolled`);
      await app.scroll("id:main-scroll", -2000);
      await app.press("id:calendar-taxpayers-trigger");
      await app.waitFor("id:calendar-taxpayers-search");
      await app.wait(300);
      await shoot(app, `calendar-${look.name}-taxpayers`);
      await app.keyboard.press("escape");
      await app.press("label:Quarterly");
      await app.waitFor("id:calendar-quarter");
      await app.press("label:Q3");
      await app.wait(300);
      await shoot(app, `calendar-${look.name}-quarterly`);
      await app.press("label:Monthly");
      await app.waitFor("id:calendar-month");
      await app.press("label:Oct");
      await app.wait(300);
      await shoot(app, `calendar-${look.name}-monthly`);
      await app.press("label:All");
      await app.press("label:Month");
      await app.waitFor("id:calendar-grid");
      await app.press(`id:calendar-day-${day}`);
      await app.wait(400);
      await shoot(app, `calendar-${look.name}-month`);
      await app.press("label:Quarterly");
      await app.press("id:calendar-prev-month");
      await app.wait(300);
      await shoot(app, `calendar-${look.name}-month-filtered`);
      await app.press("label:All");
      await app.press("label:List");
      await app.waitFor("id:calendar-list");
      await app.press("id:calendar-year-next");
      await app.waitFor("id:calendar-empty-row");
      await app.wait(300);
      await shoot(app, `calendar-${look.name}-empty`);
    } else {
      await shoot(app, `calendar-${look.name}-top`);
      await app.scroll("id:main-scroll", 520);
      await app.wait(300);
      await shoot(app, `calendar-${look.name}-upcoming`);
      await app.scroll("id:main-scroll", -2000);
      await app.press(`id:calendar-day-${day}`);
      await app.wait(400);
      await shoot(app, `calendar-${look.name}-day`);
      await app.press("id:calendar-filters-open");
      await app.waitFor("id:calendar-sheet-done");
      await app.wait(500);
      await shoot(app, `calendar-${look.name}-filters`);
      await app.press("label:Quarterly");
      await app.press("id:calendar-tp-000111225");
      await app.wait(300);
      await shoot(app, `calendar-${look.name}-filters-on`);
      await app.press("id:calendar-sheet-done");
      await app.wait(500);
      await shoot(app, `calendar-${look.name}-filtered`);
    }
    expect(app.shows("id:shell")).toBe(true);
  }, 180_000);
}

for (const look of LOOKS.filter((l) => l.width > 600)) {
  test(`calendar at 700 points, ${look.name}`, async () => {
    const app = await open(ENTRY, { width: 700, height: 800, platform: look.platform, colorScheme: look.colorScheme, ...asking(DATA) });
    await app.waitFor("id:calendar-list");
    await app.waitFor("text:Draft");
    await app.press("label:Monthly");
    await app.waitFor("id:calendar-month");
    await app.wait(400);
    await shoot(app, `calendar-${look.name}-700-monthly`);
    await app.press("label:Month");
    await app.waitFor("id:calendar-grid");
    await app.press(`id:calendar-day-${day}`);
    await app.wait(400);
    await shoot(app, `calendar-${look.name}-700-month`);
    expect(app.shows("id:shell")).toBe(true);
  }, 120_000);
}
