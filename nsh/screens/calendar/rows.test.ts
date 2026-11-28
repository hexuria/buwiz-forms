// The Calendar's pure side: what each filter keeps, how the List groups, the grid's days and the tones of a chip and a dot.
import { expect, test } from "nsh/test";
import type { DeadlineRow } from "../../bir";
import { statusWord } from "../../lib/statusWord";
import {
  byWhoAndHowOften, calendarRows, countText, dayTone, filtersActive, type Filters, gridCells, groupRows, inMonthCount, kindOf, longDay,
  monthGridStart, rowKey, toneOf, upcomingRange, upcomingRows, whoLine,
} from "./rows";

let next = 1;
function row(over: Partial<DeadlineRow> & Pick<DeadlineRow, "form_code" | "due_date">): DeadlineRow {
  return {
    tin: "00012345300000",
    tin_root: "000123453",
    branch_code: "00000",
    branch_name: "Head office",
    taxpayer_name: "Juana Santos",
    branch_count: 1,
    form_name: "A form",
    frequency: "Monthly",
    taxable_year: 2026,
    period: 1,
    period_label: "January 2026",
    status: "Not started",
    submitted_dry_run: false,
    draft_id: null,
    return_key: next++,
    ...over,
  };
}

const base: Filters = { tps: null, freq: "all", month: 0, quarter: 0, year: 2026 };
// A December 1601C due January 10, the year after; two quarterlies; a yearly; a second taxpayer's monthly.
const rows = [
  row({ form_code: "1601C", due_date: "2027-01-10", period: 12, period_label: "December 2026" }),
  row({ form_code: "1601C", due_date: "2026-02-10", period: 1 }),
  row({ form_code: "2551Q", due_date: "2026-04-20", frequency: "Quarterly", period: 1, period_label: "Q1 2026" }),
  row({ form_code: "2551Q", due_date: "2026-07-20", frequency: "Quarterly", period: 2, period_label: "Q2 2026" }),
  row({ form_code: "1701A", due_date: "2027-04-15", frequency: "Annual", period: 0, period_label: "TY 2026" }),
  row({ form_code: "1601C", due_date: "2026-02-10", period: 1, tin: "00011122500000", tin_root: "000111225", taxpayer_name: "Sample Trading Corp" }),
];

test("taxpayer and how-often filters apply to every view", () => {
  expect(byWhoAndHowOften(base, rows)).toHaveLength(6);
  expect(byWhoAndHowOften({ ...base, tps: new Set(["000111225"]) }, rows)).toHaveLength(1);
  expect(byWhoAndHowOften({ ...base, tps: new Set() }, rows)).toHaveLength(0);
  expect(byWhoAndHowOften({ ...base, freq: "quarterly" }, rows).map((d) => d.form_code)).toEqual(["2551Q", "2551Q"]);
  expect(byWhoAndHowOften({ ...base, freq: "yearly" }, rows).map((d) => d.form_code)).toEqual(["1701A"]);
  expect(byWhoAndHowOften({ ...base, freq: "monthly" }, rows)).toHaveLength(3);
});

test("the year is the tax year the returns cover; a quarter chip keeps one covered quarter", () => {
  // December 2026's 1601C is due in 2027 but covers 2026: it is in 2026's list and not in 2027's.
  expect(calendarRows(base, rows).map((d) => d.due_date)).toContain("2027-01-10");
  expect(calendarRows({ ...base, year: 2027 }, rows)).toHaveLength(0);
  expect(calendarRows({ ...base, freq: "quarterly", quarter: 2 }, rows).map((d) => d.period_label)).toEqual(["Q2 2026"]);
  expect(calendarRows({ ...base, freq: "quarterly", quarter: 0 }, rows)).toHaveLength(2);
});

test("a month chip keeps what is due in that month of the year (January shows December's 1601C)", () => {
  const jan = calendarRows({ ...base, year: 2027, freq: "monthly", month: 1 }, rows);
  expect(jan.map((d) => d.period_label)).toEqual(["December 2026"]);
  const feb = calendarRows({ ...base, freq: "monthly", month: 2 }, rows);
  expect(feb).toHaveLength(2);
  // The month chip only counts under Monthly: with another frequency it is ignored.
  expect(calendarRows({ ...base, freq: "all", month: 2 }, rows)).toHaveLength(6);
});

