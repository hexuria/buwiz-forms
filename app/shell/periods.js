// Filing periods as the screens name them. The canonical period of a return
// is the number the host takes: month 1-12 for monthly forms, quarter 1-4
// for quarterly ones, 0 for annual returns and 1 for an as-needed return's
// first open-ended slot.

import { MONTHS, MONTHS_LONG } from "./store.js";

/** "Monthly" | "Quarterly" | "Annual" | "As needed" for a form code. */
export function frequencyOf(index, code) {
  const entry = index.find((f) => f.c === code);
  return entry ? entry.freq : "Annual";
}

/** Periods the start-a-return dialog offers for a form in a tax year, newest first. */
export function periodOptions(code, freq, year) {
  const now = new Date();
  const lastMonth = year < now.getFullYear() ? 12 : year > now.getFullYear() ? 1 : now.getMonth() + 1;
  if (freq === "Monthly") {
    const out = [];
    for (let m = lastMonth; m >= 1; m--) out.push({ value: m, label: `${MONTHS_LONG[m - 1]} ${year}` });
    return out;
  }
  if (freq === "Quarterly") {
    const lastQuarter = Math.ceil(lastMonth / 3);
    const out = [];
    for (let q = lastQuarter; q >= 1; q--) out.push({ value: q, label: `Q${q} ${year}` });
    return out;
  }
  if (freq === "As needed") return [{ value: 1, label: `${year}, first return` }];
  return [{ value: 0, label: `TY ${year}` }];
}

/** Human label for a period of a form with frequency `freq`. `short` gives "Sep 2026". */
export function periodLabel(freq, year, period, short) {
  if (freq === "Monthly" && period >= 1 && period <= 12) {
    return `${short ? MONTHS[period - 1] : MONTHS_LONG[period - 1]} ${year}`;
  }
  if (freq === "Quarterly" && period >= 1 && period <= 4) return `Q${period} ${year}`;
  if (freq === "As needed") return `${year} #${period || 1}`;
  return `TY ${year}`;
}
