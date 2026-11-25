// The Calendar's pure side (the old shell's calendar_page.js): which deadlines a filter keeps, how the List groups them, the
// month grid's days, and the tones a chip or a dot is drawn in. No UI and no host: the screen reads the deadlines through
// the store and hands them here. The clock is a parameter (the system's by default).
import type { DeadlineRow } from "../../bir";
import { addDays, daysUntil, isoDate, MONTHS, MONTHS_LONG } from "../../lib/dates";
import { DRY_RUN_WORD, isOpenDeadline, statusWord } from "../../lib/statusWord";
import type { ChipTone } from "../../ui";

/** How often: every frequency, or one of three. */
export type Freq = "all" | "yearly" | "quarterly" | "monthly";

/** The page's filters: the taxpayers (TIN roots; null is every taxpayer), how often, the month a return is due (1-12, 0 for every month), the quarter it covers (1-4, 0 for every quarter) and the tax year. */
export interface Filters {
  tps: ReadonlySet<string> | null;
  freq: Freq;
  month: number;
  quarter: number;
  year: number;
}

/** The choices of the How often control. */
export const FREQ_OPTIONS: { value: Freq; label: string }[] = [
  { value: "all", label: "All" },
  { value: "yearly", label: "Yearly" },
  { value: "quarterly", label: "Quarterly" },
  { value: "monthly", label: "Monthly" },
];

/** The frequency each choice keeps (the host's names). */
export const FREQ_OF: Record<Exclude<Freq, "all">, DeadlineRow["frequency"]> = { yearly: "Annual", quarterly: "Quarterly", monthly: "Monthly" };

/** The months a quarter covers, for the caption under the quarter chips. */
export const QUARTER_MONTHS = ["January to March", "April to June", "July to September", "October to December"];

/** Weekday names, Sunday first (the grid starts on Sunday). */
export const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/** "1 deadline", "2 deadlines". */
export const plural = (n: number, word: string): string => `${n} ${word}${n === 1 ? "" : "s"}`;

/** The taxpayer and how-often filters, which every view applies. */
export function byWhoAndHowOften(f: Pick<Filters, "tps" | "freq">, rows: readonly DeadlineRow[]): DeadlineRow[] {
  let r = rows.slice();
  if (f.tps) r = r.filter((d) => f.tps!.has(d.tin_root));
  if (f.freq !== "all") r = r.filter((d) => d.frequency === FREQ_OF[f.freq as Exclude<Freq, "all">]);
  return r;
}

/**
 * The List view's rows: a month chip keeps the returns due in that month of the year; otherwise the year is the tax year
 * the returns cover, and a quarter chip keeps one covered quarter.
 */
export function calendarRows(f: Filters, rows: readonly DeadlineRow[]): DeadlineRow[] {
  const r = byWhoAndHowOften(f, rows);
  if (f.freq === "monthly" && f.month) {
    const prefix = `${f.year}-${String(f.month).padStart(2, "0")}-`;
    return r.filter((d) => d.due_date.startsWith(prefix));
  }
  let out = r.filter((d) => d.taxable_year === f.year);
  if (f.freq === "quarterly" && f.quarter) out = out.filter((d) => d.period === f.quarter);
  return out;
}

/** True when any filter differs from its start (every taxpayer, all frequencies, no chip, this year). */
export function filtersActive(f: Filters, now: Date = new Date()): boolean {
  return f.tps !== null || f.freq !== "all" || f.month !== 0 || f.quarter !== 0 || f.year !== now.getFullYear();
}

/** Date order (ISO dates compare as text). */
export const byDue = (a: DeadlineRow, b: DeadlineRow): number => (a.due_date < b.due_date ? -1 : a.due_date > b.due_date ? 1 : 0);

/** The line under the filter bar: "12 deadlines · 1 taxpayer · 2026 returns". */
export function countText(f: Filters, rows: readonly DeadlineRow[]): string {
  const who = new Set(rows.map((d) => d.tin_root)).size;
  let scope = `${f.year} returns`;
  if (f.freq === "monthly" && f.month) scope = `due in ${MONTHS_LONG[f.month - 1]} ${f.year}`;
  else if (f.freq === "quarterly" && f.quarter) scope = `returns for Q${f.quarter} ${f.year}`;
  return `${plural(rows.length, "deadline")} · ${plural(who, "taxpayer")} · ${scope}`;
}