test("filters are active once any differs from its start", () => {
  const now = new Date(2026, 5, 1);
  expect(filtersActive(base, now)).toBe(false);
  expect(filtersActive({ ...base, tps: new Set(["x"]) }, now)).toBe(true);
  expect(filtersActive({ ...base, freq: "yearly" }, now)).toBe(true);
  expect(filtersActive({ ...base, month: 3 }, now)).toBe(true);
  expect(filtersActive({ ...base, quarter: 3 }, now)).toBe(true);
  expect(filtersActive({ ...base, year: 2025 }, now)).toBe(true);
});

test("the count line says deadlines, taxpayers and scope", () => {
  expect(countText(base, rows)).toBe("6 deadlines · 2 taxpayers · 2026 returns");
  expect(countText(base, [rows[0]])).toBe("1 deadline · 1 taxpayer · 2026 returns");
  expect(countText({ ...base, freq: "monthly", month: 2 }, rows)).toContain("due in February 2026");
  expect(countText({ ...base, freq: "quarterly", quarter: 3 }, rows)).toContain("returns for Q3 2026");
});

test("the List groups by due month, in date order; Quarterly groups by covered quarter with the due dates", () => {
  const byMonth = groupRows({ freq: "all" }, rows);
  expect(byMonth.map((g) => g.label)).toEqual(["February 2026", "April 2026", "July 2026", "January 2027", "April 2027"]);
  expect(byMonth[0].rows).toHaveLength(2);
  expect(byMonth[0].caption).toBeNull();
  const quarterly = groupRows({ freq: "quarterly" }, byWhoAndHowOften({ ...base, freq: "quarterly" }, rows));
  expect(quarterly.map((g) => g.title)).toEqual(["Q1 2026 returns", "Q2 2026 returns"]);
  expect(quarterly[0].caption).toBe("due Apr 20");
});

test("a row's status, who and key", () => {
  expect(kindOf("Draft")).toBe("draft");
  expect(kindOf("Queued")).toBe("queued");
  expect(kindOf("Submitted")).toBe("filed");
  expect(kindOf("Confirmed")).toBe("filed");
  expect(kindOf("Paid")).toBe("filed");
  expect(kindOf("Not started")).toBe("not_started");
  // A chip passes the shown word: a simulated send is "Test send: not filed", a real one keeps Submitted's "filed".
  expect(kindOf(statusWord(row({ form_code: "0619E", due_date: "2026-02-10", status: "Submitted", submitted_dry_run: true })))).toBe("dry_run");
  expect(kindOf(statusWord(row({ form_code: "0619E", due_date: "2026-02-10", status: "Submitted" })))).toBe("filed");
  expect(whoLine(rows[0])).toBe("Juana Santos");
  expect(whoLine({ ...rows[0], branch_count: 2 })).toBe("Juana Santos · Head office");
  expect(rowKey(rows[1])).toBe("00012345300000-1601C-2026-1");
  expect(rowKey({ ...rows[1], frequency: "As needed", draft_id: 7 })).toBe("00012345300000-1601C-2026-1-7");
});

test("a chip's tone: overdue, due within 14 days, filed, in progress, else neutral", () => {
  const now = new Date(2026, 5, 10);
  expect(toneOf({ status: "Not started", due_date: "2026-06-09" }, now)).toBe("danger");
  expect(toneOf({ status: "Draft", due_date: "2026-06-01" }, now)).toBe("danger");
  expect(toneOf({ status: "Not started", due_date: "2026-06-10" }, now)).toBe("warning");
  expect(toneOf({ status: "Draft", due_date: "2026-06-24" }, now)).toBe("warning");
  expect(toneOf({ status: "Not started", due_date: "2026-06-25" }, now)).toBe("neutral");
  expect(toneOf({ status: "Draft", due_date: "2026-07-25" }, now)).toBe("accent");
  expect(toneOf({ status: "Queued", due_date: "2026-06-01" }, now)).toBe("accent");
  expect(toneOf({ status: "Submitted", due_date: "2026-06-01" }, now)).toBe("success");
  expect(toneOf({ status: "Paid", due_date: "2026-08-01" }, now)).toBe("success");
  // A real send (no dry-run fact) is filed; a dry run sent nothing: late, soon or in progress as a Draft is, never success.
  expect(toneOf({ status: "Submitted", submitted_dry_run: false, due_date: "2026-06-01" }, now)).toBe("success");
  expect(toneOf({ status: "Submitted", submitted_dry_run: true, due_date: "2026-06-01" }, now)).toBe("danger");
  expect(toneOf({ status: "Submitted", submitted_dry_run: true, due_date: "2026-06-24" }, now)).toBe("warning");
  expect(toneOf({ status: "Submitted", submitted_dry_run: true, due_date: "2026-07-25" }, now)).toBe("accent");
});

