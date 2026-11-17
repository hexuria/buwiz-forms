// Filing periods as the screens name them (periods.js). The canonical period of a return is the number the host
// takes: month 1-12 for monthly forms, quarter 1-4 for quarterly ones, 0 for annual returns and 1 for an as-needed
// return's first open-ended slot. Pure: the clock is a parameter, the system's by default.

import { MONTHS, MONTHS_LONG } from "./dates";

/** How often a form is filed. */
export type Frequency = "Monthly" | "Quarterly" | "Annual" | "As needed";

/** One line of the forms index: a form code and its frequency. */
export interface FormIndexEntry {
  c: string;
  freq: Frequency;
}

/** A period the start-a-return dialog offers: the value the host takes and the label the dialog shows. */
export interface PeriodOption {
  value: number;
  label: string;
}

/** The frequency of a form code in the forms index; Annual when the index does not list it. */
export function frequencyOf(index: readonly FormIndexEntry[], code: string): Frequency {
  const entry = index.find((f) => f.c === code);
  return entry ? entry.freq : "Annual";
}

/**
 * Periods the start-a-return dialog offers for a form in a tax year, newest first. The current year offers the months
 * or quarters up to now; a past year all of them; a later year only January or Q1.
 */
export function periodOptions(code: string, freq: Frequency, year: number, now: Date = new Date()): PeriodOption[] {
  const lastMonth = year < now.getFullYear() ? 12 : year > now.getFullYear() ? 1 : now.getMonth() + 1;
  if (freq === "Monthly") {
    const out: PeriodOption[] = [];
    for (let m = lastMonth; m >= 1; m--) out.push({ value: m, label: `${MONTHS_LONG[m - 1]} ${year}` });
    return out;
  }
  if (freq === "Quarterly") {
    const lastQuarter = Math.ceil(lastMonth / 3);
    const out: PeriodOption[] = [];
    for (let q = lastQuarter; q >= 1; q--) out.push({ value: q, label: `Q${q} ${year}` });
    return out;
  }
  if (freq === "As needed") return [{ value: 1, label: `${year}, first return` }];
  return [{ value: 0, label: `TY ${year}` }];
}

/** The human label for a period of a form with frequency freq. short gives Sep 2026. */
export function periodLabel(freq: Frequency, year: number, period: number, short = false): string {
  if (freq === "Monthly" && period >= 1 && period <= 12) {
    return `${short ? MONTHS[period - 1] : MONTHS_LONG[period - 1]} ${year}`;
  }
  if (freq === "Quarterly" && period >= 1 && period <= 4) return `Q${period} ${year}`;
  if (freq === "As needed") return `${year} #${period || 1}`;
  return `TY ${year}`;
}
