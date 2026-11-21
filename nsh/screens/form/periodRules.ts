// The month+year pair rules (PR 3, P3a): the typed shapes a period box takes, its MM/YYYY text, and
// the year forms the underlying boxes store (a four-digit year, a 20YY box's two digits, a select's
// listed code). Pure: no signals, no door — the session applies these.
import { MONTHS_LONG } from "../../lib/dates";

/** What the filer typed, read as a month and a year: m is 1-12-or-not (the session judges), y its digits as typed. */
export interface ParsedPeriod {
  m: number;
  y: number;
  /** How many digits the year was typed with (2 for "12/23"). */
  yDigits: number;
}

/**
 * A period typed as text: 12/2023, 12-2023, 12 2023, 122023 or 1/2023 all mean December 2023 (or
 * January). A two-digit year reads too (12/23) — whether it is allowed is the box's call (a 20YY box
 * takes it, a four-digit one refuses it). Null when the text is no period at all.
 */
export function parsePeriod(text: string | null | undefined): ParsedPeriod | null {
  const v = String(text || "").trim();
  if (!v) return null;
  const compact = /^(\d{1,2})(\d{4})$/.exec(v);
  const split = /^(\d{1,2})\s*[\/\-. ]\s*(\d{2,4})$/.exec(v);
  let m = 0;
  let y = 0;
  let yDigits = 0;
  if (compact) {
    m = Number(compact[1]);
    y = Number(compact[2]);
    yDigits = 4;
  } else if (split) {
    m = Number(split[1]);
    yDigits = split[2].length;
    y = Number(split[2]);
  } else {
    return null;
  }
  if (m < 1 || m > 31 || y < 1) return null;
  return { m, y, yDigits };
}

/** The sentence a period box shows for text that is no MM/YYYY (the session prefixes the item). */
export const PERIOD_ERROR = "type the month and year as MM/YYYY, for example 12/2023.";

/** A typed two-digit year where the box keeps four digits. */
export const PERIOD_TWO_DIGIT = "two-digit period year";

/** The period's MM/YYYY text for the draft's month and year values ("" while either is blank or zero). */
export function periodText(month: string, year: string): string {
  const m = String(month ?? "").trim();
  const y = String(year ?? "").trim();
  if (!m || !y || /^0+$/.test(m) || /^0+$/.test(y)) return "";
  const mm = Number(m);
  if (!(mm >= 1 && mm <= 12)) return "";
  const yy = /^\d{1,2}$/.test(y) ? 2000 + Number(y) : Number(y);
  if (!Number.isInteger(yy) || yy < 1900 || yy > 9999) return "";
  return `${String(mm).padStart(2, "0")}/${yy}`;
}

/** The period's long text, "December 2023", for a box that is not being typed in. */
export function periodLong(month: string, year: string): string {
  const t = periodText(month, year);
  if (!t) return "";
  const [mm, yy] = t.split("/").map(Number);
  return `${MONTHS_LONG[mm - 1]} ${yy}`;
}

/** The four-digit year a stored year box value means (a 20YY box's "23" is 2023). */
export function periodYear4(year: string): number {
  const y = String(year ?? "").trim();
  return /^\d{1,2}$/.test(y) ? 2000 + Number(y) : Number(y);
}
