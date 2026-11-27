// Home's dashboard, pure data (round-two plan, claim 5): the numbers the four tiles show, the 30-day deadline strip, the
// list the picked tile or day narrows to, and each taxpayer's progress and next deadline. The element builders stay with
// the screen; this file computes what they draw. Every number comes from one named read, through DashboardReads:
//   Overdue, Due in 7 days   deadlines(today - 366 days, today + 7 days)
//   Drafts                   savedReturns()
//   Waiting to send          returns("filed"), status Queued
//   30-day strip             deadlines(today, today + 29 days)
//   Taxpayer progress        deadlines(January 1, today): filed of due (a dry run's deadline is due and not filed)
// A deadline is open while nobody queued or filed it, and while its return was only a dry run (it sent nothing).
//   Taxpayer next deadline   deadlines(today, today + 60 days): the first open one
//   Taxpayer rows            taxpayers()
// The clock is a parameter (the system's by default); the clock given is not changed.

import { MONTHS, MONTHS_LONG, addDays, isoDate } from "./dates";
import { countsAsFiled, isOpenDeadline } from "./statusWord";

/** A deadline as the host lists it (deadlines_between). */
export interface DashboardDeadline {
  tin: string;
  tin_root: string;
  branch_code: string;
  form_code: string;
  taxable_year: number;
  period: number;
  /** ISO date, YYYY-MM-DD. */
  due_date: string;
  status: string;
  /** The Submitted return was only a dry run (deadlines_between): the deadline stays open. */
  submitted_dry_run?: boolean | null;
  return_key?: number | string | null;
}

/** A return as the host lists it (list_returns). */
export interface DashboardReturn {
  tin_root: string;
  status: string;
  /** Its send state (list_returns): "test_sent" is a dry run, which is not filed. */
  send?: { stage: string } | null;
  /** A SQLite timestamp (UTC) or an ISO date. */
  updated_at?: string | null;
}

/** A taxpayer as the host lists it (list_taxpayers). */
export interface DashboardTaxpayer {
  tin_root: string;
  name?: string | null;
  tin_display?: string | null;
  archived?: boolean;
}

/** The reads the dashboard uses; the Store provides each one (memoised there). */
export interface DashboardReads {
  deadlines(from: string, to: string): DashboardDeadline[];
  savedReturns(): DashboardReturn[];
  returns(which: string): DashboardReturn[];
  taxpayers(): DashboardTaxpayer[];
}

/** One day of the 30-day strip. */
export interface StripDay {
  /** The day, as YYYY-MM-DD. */
  iso: string;
  date: Date;
  /** The open deadlines due that day. */
  rows: DashboardDeadline[];
}

/** One taxpayer's row: filed of due this year, and the next open deadline within 60 days. */
export interface DashboardTaxpayerRow {
  taxpayer: DashboardTaxpayer;
  due: number;
  done: number;
  next: DashboardDeadline | null;
}

/** Everything the dashboard shows. */
export interface DashboardData {
  today: string;
  in7: string;
  overdue: DashboardDeadline[];
  week: DashboardDeadline[];
  drafts: number;
  waiting: number;
  strip: StripDay[];
  upcoming: DashboardDeadline[];
  /** With nothing open on the strip: the first open deadline after it (within about 13 months), else null. */
  later: DashboardDeadline | null;
  anyDeadlines: boolean;
  taxpayers: DashboardTaxpayerRow[];
}

/** What the home view has picked: a tile (overdue or week), a day of the strip, or nothing. */
export type HomePick = "overdue" | "week" | "day";

/** The home view's pick: homePick, and homeDay (YYYY-MM-DD) for a day. */
export interface HomeView {
  homePick: HomePick | null;
  homeDay: string | null;
}

/** The part of the dashboard the picked list reads. */
export type PickedSource = Pick<DashboardData, "overdue" | "week" | "strip" | "upcoming" | "later" | "anyDeadlines">;

/** The list under the strip: its heading, its rows, and the line shown when it has none. */
export interface Picked {
  title: string;
  rows: DashboardDeadline[];
  empty: string;
}

/** The part of the dashboard the tiles read. */
export type TileSource = Pick<DashboardData, "in7" | "overdue" | "week" | "drafts" | "waiting">;

/** The tones a tile is drawn in (theme tokens). */
export type TileTone = "danger" | "warning" | "accent" | "fg2";

/** One tile: its key, its texts, its icon, its tone and its count. */
export interface TileSpec {
  key: "overdue" | "week" | "drafts" | "waiting";
  label: string;
  sub: string;
  tone: TileTone;
  icon: string;
  n: number;
}

const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const STRIP_DAYS = 30;
/** How far past the strip Home looks for the next deadline when the strip has none: a quarter's deadline and the annual return. */
const LATER_DAYS = 400;

/** Still to do: nobody has queued or filed it. A dry run sent nothing, so it is still to do. */
function isOpen(d: DashboardDeadline): boolean {
  return isOpenDeadline(d);
}

