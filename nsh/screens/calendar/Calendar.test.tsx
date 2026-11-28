// The Calendar on a desktop window (1440 by 900): the filter bar, the List, the Month view, the empty states, opening a return.
// Data is generated (screens/calendar/seed.ts): the demo taxpayer Juana Santos, a branch of hers and Sample Trading Corp. The
// expectations come from the host's own deadlines, read in this file's database with the same seed, so they hold on any day.
import { expect, open, test } from "nsh/test";
import { bir } from "../../bir";
import { isoDate, MONTHS_LONG } from "../../lib/dates";
import { calendarRows, countText, type Filters, gridCells, gridRange, groupRows, inMonthCount, longDay, rowKey } from "./rows";
import { dueThisMonth, SECOND_ROOT, seed, YEAR } from "./seed";

const ENTRY = "../../CalendarDemo.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" } as const;
const asking = (params: Record<string, unknown> = {}) => ({ ...DESKTOP, host: { params } });

seed({ branch: true, second: true });
const all = bir.deadlines_between(`${YEAR}-01-01`, `${YEAR + 1}-12-31`);
const start: Filters = { tps: null, freq: "all", month: 0, quarter: 0, year: YEAR };
const yearRows = calendarRows(start, all);
const JUANA = "000123453";
const headOffice = (code: string, period: number) =>
  all.find((d) => d.form_code === code && d.period === period && d.tin_root === JUANA && d.branch_code === "00000" && d.taxable_year === YEAR)!;

test("the List groups every deadline of the year by due month, under the filter bar", async () => {
  const app = await open(ENTRY, asking({ branch: true, second: true }));
  await app.waitFor("id:calendar-list");
  const texts = app.texts();
  expect(texts).toContain("Calendar");
  expect(texts).toContain("Every deadline for the forms your taxpayers file, from the BIR tax calendar.");
  for (const word of ["All taxpayers", "All", "Yearly", "Quarterly", "Monthly", "List", "Month", String(YEAR)]) expect(texts).toContain(word);
  expect(app.shows("id:calendar-filters")).toBe(true);
  expect(app.shows("id:calendar-quarter")).toBe(false);
  expect(app.shows("id:calendar-month")).toBe(false);

  // The count line, and a heading for each month something falls due in (a December return is due in January of the next year).
  expect(texts).toContain(countText(start, yearRows));
  expect(texts).toContain(`${yearRows.length} deadlines · 2 taxpayers · ${YEAR} returns`);
  const groups = groupRows(start, yearRows);
  expect(groups.length).toBeGreaterThan(6);
  for (const g of groups) expect(texts).toContain(g.label);
  // A taxpayer with branches is named with the branch; a row says what it is and when it is due.
  expect(texts).toContain("Juana Santos · Cebu branch");
  expect(texts).toContain("Sample Trading Corp");
  expect(texts).toContain("1601C · January " + YEAR);
  expect(app.shows(`id:calendar-row-${rowKey(headOffice("1601C", 1))}`)).toBe(true);
}, 60_000);

test("a dry-run send's row says Dry run and stays due (a late or soon chip); a real send's says Filed and is done", async () => {
  // A row of the List (this tax year) due this month: the fixture answers those as sent.
  const row = yearRows.find((d) => dueThisMonth(d))!;
  expect(row).toBeTruthy();
  const id = `id:calendar-row-${rowKey(row)}`;
  const dry = await open(ENTRY, asking({ branch: true, second: true, sent: "dry" }));
  await dry.waitFor(id);
  expect(dry.shows(`${id} >> text:Test send: not filed`)).toBe(true);
  expect(dry.shows(`${id} >> text:Filed`)).toBe(false);
  // This month's deadline is past or within 14 days: a dry run sent nothing, so it says how late or soon it is.
  expect(dry.shows(`${id} >> text:/^(\\d+ days? overdue|Due today|In \\d+ days?)$/`)).toBe(true);
  const real = await open(ENTRY, asking({ branch: true, second: true, sent: "real" }));
  await real.waitFor(id);
  expect(real.shows(`${id} >> text:Filed`)).toBe(true);
  expect(real.shows(`${id} >> text:Test send: not filed`)).toBe(false);
  // Done: no chip of lateness (a later day's plain "In N days" is its date, not a chip, so only a past one is checked).
  if (row.due_date < isoDate(new Date())) expect(real.shows(`${id} >> text:/overdue$/`)).toBe(false);
}, 90_000);