/** One heading of the List and its rows. */
export interface Group {
  /** "November 2026" (by due month), or "Q1 2026" (by covered quarter, under Quarterly). */
  label: string;
  /** The heading: the label, or "Q1 2026 returns" under Quarterly. */
  title: string;
  /** Under Quarterly: "due Apr 15, Jul 15". */
  caption: string | null;
  rows: DeadlineRow[];
}

/** The List's groups: by due month, or by covered quarter under Quarterly; rows in date order. */
export function groupRows(f: Pick<Filters, "freq">, rows: readonly DeadlineRow[]): Group[] {
  const sorted = rows.slice().sort(byDue);
  const groups = new Map<string, DeadlineRow[]>();
  const quarterly = f.freq === "quarterly";
  if (quarterly) sorted.sort((a, b) => a.taxable_year - b.taxable_year || a.period - b.period || byDue(a, b));
  for (const d of sorted) {
    const key = quarterly ? `Q${d.period} ${d.taxable_year}` : `${MONTHS_LONG[Number(d.due_date.split("-")[1]) - 1]} ${d.due_date.split("-")[0]}`;
    const list = groups.get(key);
    if (list) list.push(d);
    else groups.set(key, [d]);
  }
  return [...groups].map(([label, list]) => {
    if (!quarterly) return { label, title: label, caption: null, rows: list };
    const due = [...new Set(list.map((d) => d.due_date))].map((iso) => {
      const [, m, day] = iso.split("-").map(Number);
      return `${MONTHS[m - 1]} ${day}`;
    });
    return { label, title: `${label} returns`, caption: `due ${due.join(", ")}`, rows: list };
  });
}

// ---------------------------------------------------------------- status and tone

/** The status a row's chip says. */
export type RowKind = "draft" | "queued" | "dry_run" | "filed" | "not_started";

/** A deadline's status as the chips name it. A chip passes the shown word (`statusWord`): a simulated send's "Dry run" is its own chip. */
export function kindOf(status: string): RowKind {
  if (status === DRY_RUN_WORD) return "dry_run";
  if (status === "Draft") return "draft";
  if (status === "Queued") return "queued";
  if (status === "Submitted" || status === "Confirmed" || status === "Paid") return "filed";
  return "not_started";
}

/** Still to do: nobody has queued or filed it, or its return was only a dry run (`isOpenDeadline`: it sent nothing). */
export const isOpenStatus = (d: Pick<DeadlineRow, "status"> & { submitted_dry_run?: boolean | null }): boolean => isOpenDeadline(d);

/**
 * A chip's tone: overdue (danger), due within 14 days (warning), filed (success), in progress (accent), else neutral. A dry run
 * is open, so it is late or soon as a Draft is, and in progress (accent) before that: never filed.
 */
export function toneOf(d: Pick<DeadlineRow, "status" | "due_date"> & { submitted_dry_run?: boolean | null }, now: Date = new Date()): ChipTone {
  const n = daysUntil(d.due_date, now);
  if (isOpenStatus(d) && n < 0) return "danger";
  if (isOpenStatus(d) && n <= 14) return "warning";
  const kind = kindOf(statusWord(d));
  if (kind === "filed") return "success";
  if (kind === "draft" || kind === "queued" || kind === "dry_run") return "accent";
  return "neutral";
}

/** The tone of a day's dot: the most urgent of its deadlines (late, soon, in progress, to do, filed). */
export function dayTone(items: readonly (Pick<DeadlineRow, "status" | "due_date"> & { submitted_dry_run?: boolean | null })[], now: Date = new Date()): ChipTone {
  const order: ChipTone[] = ["danger", "warning", "accent", "neutral", "success"];
  let best = order.length;
  for (const d of items) best = Math.min(best, order.indexOf(toneOf(d, now)));
  return order[best] ?? "neutral";
}

/** Who a row belongs to: the taxpayer, plus the branch when it has several. */
export const whoLine = (d: Pick<DeadlineRow, "branch_count" | "taxpayer_name" | "branch_name">): string =>
  d.branch_count > 1 ? `${d.taxpayer_name} · ${d.branch_name}` : d.taxpayer_name;