test("a day's dot takes the most urgent tone (late first, filed last)", () => {
  const now = new Date(2026, 5, 10);
  const late = { status: "Not started", due_date: "2026-06-01" };
  const soon = { status: "Not started", due_date: "2026-06-12" };
  const filed = { status: "Submitted", due_date: "2026-06-01" };
  expect(dayTone([filed, late], now)).toBe("danger");
  expect(dayTone([filed, soon], now)).toBe("warning");
  expect(dayTone([filed], now)).toBe("success");
  expect(dayTone([filed, { status: "Not started", due_date: "2026-09-01" }], now)).toBe("neutral");
  expect(dayTone([], now)).toBe("neutral");
  // A dry run is not filed: its day reads late (or soon) as a Draft's would.
  expect(dayTone([{ status: "Submitted", submitted_dry_run: true, due_date: "2026-06-01" }], now)).toBe("danger");
  expect(dayTone([{ status: "Submitted", submitted_dry_run: true, due_date: "2026-06-12" }], now)).toBe("warning");
});

test("the grid is six weeks from the Sunday on or before the first", () => {
  // November 2026 starts on a Sunday; December 2026 on a Tuesday (the grid starts November 29).
  expect(monthGridStart(2026, 10).getDate()).toBe(1);
  expect(monthGridStart(2026, 10).getDay()).toBe(0);
  const dec = monthGridStart(2026, 11);
  expect([dec.getMonth(), dec.getDate()]).toEqual([10, 29]);
  const cells = gridCells(2026, 1, rows, new Date(2026, 1, 10));
  expect(cells).toHaveLength(42);
  expect(cells[0].iso).toBe("2026-02-01");
  expect(cells[41].iso).toBe("2026-03-14");
  expect(cells.filter((c) => c.inMonth)).toHaveLength(28);
  const tenth = cells.find((c) => c.iso === "2026-02-10")!;
  expect(tenth.items).toHaveLength(2);
  expect(tenth.isToday).toBe(true);
  expect(tenth.label).toBe("February 10, 2 deadlines");
  expect(cells.find((c) => c.iso === "2026-02-11")!.label).toBe("February 11, 0 deadlines");
  expect(cells.filter((c) => c.isToday)).toHaveLength(1);
  expect([cells[8].col, cells[8].row]).toEqual([1, 1]);
  expect(inMonthCount(2026, 1, rows)).toBe(2);
  expect(inMonthCount(2026, 3, rows)).toBe(1);
  expect(longDay("2026-02-10")).toBe("Tuesday, February 10");
});

test("Upcoming: what is still to do, the late ones first, then from today on", () => {
  const now = new Date(2026, 5, 10);
  const list = [
    row({ form_code: "A", due_date: "2026-06-20" }),
    row({ form_code: "B", due_date: "2026-05-10" }),
    row({ form_code: "C", due_date: "2026-06-10", status: "Draft" }),
    row({ form_code: "D", due_date: "2026-04-10", status: "Submitted" }),
    row({ form_code: "E", due_date: "2026-06-01" }),
    row({ form_code: "F", due_date: "2026-06-15", status: "Queued" }),
    // A dry run sent nothing: still to do. A real send is not.
    row({ form_code: "G", due_date: "2026-05-20", status: "Submitted", submitted_dry_run: true }),
    row({ form_code: "H", due_date: "2026-05-21", status: "Submitted", submitted_dry_run: false }),
  ];
  expect(upcomingRows(list, now).map((d) => d.form_code)).toEqual(["B", "G", "E", "C", "A"]);
  expect(upcomingRange(now)).toEqual(["2025-06-09", "2026-08-09"]);
});
