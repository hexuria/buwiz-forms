// The Calendar's filters and what it has open (old CalendarFilters, made once at boot): they outlive the view, so leaving
// the Calendar and coming back finds the same taxpayers, chips, year, view and month. One owner: this module holds the
// signals, the screen reads the accessors and calls the setters.
import { batch, effect, signal, untracked } from "nsh";
import { isoDate } from "../../lib/dates";
import { app, todayIso } from "../../shell";
import { type Filters, filtersActive, type Freq } from "./rows";

/** The two layouts of the wide Calendar. */
export type CalendarView = "list" | "month";

const now = new Date();
const tps = signal<ReadonlySet<string> | null>(null);
const freq = signal<Freq>("all");
const month = signal(0);
const quarter = signal(0);
const year = signal(now.getFullYear());
const view = signal<CalendarView>("list");
const gridYear = signal(now.getFullYear());
const gridMonth = signal(now.getMonth());
const day = signal<string | null>(isoDate(now));
const picked = signal<string | null>(null);
/** The day the defaults above were taken on: when the clock moves past it, what followed "today" follows the new day. */
let shownToday = isoDate(now);

/**
 * The day changed (midnight, the year end, the app back in the foreground the next morning): the year filter that was this year, the
 * month shown that held today and today's mark move to the new day. What the person chose (another year, another month) stays.
 */
function followToday(iso: string): void {
  if (iso === shownToday) return;
  const wasYear = Number(shownToday.slice(0, 4));
  const wasMonth = Number(shownToday.slice(5, 7)) - 1;
  const was = shownToday;
  shownToday = iso;
  const y = Number(iso.slice(0, 4));
  const m = Number(iso.slice(5, 7)) - 1;
  batch(() => {
    if (year() === wasYear) year.set(y);
    const onToday = gridYear() === wasYear && gridMonth() === wasMonth;
    if (onToday) {
      gridYear.set(y);
      gridMonth.set(m);
    }
    if (day() === was) day.set(onToday || (gridYear() === y && gridMonth() === m) ? iso : null);
  });
}

effect(() => {
  const iso = todayIso();
  untracked(() => followToday(iso));
});

// The next session starts from the defaults (a lock, a sign-out or another taxpayer signing in): every filter, the layout and the month.
app.onSessionChange(() => {
  const today = new Date();
  shownToday = isoDate(today);
  batch(() => {
    tps.set(null);
    freq.set("all");
    month.set(0);
    quarter.set(0);
    year.set(today.getFullYear());
    view.set("list");
    gridYear.set(today.getFullYear());
    gridMonth.set(today.getMonth());
    day.set(isoDate(today));
    picked.set(null);
  });
});

/** The filters and the month shown. Read the accessors inside JSX or a closure; write through the setters. */
export const calendarState = {
  /** The five filters as one value (reads all five). */
  filters(): Filters {
    return { tps: tps(), freq: freq(), month: month(), quarter: quarter(), year: year() };
  },
  /** A filter differs from its start (the Clear filters button shows). */
  active: (): boolean => filtersActive({ tps: tps(), freq: freq(), month: month(), quarter: quarter(), year: year() }, new Date(todayIso() + "T12:00:00")),
  taxpayers: (): ReadonlySet<string> | null => tps(),
  setTaxpayers(next: ReadonlySet<string> | null): void {
    tps.set(next);
  },
  freq: (): Freq => freq(),
  /** A new choice drops the month and quarter chips, which belong to Monthly and Quarterly. */
  setFreq(next: Freq): void {
    batch(() => {
      freq.set(next);
      month.set(0);
      quarter.set(0);
    });
  },
  /** The month a return is due, 1-12; 0 for every month. */
  month: (): number => month(),
  setMonth(next: number): void {
    month.set(next);
  },
  /** The quarter a return covers, 1-4; 0 for every quarter. */
  quarter: (): number => quarter(),
  setQuarter(next: number): void {
    quarter.set(next);
  },
  /** The tax year the returns cover (the due year under a month chip). */
  year: (): number => year(),
  /** Moves the year by one step inside [min, max]. */
  stepYear(by: number, min: number, max: number): void {
    year.set(Math.min(max, Math.max(min, year() + by)));
  },
  view: (): CalendarView => view(),
  setView(next: CalendarView): void {
    view.set(next);
  },
  /** The year of the month the grid shows. */
  gridYear: (): number => gridYear(),
  /** The month the grid shows, 0-11. */
  gridMonth: (): number => gridMonth(),
  /**
   * The day the Month view lists, as an ISO date, or null when none is chosen. Moving to another month moves it to today
   * when that month holds today and clears it otherwise (showMonth), so the panel never lists a day the grid no longer shows.
   */
  day: (): string | null => day(),
  setDay(iso: string | null): void {
    day.set(iso);
  },
  /** The narrow layout's chosen day: the Upcoming list narrows to it; null lists what is coming. */
  picked: (): string | null => picked(),
  /** A press on a day: chooses it, and a press on the chosen day lets go of it. */
  togglePicked(iso: string): void {
    picked.set(picked() === iso ? null : iso);
  },
  clearPicked(): void {
    picked.set(null);
  },
  /** Shows a month (0-11) of a year in the grid; the chosen day follows (see day). */
  showMonth(y: number, m: number): void {
    const today = new Date();
    batch(() => {
      gridYear.set(y);
      gridMonth.set(m);
      day.set(today.getFullYear() === y && today.getMonth() === m ? isoDate(today) : null);
      picked.set(null);
    });
  },
  /** The month before (-1) or after (+1) the one shown. */
  stepMonth(by: number): void {
    const d = new Date(gridYear(), gridMonth() + by, 1);
    calendarState.showMonth(d.getFullYear(), d.getMonth());
  },
  /** The month that holds today. */
  showToday(): void {
    const today = new Date();
    calendarState.showMonth(today.getFullYear(), today.getMonth());
  },
  /** Back to every taxpayer, all frequencies, no chip and this year (the view and the month shown stay). */
  clear(): void {
    batch(() => {
      tps.set(null);
      freq.set("all");
      month.set(0);
      quarter.set(0);
      year.set(new Date().getFullYear());
    });
  },
};
