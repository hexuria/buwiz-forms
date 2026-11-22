// The calendar button beside a date printed in split boxes (old form_page.js pickDate): a day picked fills the parts that are open.
// Through the real door, against a scratch database and the demo taxpayer (Juana Santos).
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO, createDemoTaxpayer, warmForms } from "../../bir/testing";
import { dateRange, splitDay } from "./rows";
import type { FormSchema } from "./contract";
import { dateParts, oneDate } from "./schema";
import { FormSession, type FormTarget, type SessionHooks } from "./session";

// The first form call of a process builds every form's schema (the host does it lazily, once per process); a test's JS call has 2 s on the
// headless runner, a file's load 10 s.
warmForms();

async function opened(target: FormTarget, hooks?: SessionHooks): Promise<FormSession> {
  createDemoTaxpayer();
  const s = new FormSession(target, hooks);
  expect(await s.open()).toBe(null);
  await s.idle();
  return s;
}

/** Ticks a radio of the For Calendar / Fiscal pair before a year-end date. The host keeps the basis per taxpayer and year, so a test says which it wants. */
async function tick(s: FormSession, key: string): Promise<void> {
  s.pickRadio(key);
  await s.idle();
}

test("0605 registered date uses the same unrestricted calendar as its parts", async () => {
  const s = await opened({ tin: DEMO.tin, formCode: "0605", year: 2023, period: 8, mode: "return" });
  try {
    const item = s.item(s.itemIdForKey("frm0605:txtDueDateMonth")!)!;
    const date = oneDate(item)!;
    expect(dateRange(s, date)).toEqual({});
    s.edit(date.oneKey, "2023-08-17");
    s.blur(date.oneKey);
    await s.idle();
    expect(s.shownOf(date.oneKey)).toBe("August 17, 2023");
    expect([s.valueOf(date.month.k), s.valueOf(date.day!.k), s.valueOf(date.year.k)]).toEqual(["08", "17", "2023"]);
    s.edit(date.oneKey, "1900-02-28");
    s.blur(date.oneKey);
    await s.idle();
    expect([s.valueOf(date.month.k), s.valueOf(date.day!.k), s.valueOf(date.year.k), s.boxMessageOf(date.month.k)])
      .toEqual(["02", "28", "1900", ""]);
    expect(splitDay(s, date)).toBe("1900-02-28");
  } finally {
    s.dispose();
  }
});

test("1707A one-date rejects a typed non-December calendar year end", async () => {
  const s = await opened({ tin: DEMO.tin, formCode: "1707A", year: 2025, period: 0, mode: "return" });
  try {
    const month = "frm1707Av2021:txtI1YearEndMonth";
    const date = oneDate(s.item(s.itemIdForKey(month)!)!)!;
    await tick(s, "frm1707Av2021:rdoI1Calendar");
    expect(s.calendarYearEnd(date)).toBe(true);
    const before = [s.valueOf(month), s.valueOf(date.day!.k), s.valueOf(date.year.k)];
    s.edit(date.oneKey, "06/15/2025");
    s.blur(date.oneKey);
    await s.idle();
    expect(s.boxMessageOf(month)).toMatch(/December/);
    expect([s.valueOf(month), s.valueOf(date.day!.k), s.valueOf(date.year.k)]).toEqual(before);
    s.edit(date.oneKey, "12/15/2025");
    s.blur(date.oneKey);
    await s.idle();
    expect([s.valueOf(month), s.valueOf(date.day!.k), s.valueOf(date.year.k)].map(Number)).toEqual([12, 31, 2025]);
    expect(s.shownOf(date.oneKey)).toBe("December 31, 2025");
  } finally {
    s.dispose();
  }
});