/**
 * A row's key. An as-needed row is keyed by its saved return as well: two removals in one month are one return for the
 * 2200 family, but other as-needed forms hold several returns that share a year and period.
 */
export const rowKey = (d: Pick<DeadlineRow, "tin" | "form_code" | "taxable_year" | "period" | "frequency" | "draft_id">): string =>
  `${d.tin}-${d.form_code}-${d.taxable_year}-${d.period}${d.frequency === "As needed" ? `-${d.draft_id}` : ""}`;

// ---------------------------------------------------------------- month grid

/** The Sunday on or before the first of the month (month 0-11): the grid's first cell. */
export function monthGridStart(year: number, month: number): Date {
  const first = new Date(year, month, 1);
  return addDays(first, -first.getDay());
}

/** One cell of the grid. */
export interface Cell {
  iso: string;
  /** The day of the month. */
  day: number;
  /** Whether the day belongs to the month shown (the other cells are dimmed). */
  inMonth: boolean;
  isToday: boolean;
  /** 0-6 (Sunday first) and 0-5. */
  col: number;
  row: number;
  /** The deadlines due that day, in date order. */
  items: DeadlineRow[];
  /** "November 15, 2 deadlines" for screen readers. */
  label: string;
}

/** The six weeks the grid shows for a month (0-11), each day with its deadlines: six weeks always, so the grid keeps its height. */
export function gridCells(year: number, month: number, rows: readonly DeadlineRow[], now: Date = new Date()): Cell[] {
  const start = monthGridStart(year, month);
  const byDay = new Map<string, DeadlineRow[]>();
  for (const d of rows.slice().sort(byDue)) {
    const list = byDay.get(d.due_date);
    if (list) list.push(d);
    else byDay.set(d.due_date, [d]);
  }
  const today = isoDate(now);
  const cells: Cell[] = [];
  for (let i = 0; i < 42; i++) {
    const date = addDays(start, i);
    const iso = isoDate(date);
    const items = byDay.get(iso) ?? [];
    cells.push({
      iso,
      day: date.getDate(),
      inMonth: date.getMonth() === month,
      isToday: iso === today,
      col: i % 7,
      row: Math.floor(i / 7),
      items,
      label: `${MONTHS_LONG[date.getMonth()]} ${date.getDate()}, ${plural(items.length, "deadline")}`,
    });
  }
  return cells;
}

/** The first and last day the grid shows for a month, as ISO dates (one read of the host covers both). */
export function gridRange(year: number, month: number): [string, string] {
  const start = monthGridStart(year, month);
  return [isoDate(start), isoDate(addDays(start, 41))];
}

/** "Sunday, November 15" for an ISO date. */
export function longDay(iso: string): string {
  const [y, m, d] = iso.split("-").map(Number);
  const date = new Date(y, m - 1, d);
  return `${WEEKDAYS[date.getDay()]}, ${MONTHS_LONG[date.getMonth()]} ${date.getDate()}`;
}

/** How many of the rows fall in the month shown (month 0-11) of the year. */
export function inMonthCount(year: number, month: number, rows: readonly DeadlineRow[]): number {
  const prefix = `${year}-${String(month + 1).padStart(2, "0")}-`;
  return rows.filter((d) => d.due_date.startsWith(prefix)).length;
}

// ---------------------------------------------------------------- narrow: the Upcoming list

/** How far ahead the narrow layout's Upcoming list looks. */
export const UPCOMING_DAYS = 60;
/** How far back it looks for late returns (as Home's Overdue does). */
export const LATE_DAYS = 366;

/** The window the Upcoming list reads: [from, to] as ISO dates. */
export function upcomingRange(now: Date = new Date()): [string, string] {
  const today = new Date(now.getTime());
  today.setHours(0, 0, 0, 0);
  return [isoDate(addDays(today, -LATE_DAYS)), isoDate(addDays(today, UPCOMING_DAYS))];
}

/** What is still to do: the late ones first (oldest due date first), then what falls due from today on. */
export function upcomingRows(rows: readonly DeadlineRow[], now: Date = new Date()): DeadlineRow[] {
  const today = isoDate(now);
  const open = rows.filter(isOpenStatus).sort(byDue);
  return [...open.filter((d) => d.due_date < today), ...open.filter((d) => d.due_date >= today)];
}
