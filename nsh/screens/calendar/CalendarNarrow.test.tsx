// The Calendar on a phone (390 by 844): a month grid with a dot on each due day, a month stepper, the Upcoming list (late first),
// a day that narrows the list, and the taxpayer and how-often filters in a sheet. Generated data (screens/calendar/seed.ts); the
// expectations come from the host's own deadlines read in this file's database with the same seed.
import { expect, open, test } from "nsh/test";
import { bir } from "../../bir";
import { MONTHS_LONG } from "../../lib/dates";
import {
  byWhoAndHowOften, dayTone, type Filters, gridCells, gridRange, inMonthCount, longDay, rowKey, upcomingRange, upcomingRows,
} from "./rows";
import { SECOND_ROOT, seed, YEAR } from "./seed";

const ENTRY = "../../CalendarDemo.tsx";
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });

seed({ branch: true, second: true });
const JUANA = "000123453";
const everyone: Pick<Filters, "tps" | "freq"> = { tps: null, freq: "all" };
const now = new Date();
const [upFrom, upTo] = upcomingRange(now);
const comingFor = (who: Pick<Filters, "tps" | "freq">) => upcomingRows(byWhoAndHowOften(who, bir.deadlines_between(upFrom, upTo)), now);
/** The "<code> · <period>" lines of the Upcoming rows on screen, in order. */
const listed = (texts: string[]) => texts.filter((t) => /^[0-9A-Z]+ · (?:[A-Z][a-z]+ \d{4}|Q\d \d{4}|TY \d{4})$/.test(t));
const lines = (rows: ReturnType<typeof comingFor>) => rows.map((d) => `${d.form_code} · ${d.period_label}`);