test("1706 item 1 (month, day and year boxes): a picked day fills all three", async () => {
  const s = await opened({ tin: DEMO.tin, formCode: "1706", year: 2025, period: 0, mode: "return" });
  const id = s.itemIdForKey("frm1706:txtDateMonth")!;
  const date = dateParts(s.item(id)!.f || [])!;
  expect([date.month.k, date.day?.k, date.year.k]).toEqual(["frm1706:txtDateMonth", "frm1706:txtDateDay", "frm1706:txtDateYear"]);
  s.pickDate(date, "2025-03-15");
  expect(Number(s.valueOf("frm1706:txtDateMonth"))).toBe(3);
  expect(Number(s.valueOf("frm1706:txtDateDay"))).toBe(15);
  expect(Number(s.valueOf("frm1706:txtDateYear"))).toBe(2025);
  await s.idle();
  expect(Number(s.valueOf("frm1706:txtDateMonth"))).toBe(3);
  expect(Number(s.valueOf("frm1706:txtDateDay"))).toBe(15);
  s.dispose();
});

test("2551Q item 2 (a month list and a locked year): the pick sets the month by its listed value and leaves the year", async () => {
  const s = await opened({ tin: DEMO.tin, formCode: "2551Q", year: 2025, period: 1, mode: "return" });
  await tick(s, "frm2551Qv2018:forThe_2");
  const id = s.itemIdForKey("frm2551Qv2018:rtnMonth")!;
  const date = dateParts(s.item(id)!.f || [])!;
  expect(date.day).toBe(null);
  expect(s.box("frm2551Qv2018:txtYear").locked()).toBe(true);
  // The year is locked: a locked part is skipped, so the pick fills the open ones (June, here of 2025). That is why the page's calendar
  // is limited to the locked year (dateRange, the next tests): 2024-06-15 is never offered there.
  s.pickDate(date, "2024-06-15");
  expect(s.valueOf("frm2551Qv2018:rtnMonth")).toBe("06");
  expect(s.valueOf("frm2551Qv2018:txtYear")).toBe("2025");
  await s.idle();
  s.dispose();
});

test("2551Q item 2 (Fiscal): the calendar is limited to the locked year (min and max), and every day in that year fills the whole date", async () => {
  const s = await opened({ tin: DEMO.tin, formCode: "2551Q", year: 2025, period: 1, mode: "return" });
  await tick(s, "frm2551Qv2018:forThe_2");
  const date = dateParts(s.item(s.itemIdForKey("frm2551Qv2018:rtnMonth")!)!.f || [])!;
  expect(dateRange(s, date)).toEqual({ min: "2025-01-01", max: "2025-12-31" });
  s.pickDate(date, "2025-03-31");
  expect(s.valueOf("frm2551Qv2018:rtnMonth")).toBe("03");
  expect(s.valueOf("frm2551Qv2018:txtYear")).toBe("2025");
  await s.idle();
  s.dispose();
});

test("0619E Due Date: a locked month and year limit the calendar to that month, the open day takes the pick", async () => {
  const s = await opened({ tin: DEMO.tin, formCode: "0619E", year: 2025, period: 4, mode: "return" });
  const date = dateParts(s.item(s.itemIdForKey("frm0619E:txtDueMonth")!)!.f || [])!;
  expect([s.valueOf(date.month.k), s.valueOf(date.year.k), s.box(date.month.k).locked(), s.box(date.year.k).locked(), s.box(date.day!.k).locked()]).toEqual(["05", "2025", true, true, false]);
  expect(dateRange(s, date)).toEqual({ min: "2025-05-01", max: "2025-05-31" });
  s.pickDate(date, "2025-05-17");
  expect(Number(s.valueOf(date.day!.k))).toBe(17);
  expect(s.valueOf(date.month.k)).toBe("05");
  await s.idle();
  s.dispose();
});