test("a row opens its return, and back returns to the Calendar as it was", async () => {
  const app = await open(ENTRY, asking({ branch: true, second: true }));
  await app.waitFor("id:calendar-list");
  await app.press("label:Quarterly");
  await app.waitFor("id:calendar-quarter");
  const target = headOffice("2551Q", 1);
  await app.press(`id:calendar-row-${rowKey(target)}`);
  await app.waitFor("id:form-page");
  expect(app.shows("id:calendar")).toBe(false);
  await app.press("id:tb-back");
  await app.waitFor("id:calendar-list");
  // The filters outlive the view.
  expect(app.shows("id:calendar-quarter")).toBe(true);
}, 90_000);

test("the taxpayer filter: pick one, none, all; the count follows", async () => {
  const app = await open(ENTRY, asking({ second: true }));
  await app.waitFor("id:calendar-list");
  // This database has Juana's second branch too; the window's has not.
  const both = calendarRows(start, all.filter((d) => d.branch_code !== "00001"));
  expect(app.texts()).toContain(countText(start, both));
  await app.press("id:calendar-taxpayers-trigger");
  await app.waitFor(`id:calendar-taxpayers-opt-${SECOND_ROOT}`);
  expect(app.shows(`id:calendar-taxpayers-opt-${JUANA}`)).toBe(true);
  // Unchecking the second taxpayer leaves hers.
  await app.press(`id:calendar-taxpayers-opt-${SECOND_ROOT}`);
  const mine = both.filter((d) => d.tin_root === JUANA);
  await app.waitFor(`text:${countText(start, mine)}`);
  expect(app.texts()).toContain("Juana Santos");
  // Clear leaves none: the empty state, with its button.
  await app.press("id:calendar-taxpayers-clear");
  await app.waitFor("text:No deadlines match these filters.");
  expect(app.texts()).toContain("No taxpayers");
  expect(app.texts()).toContain("0 deadlines · 0 taxpayers · " + YEAR + " returns");
  // Select all brings every taxpayer back.
  await app.press("id:calendar-taxpayers-all");
  await app.waitFor(`text:${countText(start, both)}`);
  expect(app.texts()).toContain("All taxpayers");
  // The search narrows the choices.
  await app.type("id:calendar-taxpayers-search", "sample");
  await app.waitFor(`id:calendar-taxpayers-opt-${SECOND_ROOT}`);
  expect(app.shows(`id:calendar-taxpayers-opt-${JUANA}`)).toBe(false);
  // Esc closes the choices (the old page's Esc closed its combo box first).
  await app.keyboard.press("escape");
  await app.waitFor("id:calendar-taxpayers-search", { state: "gone" });
}, 90_000);

test("how often: Quarterly shows quarter chips and groups by covered quarter; Monthly shows month chips", async () => {
  const app = await open(ENTRY, asking({ branch: true, second: true }));
  await app.waitFor("id:calendar-list");
  const quarterly: Filters = { ...start, freq: "quarterly" };
  await app.press("label:Quarterly");
  await app.waitFor("id:calendar-quarter");
  for (const word of ["All quarters", "Q1", "Q2", "Q3", "Q4"]) expect(app.texts()).toContain(word);
  const qRows = calendarRows(quarterly, all);
  for (const g of groupRows(quarterly, qRows)) {
    expect(app.texts()).toContain(g.title);
    expect(app.texts()).toContain(g.caption);
  }
  // A quarter chip keeps one covered quarter, and says which months it covers.
  await app.press("label:Q2");
  await app.waitFor("text:Returns for April to June");
  const q2: Filters = { ...quarterly, quarter: 2 };
  expect(app.texts()).toContain(countText(q2, calendarRows(q2, all)));
  expect(app.texts()).toContain(`Q2 ${YEAR} returns`);
  expect(app.texts()).not.toContain(`Q1 ${YEAR} returns`);

  // Monthly drops the quarter chip; a month chip keeps what is due in that month.
  await app.press("label:Monthly");
  await app.waitFor("id:calendar-month");
  expect(app.shows("id:calendar-quarter")).toBe(false);
  expect(app.texts()).not.toContain("Returns for April to June");
  expect(app.texts()).toContain("All months");
  await app.press("label:Feb");
  await app.waitFor("text:Forms due in February");
  const feb: Filters = { ...start, freq: "monthly", month: 2 };
  expect(app.texts()).toContain(countText(feb, calendarRows(feb, all)));
  expect(app.texts()).toContain(`February ${YEAR}`);
  expect(app.texts()).not.toContain(`March ${YEAR}`);
  // The month chip's year is the due year: a December return shows under January of the next year.
  await app.press("id:calendar-year-next");
  await app.press("label:Jan");
  await app.waitFor("text:Forms due in January");
  const jan: Filters = { ...start, year: YEAR + 1, freq: "monthly", month: 1 };
  expect(app.texts()).toContain(countText(jan, calendarRows(jan, all)));
  expect(app.texts()).toContain("1601C · December " + YEAR);

  // Yearly has no deadline in this data: the empty state offers Clear filters, which resets everything.
  await app.press("label:Yearly");
  await app.waitFor("text:No deadlines match these filters.");
  await app.press("id:calendar-clear");
  await app.waitFor(`text:${countText(start, yearRows)}`);
  expect(app.shows("id:calendar-month")).toBe(false);
}, 90_000);

