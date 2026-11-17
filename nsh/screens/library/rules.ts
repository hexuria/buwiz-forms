// The Form library's rules, ported from app/shell/library.js: the search text, the frequency choices, and which forms a
// scope, a frequency and a search show. Pure: Library.tsx reads the index and the year's forms, then calls these.
import type { ScreenFormIndexEntry } from "../../bir";

/** The frequency choices, in the old order. */
export const FREQS = ["All", "Monthly", "Quarterly", "Annual", "As needed"] as const;

/** One of FREQS. */
export type FrequencyFilter = (typeof FREQS)[number];

/** "owned": the forms the selected taxpayer files in the selected year; "all": every form. */
export type FormScope = "owned" | "all";

/** What a search reads of a form: its code with and without dashes, and its title; "VAT" also finds the Value-Added forms. */
export function searchText(f: Pick<ScreenFormIndexEntry, "c" | "t">): string {
  const vat = /Value-Added/i.test(f.t) ? " vat" : "";
  return `${f.c} ${f.c.replace(/-/g, "")} ${f.t}${vat}`.toLowerCase();
}

/** A frequency choice from a control's value; anything that is not one of FREQS is "All". */
export function asFrequency(value: string): FrequencyFilter {
  return (FREQS as readonly string[]).includes(value) ? (value as FrequencyFilter) : "All";
}

/** The three things the library filters by. */
export interface FormFilter {
  scope: FormScope;
  /** The codes of the forms the selected taxpayer files this year. */
  owned: ReadonlySet<string>;
  freq: FrequencyFilter;
  /** What the search box holds; trimmed and lower-cased here. */
  query: string;
}

/** The forms a filter shows, in the index's order. */
export function matchingForms(forms: readonly ScreenFormIndexEntry[], f: FormFilter): ScreenFormIndexEntry[] {
  const needle = f.query.trim().toLowerCase();
  return forms.filter(
    (form) =>
      (f.scope === "all" || f.owned.has(form.c)) &&
      (f.freq === "All" || form.freq === f.freq) &&
      (!needle || searchText(form).includes(needle)),
  );
}