test("a month with a leap day: February of 2024 ends on the 29th; a year printed as 20YY (1702Q) holds two digits and still limits to 2025", async () => {
  const feb = await opened({ tin: DEMO.tin, formCode: "0619E", year: 2024, period: 1, mode: "return" });
  const due = dateParts(feb.item(feb.itemIdForKey("frm0619E:txtDueMonth")!)!.f || [])!;
  expect(feb.valueOf(due.month.k)).toBe("02");
  expect(dateRange(feb, due)).toEqual({ min: "2024-02-01", max: "2024-02-29" });
  feb.dispose();

  const q = await opened({ tin: DEMO.tin, formCode: "1702Q", year: 2025, period: 1, mode: "return" });
  const ended = dateParts(q.item(q.itemIdForKey("frm1702q:txtYrEndYear")!)!.f || [])!;
  expect([q.valueOf(ended.year.k), q.box(ended.year.k).locked(), q.box(ended.month.k).locked()]).toEqual(["25", true, false]);
  await tick(q, "frm1702q:rbForClndrFscl_2");
  expect(dateRange(q, ended)).toEqual({ min: "2025-01-01", max: "2025-12-31" });
  q.dispose();
});

test("1707A item 2: the box captioned Year end day is the day, and Year ended the year", async () => {
  const fields = ((bir.screen_form_schema("1707A") as FormSchema).parts ?? []).flatMap((part) => part.items ?? []).map((it) => it.f || []);
  const date = dateParts(fields.find((fs) => fs.some((f) => f.k === "frm1707Av2021:txtI1YearEndMonth"))!)!;
  expect(date.month.k).toBe("frm1707Av2021:txtI1YearEndMonth");
  expect(date.day?.k).toBe("frm1707Av2021:txtI1YearEndDay");
  expect(date.year.k).toBe("frm1707Av2021:txtI1YearEndYear");
});

test("on no form is a box captioned for a day read as a split date's year, or its month box as anything else", () => {
  let dates = 0;
  for (const form of bir.screen_forms_index()) {
    for (const part of (bir.screen_form_schema(form.c) as FormSchema).parts ?? []) {
      for (const it of part.items ?? []) {
        const date = dateParts(it.f || []);
        if (!date) continue;
        dates++;
        expect(`${form.c}: ${/\bday\b/i.test(date.year.l) ? date.year.l : ""}`).toBe(`${form.c}: `);
        expect(new Set([date.month.k, date.day?.k ?? "none", date.year.k]).size).toBe(3);
      }
    }
  }
  expect(dates).toBeGreaterThan(20);
});

test("nothing locked: the calendar offers every day (1706 Date of Transaction)", async () => {
  const s = await opened({ tin: DEMO.tin, formCode: "1706", year: 2025, period: 0, mode: "return" });
  const date = dateParts(s.item(s.itemIdForKey("frm1706:txtDateMonth")!)!.f || [])!;
  expect(dateRange(s, date)).toEqual({});
  s.dispose();
});

// The For Calendar / Fiscal pair, the Year Ended date after it, and the year as the form prints it (MM/YYYY or MM/20YY).
const YEAR_END: { code: string; period: number; calendar: string; fiscal: string; month: string; year: string }[] = [
  { code: "1702Q", period: 3, calendar: "frm1702q:rbForClndrFscl_1", fiscal: "frm1702q:rbForClndrFscl_2", month: "frm1702q:rbYrEndMonth", year: "25" },
  { code: "2551Q", period: 3, calendar: "frm2551Qv2018:forThe_1", fiscal: "frm2551Qv2018:forThe_2", month: "frm2551Qv2018:rtnMonth", year: "2025" },
  { code: "2550Q", period: 3, calendar: "frm2550qv2024:calendarNo1", fiscal: "frm2550qv2024:fiscalNo1", month: "frm2550qv2024:selectedMonthNo2", year: "2025" },
];