test("the year stepper moves the tax year inside the years the app has", async () => {
  const app = await open(ENTRY, asking({ branch: true, second: true }));
  await app.waitFor("id:calendar-list");
  expect(app.enabled("id:calendar-year-next")).toBe(true); // the host allows one year ahead
  expect(app.enabled("id:calendar-year-prev")).toBe(true);
  await app.press("id:calendar-year-next");
  await app.waitFor(`text:${YEAR + 1}`);
  expect(app.enabled("id:calendar-year-next")).toBe(false);
  // Nothing covers next year yet: the empty state, since there are deadlines in the data.
  await app.waitFor("text:No deadlines match these filters.");
  expect(app.texts()).toContain(`0 deadlines · 0 taxpayers · ${YEAR + 1} returns`);
  await app.press("id:calendar-year-prev");
  await app.waitFor(`text:${countText(start, yearRows)}`);
  await app.press("id:calendar-year-prev");
  await app.waitFor(`text:0 deadlines · 0 taxpayers · ${YEAR - 1} returns`);
}, 90_000);

test("empty states: no taxpayer yet; a taxpayer with no tax year; filters that match nothing", async () => {
  const fresh = await open(ENTRY, asking({ demo: false }));
  await fresh.waitFor("id:calendar");
  await fresh.waitFor("id:calendar-empty-row");
  expect(fresh.texts()).toContain("Deadlines show here once a taxpayer has forms for a tax year.");
  expect(fresh.shows("id:calendar-filters")).toBe(true);
  expect(fresh.shows("id:calendar-list")).toBe(false);
  expect(fresh.shows("id:calendar-clear")).toBe(false);

  const bare = await open(ENTRY, asking({ years: false }));
  await bare.waitFor("id:calendar-list");
  expect(bare.texts()).toContain(`0 deadlines · 0 taxpayers · ${YEAR} returns`);
  expect(bare.texts()).toContain("Deadlines show here once a taxpayer has forms for a tax year.");
  expect(bare.shows("id:calendar-clear")).toBe(false);
  // Once a filter is on, the line says the filters match nothing and offers to clear them.
  await bare.press("label:Quarterly");
  await bare.waitFor("id:calendar-clear");
  expect(bare.texts()).toContain("No deadlines match these filters.");
  await bare.press("id:calendar-clear");
  await bare.waitFor("text:Deadlines show here once a taxpayer has forms for a tax year.");
  expect(bare.shows("id:calendar-clear")).toBe(false);
}, 90_000);

