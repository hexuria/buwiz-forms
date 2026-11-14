// Date helpers, in one file. From store.js: isoDate, addDays, daysUntil, shortDate, MONTHS, MONTHS_LONG.
// From form_page.js: parseTypedDate, formatMDY, formatLongDate, shownDate, oneDateText, DATE_ERROR, and its
// shortDate, named shortIsoDate here because store.js's shortDate is a different function.
// Local time throughout, as the old code was. Pure: no host and no Intl (QuickJS has none).

/** A calendar day: the year, the month (1-12) and the day of the month. */
export interface YMD {
  y: number;
  m: number;
  d: number;
}

/** A date split into the month, day and year boxes that hold it (oneDateText). day is null when the form has no day box. */
export interface OneDateParts {
  month: { k: string };
  day: { k: string } | null;
  year: { k: string };
}

export const MONTHS: readonly string[] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

export const MONTHS_LONG: readonly string[] = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

/** What a date box says when its text is not a date. */
export const DATE_ERROR = "Enter a valid date as MM/DD/YYYY (e.g. 12/01/2020 or 12012020)";

/** ISO date (YYYY-MM-DD) in local time. */
export function isoDate(date: Date): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

/** The date days after (or, negative, before) the given one. The given date is not changed. */
export function addDays(date: Date, days: number): Date {
  const next = new Date(date.getTime());
  next.setDate(next.getDate() + days);
  return next;
}

/** Whole days from today (local midnight) to an ISO date. now is the clock; the system's by default. */
export function daysUntil(iso: string, now: Date = new Date()): number {
  const today = new Date(now.getTime());
  today.setHours(0, 0, 0, 0);
  const [y, m, d] = iso.split("-").map(Number);
  return Math.round((new Date(y, m - 1, d).getTime() - today.getTime()) / 86400000);
}

/** "Oct 9, 2026" from an ISO date or a SQLite timestamp. */
export function shortDate(text: string | null | undefined): string {
  if (!text) return "";
  // SQLite timestamps ("2026-10-14 22:10:03") are UTC; show the local day.
  const ts = /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2})/.exec(String(text));
  if (ts) {
    const local = new Date(Date.UTC(+ts[1], +ts[2] - 1, +ts[3], +ts[4], +ts[5]));
    return `${MONTHS[local.getMonth()]} ${local.getDate()}, ${local.getFullYear()}`;
  }
  const [datePart] = String(text).split(/[ T]/);
  const [y, m, d] = datePart.split("-").map(Number);
  if (!y || !m || !d) return String(text);
  return `${MONTHS[m - 1]} ${d}, ${y}`;
}

/** A timestamp starting with an ISO date reads as Oct 1, 2026; anything else as given. (form_page.js's shortDate.) */
export function shortIsoDate(text: string | null | undefined): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(String(text || ""));
  if (!m) return String(text || "");
  return `${MONTHS[Number(m[2]) - 1]} ${Number(m[3])}, ${m[1]}`;
}

/**
 * A date the filer typed, read the way the old tax-profile date input read it: 12012020, 12/01/2020, 12-01-2020,
 * 2020-12-01 or Dec 01 2020 all mean December 1, 2020. Null when it is not a real day.
 */
export function parseTypedDate(text: string | null | undefined): YMD | null {
  const v = String(text || "").trim();
  if (!v) return null;
  const MON = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
  const compact = /^(\d{2})(\d{2})(\d{4})$/.exec(v);
  const iso = /^(\d{4})-(\d{1,2})-(\d{1,2})$/.exec(v);
  const slashed = /^(\d{1,2})[/-](\d{1,2})[/-](\d{4})$/.exec(v);
  const named = /^([a-z]{3})[a-z]*\.?\s+(\d{1,2}),?\s+(\d{4})$/i.exec(v);
  let y = 0;
  let m = 0;
  let d = 0;
  if (compact) {
    [m, d, y] = [Number(compact[1]), Number(compact[2]), Number(compact[3])];
  } else if (iso) {
    [y, m, d] = [Number(iso[1]), Number(iso[2]), Number(iso[3])];
  } else if (slashed) {
    [m, d, y] = [Number(slashed[1]), Number(slashed[2]), Number(slashed[3])];
  } else if (named) {
    m = MON.indexOf(named[1].toLowerCase()) + 1;
    [d, y] = [Number(named[2]), Number(named[3])];
  } else {
    return null;
  }
  if (y < 1900 || y > 2100 || m < 1 || m > 12 || d < 1) return null;
  if (d > new Date(Date.UTC(y, m, 0)).getUTCDate()) return null;
  return { y, m, d };
}

/** The form's MM/DD/YYYY text for a parsed date. */
export function formatMDY({ y, m, d }: YMD): string {
  return `${String(m).padStart(2, "0")}/${String(d).padStart(2, "0")}/${y}`;
}

/** How a date box shows a parsed date: Dec 10, 2020 (rule 4). */
export function formatLongDate({ y, m, d }: YMD): string {
  return `${MONTHS[m - 1]} ${d}, ${y}`;
}

/**
 * What a date box shows for the draft's text: the date as Dec 10, 2020 when it reads as one, else the text as it is
 * (a blank, a part-typed date). Display only: the draft keeps the form's own format.
 */
export function shownDate(text: string | null | undefined): string {
  const date = parseTypedDate(text);
  return date ? formatLongDate(date) : String(text ?? "");
}

/** The three boxes' values as the MM/DD/YYYY text a date box reads ("" when all are blank). */
export function oneDateText(values: Readonly<Record<string, unknown>>, date: OneDateParts): string {
  const parts = [date.month, date.day, date.year].map((f) => String((f && values[f.k]) ?? "").trim());
  return parts.every((v) => !v) ? "" : parts.join("/");
}