for (const f of YEAR_END) {
  test(`${f.code} item 2: with Calendar year ticked the calendar offers December of the locked year only; Fiscal frees the year; Calendar again limits it`, async () => {
    const s = await opened({ tin: DEMO.tin, formCode: f.code, year: 2025, period: f.period, mode: "return" });
    const date = dateParts(s.item(s.itemIdForKey(f.month)!)!.f || [])!;
    await tick(s, f.calendar);
    expect(s.valueOf(f.calendar)).toBe("true");
    expect(s.calendarYearEnd(date)).toBe(true);
    expect(s.valueOf(date.year.k)).toBe(f.year);
    // The month list still offers every month (the form does not narrow it): the calendar is what keeps a day outside December away.
    expect(s.box(date.month.k).options().length).toBe(12);
    expect(dateRange(s, date)).toEqual({ min: "2025-12-01", max: "2025-12-31" });
    await tick(s, f.fiscal);
    expect(s.calendarYearEnd(date)).toBe(false);
    expect(dateRange(s, date)).toEqual({ min: "2025-01-01", max: "2025-12-31" });
    await tick(s, f.calendar);
    expect(dateRange(s, date)).toEqual({ min: "2025-12-01", max: "2025-12-31" });
    s.dispose();
  });
}

test("1702RT sample (the year is open, so the calendar cannot leave the days out): a pick outside December is refused with the reason, December is taken", async () => {
  const said: string[] = [];
  const s = await opened({ tin: DEMO.tin, formCode: "1702RT", year: 0, period: 0, mode: "sample" }, { toast: (message) => void said.push(message) });
  const date = dateParts(s.item(s.itemIdForKey("frm1702RT:ddlPg1I2Month")!)!.f || [])!;
  expect(s.calendarYearEnd(date)).toBe(true);
  expect(s.box(date.year.k).locked()).toBe(false);
  expect(dateRange(s, date)).toEqual({});
  // The host puts December back at every fill of a calendar year (1702RT, 1702EX, 1702Q): nothing is written, and the filer is told why.
  s.pickDate(date, "2024-06-15");
  await s.idle();
  expect(said).toEqual(["A calendar year ends in December. Choose Fiscal to pick another month."]);
  expect([s.valueOf(date.month.k), s.valueOf(date.year.k)]).toEqual(["", ""]);
  // A December day fills the date.
  s.pickDate(date, "2024-12-15");
  await s.idle();
  expect(said.length).toBe(1);
  expect(s.valueOf(date.month.k)).toBe("12");
  expect(Number(s.valueOf(date.year.k))).toBe(24);
  s.dispose();
});

test("the calendar opens on the month the boxes hold: MM/YYYY and MM/20YY (a two-digit year is read as 20YY), and on nothing when they hold none", async () => {
  const q = await opened({ tin: DEMO.tin, formCode: "1702Q", year: 2024, period: 1, mode: "return" });
  const ended = dateParts(q.item(q.itemIdForKey("frm1702q:txtYrEndYear")!)!.f || [])!;
  expect([q.valueOf(ended.month.k), q.valueOf(ended.year.k)]).toEqual(["12", "24"]);
  expect(splitDay(q, ended)).toBe("2024-12-01");
  await tick(q, "frm1702q:rbForClndrFscl_2");
  q.choose({ [ended.month.k]: "06" });
  await q.idle();
  expect(splitDay(q, ended)).toBe("2024-06-01");
  q.dispose();

  const r = await opened({ tin: DEMO.tin, formCode: "2551Q", year: 2024, period: 2, mode: "return" });
  const year = dateParts(r.item(r.itemIdForKey("frm2551Qv2018:rtnMonth")!)!.f || [])!;
  expect(splitDay(r, year)).toBe("2024-12-01");
  r.dispose();

  const blank = await opened({ tin: DEMO.tin, formCode: "1702RT", year: 0, period: 0, mode: "sample" });
  const none = dateParts(blank.item(blank.itemIdForKey("frm1702RT:ddlPg1I2Month")!)!.f || [])!;
  expect(splitDay(blank, none)).toBe("");
  blank.dispose();
});