test("the grid has a dot on each due day; Upcoming lists what is left, late first", async () => {
  const app = await open(ENTRY, asking({ branch: true, second: true, drafts: true }));
  await app.waitFor("id:calendar-narrow");
  expect(app.shows("id:tab-bar")).toBe(true);
  expect(app.shows("id:calendar-filters")).toBe(false); // the filter bar is the wide layout's
  const texts = app.texts();
  const y = now.getFullYear();
  const m = now.getMonth();
  expect(texts).toContain(`${MONTHS_LONG[m]} ${y}`);
  for (const w of ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"]) expect(texts).toContain(w);
  expect(texts).toContain("Filters");
  expect(texts).toContain("Upcoming");
  expect(app.shows("id:calendar-today-ring")).toBe(true);

  // A dot on every due day of the six weeks shown, and none elsewhere; a late day says so.
  const [from, to] = gridRange(y, m);
  const rows = bir.deadlines_between(from, to);
  const cells = gridCells(y, m, rows, now);
  expect(cells.filter((c) => c.items.length > 0).length).toBeGreaterThanOrEqual(1);
  for (const c of cells) expect(app.shows(`id:calendar-dot-${c.iso}`)).toBe(c.items.length > 0);
  const late = cells.find((c) => dayTone(c.items, now) === "danger");
  expect(late).toBeDefined();
  expect(app.shows(`label:${late!.label}, late`)).toBe(true);
  expect(texts).toContain(`${inMonthCount(y, m, rows)} deadlines this month`.replace("1 deadlines", "1 deadline"));

  // The list: what is still to do, the late ones first, then from today on (the same rows as the wide List).
  const expected = lines(comingFor(everyone));
  expect(expected.length).toBeGreaterThan(8);
  expect(listed(texts)).toEqual(expected);
  const firstLate = comingFor(everyone)[0];
  expect(firstLate.due_date < now.toISOString().slice(0, 10)).toBe(true);
  // The chip says how late a return is; its status follows the taxpayer (the saved drafts read as Draft).
  expect(texts.some((t) => t.endsWith(" · Draft"))).toBe(true);
  expect(texts.some((t) => /^\d+ days overdue$/.test(t))).toBe(true);
  expect(app.shows(`id:calendar-row-${rowKey(firstLate)}`)).toBe(true);
}, 90_000);

test("a day narrows the list to it; a second press, or Show upcoming, lets go", async () => {
  const app = await open(ENTRY, asking({ branch: true, second: true }));
  await app.waitFor("id:calendar-narrow");
  const y = now.getFullYear();
  const m = now.getMonth();
  const [from, to] = gridRange(y, m);
  const cells = gridCells(y, m, bir.deadlines_between(from, to), now);
  const day = cells.find((c) => c.inMonth && c.items.length > 0) ?? cells.find((c) => c.items.length > 0)!;
  const empty = cells.find((c) => c.inMonth && c.items.length === 0)!;

  await app.press(`id:calendar-day-${day.iso}`);
  await app.waitFor(`text:${longDay(day.iso)}`);
  expect(listed(app.texts())).toEqual(day.items.map((d) => `${d.form_code} · ${d.period_label}`));
  expect(app.shows("id:calendar-show-upcoming")).toBe(true);
  expect(app.texts()).not.toContain("Upcoming");
  // Pressing the chosen day lets go of it.
  await app.press(`id:calendar-day-${day.iso}`);
  await app.waitFor("text:Upcoming");
  expect(app.shows("id:calendar-show-upcoming")).toBe(false);

  // A day with nothing due says so; Show upcoming goes back to the list.
  await app.press(`id:calendar-day-${empty.iso}`);
  await app.waitFor("text:Nothing due this day.");
  await app.press("id:calendar-show-upcoming");
  await app.waitFor("text:Upcoming");
  expect(listed(app.texts())).toEqual(lines(comingFor(everyone)));
}, 90_000);

test("the month stepper moves the grid and clears the chosen day; Today comes back", async () => {
  const app = await open(ENTRY, asking({ branch: true, second: true }));
  await app.waitFor("id:calendar-narrow");
  const y = now.getFullYear();
  const m = now.getMonth();
  const next = new Date(y, m + 1, 1);
  const [from, to] = gridRange(y, m);
  const day = gridCells(y, m, bir.deadlines_between(from, to), now).find((c) => c.items.length > 0)!;
  await app.press(`id:calendar-day-${day.iso}`);
  await app.waitFor(`text:${longDay(day.iso)}`);
  await app.press("id:calendar-next-month");
  await app.waitFor(`text:${MONTHS_LONG[next.getMonth()]} ${next.getFullYear()}`);
  await app.waitFor("text:Upcoming"); // the chosen day went with the month
  const [nf, nt] = gridRange(next.getFullYear(), next.getMonth());
  expect(app.texts()).toContain(
    `${inMonthCount(next.getFullYear(), next.getMonth(), byWhoAndHowOften(everyone, bir.deadlines_between(nf, nt)))} deadlines this month`.replace("1 deadlines", "1 deadline"),
  );
  expect(app.shows("id:calendar-today-ring")).toBe(next.getMonth() === now.getMonth()); // only while today is on the grid
  await app.press("id:calendar-prev-month");
  await app.press("id:calendar-prev-month");
  const prev = new Date(y, m - 1, 1);
  await app.waitFor(`text:${MONTHS_LONG[prev.getMonth()]} ${prev.getFullYear()}`);
  await app.press("id:calendar-today");
  await app.waitFor(`text:${MONTHS_LONG[m]} ${y}`);
  expect(app.shows("id:calendar-today-ring")).toBe(true);
}, 90_000);

test("the filter sheet: how often and taxpayers narrow the grid and the list; a count shows on the button", async () => {
  const app = await open(ENTRY, asking({ branch: true, second: true }));
  await app.waitFor("id:calendar-narrow");
  expect(app.shows("id:calendar-filters-count")).toBe(false);
  await app.press("id:calendar-filters-open");
  await app.waitFor("id:calendar-sheet-done");
  expect(app.enabled("id:calendar-sheet-clear")).toBe(false);
  for (const word of ["How often", "Taxpayers", "Juana Santos", "Sample Trading Corp", "000-123-453", "Select all", "Clear"]) expect(app.texts()).toContain(word);

  // Unchecking a taxpayer drops her rows.
  await app.press(`id:calendar-tp-${JUANA}`);
  await app.waitFor("id:calendar-filters-count");
  expect(app.texts()).toContain("1");
  const corp = comingFor({ tps: new Set([SECOND_ROOT]), freq: "all" });
  await app.press("id:calendar-sheet-done");
  await app.waitFor("id:calendar-sheet-done", { state: "gone" });
  expect(listed(app.texts())).toEqual(lines(corp));
  expect(app.texts()).not.toContain("Juana Santos");

  // How often: Quarterly keeps the quarterly forms of what is left.
  await app.press("id:calendar-filters-open");
  await app.waitFor("id:calendar-sheet-done");
  await app.press("label:Quarterly");
  await app.waitFor("text:2");
  const quarterly = comingFor({ tps: new Set([SECOND_ROOT]), freq: "quarterly" });
  expect(quarterly.length).toBeGreaterThan(0);
  expect(quarterly.length).toBeLessThan(corp.length);
  expect(listed(app.texts())).toEqual(lines(quarterly));

  // Select all brings her back; Clear filters resets how often too.
  await app.press("id:calendar-tp-all");
  await app.press("id:calendar-sheet-clear");
  await app.waitFor("id:calendar-filters-count", { state: "gone" });
  expect(listed(app.texts())).toEqual(lines(comingFor(everyone)));
  expect(app.enabled("id:calendar-sheet-clear")).toBe(false);

  // None checked: the empty state in the list, with Clear filters there too.
  await app.press("id:calendar-tp-clear");
  await app.press("id:calendar-sheet-done");
  // The sheet slides away: until it is gone it still covers the list (a press on Clear filters reaches the sheet's search box).
  await app.waitFor("id:calendar-sheet-done", { state: "gone" });
  await app.waitFor("text:No deadlines match these filters.");
  await app.press("id:calendar-clear");
  await app.waitFor("text:Upcoming");
  expect(app.shows("id:calendar-clear")).toBe(false);
  expect(listed(app.texts())).toEqual(lines(comingFor(everyone)));
}, 120_000);

test("the sheet's search narrows the taxpayers", async () => {
  const app = await open(ENTRY, asking({ second: true }));
  await app.waitFor("id:calendar-narrow");
  await app.press("id:calendar-filters-open");
  await app.waitFor("id:calendar-sheet-done");
  await app.type("id:calendar-tp-search", "sample");
  await app.waitFor(`id:calendar-tp-${SECOND_ROOT}`);
  expect(app.shows(`id:calendar-tp-${JUANA}`)).toBe(false);
  await app.type("id:calendar-tp-search", "zzz");
  await app.waitFor("text:Nothing matches.");
}, 90_000);

test("a row opens its return; the empty states say what is missing", async () => {
  const app = await open(ENTRY, asking({}));
  await app.waitFor("id:calendar-narrow");
  const first = comingFor(everyone).find((d) => d.branch_code === "00000" && d.tin_root === JUANA)!;
  await app.press(`id:calendar-row-${rowKey(first)}`);
  await app.waitFor("id:form-page");
  expect(app.shows("id:calendar-narrow")).toBe(false);
  await app.press("id:nb-back");
  await app.waitFor("id:calendar-narrow");

  const fresh = await open(ENTRY, asking({ demo: false }));
  await fresh.waitFor("id:calendar-empty-row");
  expect(fresh.texts()).toContain("Deadlines show here once a taxpayer has forms for a tax year.");
  expect(fresh.shows("id:calendar-grid")).toBe(false);

  const bare = await open(ENTRY, asking({ years: false }));
  await bare.waitFor("id:calendar-narrow");
  await bare.waitFor("text:Nothing to do in the next 60 days.");
  expect(bare.shows("id:calendar-grid")).toBe(true);
}, 120_000);

test("the Calendar tab opens it, and leaving it takes the page away", async () => {
  const app = await open("../../ShellDemo.tsx", asking());
  await app.waitFor("id:tab-calendar");
  await app.press("id:tab-calendar");
  await app.waitFor("id:calendar-narrow");
  await app.press("id:tab-home");
  await app.waitFor("id:tab-home");
  expect(app.shows("id:calendar-narrow")).toBe(false);
}, 90_000);
