import type { SchemaField } from "./contract";
import type { DateParts } from "./schema";
import type { FormSession } from "./session";

function boxYear(s: FormSession, date: DateParts): number {
  const typed = String(s.box(date.year.k).value() || "");
  return /^\d{1,2}$/.test(typed) ? 2000 + Number(typed) : Number(typed || "");
}

/** The calendar's day, using the first when the day is blank. */
export function splitDay(s: FormSession, date: DateParts): string {
  const month = Number(String(s.box(date.month.k).value() || "").slice(0, 2));
  const year = boxYear(s, date);
  const day = date.day ? Number(s.box(date.day.k).value() || "") : 1;
  if (!(month >= 1 && month <= 12 && year >= 1900)) return "";
  return `${year}-${String(month).padStart(2, "0")}-${String(day >= 1 && day <= 31 ? day : 1).padStart(2, "0")}`;
}

/** The days permitted by a locked year and month, including calendar-year December. */
export function dateRange(s: FormSession, date: DateParts): { min?: string; max?: string } {
  const shut = (field: SchemaField) => field.c || s.box(field.k).locked();
  const year = boxYear(s, date);
  if (!shut(date.year) || !Number.isInteger(year) || year < 1900 || year > 9999) return {};
  const month = s.calendarYearEnd(date) ? 12 : shut(date.month) ? Number(String(s.box(date.month.k).value() || "").slice(0, 2)) : 0;
  if (month >= 1 && month <= 12) {
    const mm = String(month).padStart(2, "0");
    return { min: `${year}-${mm}-01`, max: `${year}-${mm}-${String(new Date(year, month, 0).getDate()).padStart(2, "0")}` };
  }
  return { min: `${year}-01-01`, max: `${year}-12-31` };
}

/** A blank calendar starts on the return period, not today. */
export function periodDay(s: FormSession): string {
  const target = s.target();
  const freq = s.schema?.freq || "";
  const period = target.period;
  const month = /month/i.test(freq) && period >= 1 && period <= 12 ? period
    : /quarter/i.test(freq) && period >= 1 && period <= 4 ? period * 3
    : 12;
  const year = target.year >= 1900 && target.year <= 2100 ? target.year : 2000;
  return `${year}-${String(month).padStart(2, "0")}-01`;
}