test("the Month view: a six-week grid, today's ring, a day's deadlines beside it, and stepping months", async () => {
  const now = new Date();
  const y = now.getFullYear();
  const m = now.getMonth();
  const [from, to] = gridRange(y, m);
  const rows = bir.deadlines_between(from, to);
  const cells = gridCells(y, m, rows, now);
  const busy = cells.filter((c) => c.items.length > 0);
  expect(busy.length).toBeGreaterThan(0);
  const day = busy.find((c) => c.inMonth) ?? busy[0];

  const app = await open(ENTRY, asking({ branch: true, second: true }));
  await app.waitFor("id:calendar-list");
  await app.press("label:Month");
  await app.waitFor("id:calendar-grid");
  expect(app.shows("id:calendar-year")).toBe(false); // the year stepper belongs to the List
  expect(app.texts()).toContain(`${MONTHS_LONG[m]} ${y}`);
  expect(app.texts()).toContain(`${inMonthCount(y, m, rows)} deadlines this month`.replace("1 deadlines", "1 deadline"));
  for (const w of ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"]) expect(app.texts()).toContain(w);
  expect(cells).toHaveLength(42);
  for (const c of cells) expect(app.shows(`id:calendar-day-${c.iso}`)).toBe(true);
  expect(app.shows("id:calendar-today-ring")).toBe(true);
  // Today starts chosen: its day's deadlines (or nothing) are beside the grid.
  const today = isoDate(now);
  expect(app.texts()).toContain(longDay(today));

  // A chip names the form and the taxpayer's initials; a busy day shows two and counts the rest.
  expect(app.texts()).toContain("1601C JS");
  const more = busy.find((c) => c.items.length > 2);
  if (more) expect(app.texts()).toContain(`+${more.items.length - 2} more`);

  // Choosing a day lists its deadlines: code, period over the taxpayer, the status chip.
  await app.press(`id:calendar-day-${day.iso}`);
  await app.waitFor(`text:${longDay(day.iso)}`);
  for (const d of day.items) expect(app.shows(`id:calendar-day-row-${rowKey(d)}`)).toBe(true);
  expect(app.shows("id:calendar-day-empty")).toBe(false);
  // The taxpayer and how-often filters apply here too.
  await app.press("label:Quarterly");
  await app.waitFor(`text:${longDay(day.iso)}`);
  const quarterly = day.items.filter((d) => d.frequency === "Quarterly");
  expect(app.shows("id:calendar-day-empty")).toBe(quarterly.length === 0);
  await app.press("label:All");

  // Another month: the title moves, no day is chosen, and Today brings today's month and day back.
  await app.press("id:calendar-prev-month");
  const prev = new Date(y, m - 1, 1);
  await app.waitFor(`text:${MONTHS_LONG[prev.getMonth()]} ${prev.getFullYear()}`);
  await app.waitFor("text:Pick a day to see what is due.");
  expect(app.shows("id:calendar-today-ring")).toBe(false);
  await app.press("id:calendar-next-month");
  await app.waitFor(`text:${longDay(today)}`);
  await app.press("id:calendar-next-month");
  await app.waitFor("text:Pick a day to see what is due.");
  await app.press("id:calendar-today");
  await app.waitFor(`text:${longDay(today)}`);
  expect(app.texts()).toContain(`${MONTHS_LONG[m]} ${y}`);

  // A day's row opens its return.
  await app.press(`id:calendar-day-${day.iso}`);
  await app.waitFor(`id:calendar-day-row-${rowKey(day.items[0])}`);
  await app.press(`id:calendar-day-row-${rowKey(day.items[0])}`);
  await app.waitFor("id:form-page");
}, 120_000);

test("the Month view is the start when asked, and the filters stay when the page is left", async () => {
  const app = await open(ENTRY, asking({ view: "month" }));
  await app.waitFor("id:calendar-grid");
  await app.press("label:Monthly");
  await app.press("label:List");
  await app.waitFor("id:calendar-month");
  await app.press("id:nav-home");
  await app.waitFor("id:nav-home");
  expect(app.shows("id:calendar")).toBe(false);
  await app.press("id:nav-calendar");
  await app.waitFor("id:calendar-month");
  expect(app.shows("id:calendar-list")).toBe(true);
}, 90_000);

test("narrowing the window swaps in the phone layout, and widening brings the filter bar back", async () => {
  const app = await open(ENTRY, asking({}));
  await app.waitFor("id:calendar-list");
  await app.setViewport(500, 800);
  await app.waitFor("id:calendar-narrow");
  expect(app.shows("id:calendar-filters")).toBe(false);
  expect(app.shows("id:calendar-filters-open")).toBe(true);
  await app.setViewport(1000, 800);
  await app.waitFor("id:calendar-filters");
  expect(app.shows("id:calendar-narrow")).toBe(false);
}, 90_000);