/** Everything the dashboard shows, from the reads listed at the top. */
export function loadDashboard(store: DashboardReads, at: Date = new Date()): DashboardData {
  const now = new Date(at.getTime());
  now.setHours(0, 0, 0, 0);
  const today = isoDate(now);
  const in7 = isoDate(addDays(now, 7));

  const near = store.deadlines(isoDate(addDays(now, -366)), in7).filter(isOpen);
  const overdue = near.filter((d) => d.due_date < today);
  const week = near.filter((d) => d.due_date >= today && d.due_date <= in7);

  const drafts = store.savedReturns().length;
  const filed = store.returns("filed");
  const waiting = filed.filter((r) => r.status === "Queued").length;

  const stripAll = store.deadlines(today, isoDate(addDays(now, STRIP_DAYS - 1)));
  const upcoming = stripAll.filter(isOpen);
  const strip: StripDay[] = [];
  for (let i = 0; i < STRIP_DAYS; i++) {
    const date = addDays(now, i);
    const iso = isoDate(date);
    strip.push({ iso, date, rows: upcoming.filter((d) => d.due_date === iso) });
  }

  // A taxpayer with forms and nothing in the next 30 days (a business that began late in the year) still has deadlines: name
  // the next one, so the empty line does not read as if there were no forms at all.
  const after = upcoming.length ? [] : store.deadlines(isoDate(addDays(now, STRIP_DAYS)), isoDate(addDays(now, LATER_DAYS)));
  const later = after.filter(isOpen).sort((a, b) => a.due_date.localeCompare(b.due_date))[0] ?? null;

  const sinceJan = store.deadlines(`${now.getFullYear()}-01-01`, today).filter((d) => d.due_date < today);
  const ahead = store.deadlines(today, isoDate(addDays(now, 60))).filter(isOpen);
  const taxpayers = store
    .taxpayers()
    .filter((t) => !t.archived)
    .map((t): DashboardTaxpayerRow => {
      const past = sinceJan.filter((d) => d.tin_root === t.tin_root);
      return {
        taxpayer: t,
        due: past.length,
        done: past.filter((d) => countsAsFiled(d)).length,
        next: ahead.find((d) => d.tin_root === t.tin_root) || null,
      };
    });

  return {
    today,
    in7,
    overdue,
    week,
    drafts,
    waiting,
    strip,
    upcoming,
    later,
    anyDeadlines: stripAll.length > 0 || near.length > 0 || after.length > 0,
    taxpayers,
  };
}

/** The four tiles: their keys, texts, icons, tones and counts, in the order they show. */
export function tileSpecs(data: TileSource): TileSpec[] {
  const [, m, d] = data.in7.split("-").map(Number);
  return [
    { key: "overdue", label: "Overdue", sub: "past the due date", tone: "danger", icon: "triangle-alert", n: data.overdue.length },
    { key: "week", label: "Due in 7 days", sub: `by ${MONTHS[m - 1]} ${d}`, tone: "warning", icon: "calendar", n: data.week.length },
    { key: "drafts", label: "Drafts", sub: "started, not queued", tone: "accent", icon: "pencil", n: data.drafts },
    { key: "waiting", label: "Waiting to send", sub: "queued to send", tone: "fg2", icon: "send", n: data.waiting },
  ];
}

/**
 * The deadlines under the strip for the tile or day picked (all open ones in the next 30 days when nothing is), with
 * its heading and its empty line.
 */
export function pickedDeadlines(view: HomeView, data: PickedSource): Picked {
  const pick = view.homePick;
  if (pick === "overdue") return { title: "Overdue", rows: data.overdue, empty: "Nothing is overdue." };
  if (pick === "week") return { title: "Due in the next 7 days", rows: data.week, empty: "Nothing is due in the next 7 days." };
  const day = pick === "day" ? data.strip.find((d) => d.iso === view.homeDay) : null;
  if (day) {
    const date = day.date;
    return {
      title: `${WEEKDAYS[date.getDay()]}, ${MONTHS_LONG[date.getMonth()]} ${date.getDate()}`,
      rows: day.rows,
      empty: "Nothing due this day.",
    };
  }
  return {
    title: "Open deadlines",
    rows: data.upcoming,
    empty: !data.anyDeadlines
      ? "Deadlines show here once a taxpayer has forms for a tax year."
      : data.later
        ? `Nothing is due in the next 30 days. The next is ${data.later.form_code} on ${longDate(data.later.due_date)}.`
        : "Nothing is due in the next 30 days.",
  };
}

/** "January 25, 2027" from YYYY-MM-DD. */
function longDate(iso: string): string {
  const [y, m, d] = iso.split("-").map(Number);
  return `${MONTHS_LONG[m - 1]} ${d}, ${y}`;
}

/** "1 of 4 filed this year" once something was due this year; "Nothing was due yet this year" before that. */
export function progressText(row: { due: number; done: number }): string {
  return row.due ? `${row.done} of ${row.due} filed this year` : "Nothing was due yet this year";
}

/** "Next: 1601C · Nov 10" for the next open deadline, or "Nothing due in 60 days". */
export function nextText(next: { form_code: string; due_date: string } | null): string {
  if (!next) return "Nothing due in 60 days";
  const [, m, d] = next.due_date.split("-").map(Number);
  return `Next: ${next.form_code} · ${MONTHS[m - 1]} ${d}`;
}
