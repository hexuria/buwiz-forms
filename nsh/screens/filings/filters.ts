// Filings' filters: one owner for the search, the four filters, the month a navigation preset asked for, and the narrow list's tab.
// They live in this module, not in the view, so they survive leaving Filings and coming back (the old app kept them in an
// object made at boot and cleared them with Clear filters or a navigation preset). Signals are private; read the
// accessors inside JSX or a closure.
import { batch, signal } from "nsh";
import { app } from "../../shell";
import { type FilingsPreset, type FilterValues, presetFilters, visibleColumns } from "./model";

const query = signal("");
const tps = signal<ReadonlySet<string> | null>(null);
const forms = signal<ReadonlySet<string> | null>(null);
const periods = signal<ReadonlySet<string> | null>(null);
const statuses = signal<ReadonlySet<string> | null>(null);
const updatedMonth = signal<string | null>(null);
const filedOnly = signal(false);
const tab = signal("all");

function reset(): void {
  query.set("");
  tps.set(null);
  forms.set(null);
  periods.set(null);
  statuses.set(null);
  updatedMonth.set(null);
  filedOnly.set(false);
}

/** The filters as the pure model takes them (reads every signal: call it inside a closure). */
function values(): FilterValues {
  return { query: query(), tps: tps(), forms: forms(), periods: periods(), statuses: statuses(), updatedMonth: updatedMonth(), filedOnly: filedOnly() };
}

/** How many of the four filters, the month and "Filed only" are narrowed (the Filters button's badge); the search is visible and not counted. */
function narrowed(): number {
  return [tps(), forms(), periods(), statuses()].filter((s) => s !== null).length + (updatedMonth() !== null ? 1 : 0) + (filedOnly() ? 1 : 0);
}

/** True when the search or any filter holds the list back (shows Clear filters). */
function active(): boolean {
  return query().trim() !== "" || narrowed() > 0;
}

/** The preset's params object last applied, so a Back to the same Filings page does not apply it a second time. */
let appliedFrom: object | null = null;

// The next session starts from the defaults (a lock, a sign-out or another taxpayer signing in): no filter, the All tab, and a
// navigation preset may apply again.
app.onSessionChange(() => {
  reset();
  tab.set("all");
  appliedFrom = null;
});

/** The one owner of Filings' filter state. */
export const filters = {
  query: () => query(),
  tps: () => tps(),
  forms: () => forms(),
  periods: () => periods(),
  statuses: () => statuses(),
  updatedMonth: () => updatedMonth(),
  filedOnly: () => filedOnly(),
  tab: () => tab(),
  setQuery: (v: string) => query.set(v),
  setTps: (v: ReadonlySet<string> | null) => tps.set(v),
  setForms: (v: ReadonlySet<string> | null) => forms.set(v),
  setPeriods: (v: ReadonlySet<string> | null) => periods.set(v),
  setStatuses: (v: ReadonlySet<string> | null) => {
    batch(() => {
      statuses.set(v);
      // A tab the filter hides cannot stay chosen: All is the one tab every selection keeps.
      if (tab() !== "all" && !visibleColumns(v).some((c) => c.id === tab())) tab.set("all");
    });
  },
  setTab: (v: string) => tab.set(v),
  clearMonth: () => updatedMonth.set(null),
  clearFiledOnly: () => filedOnly.set(false),
  values,
  narrowed,
  active,
  /** Clear filters: the search and every filter go back to all. The narrow list's tab stays. */
  clear: reset,
  /**
   * A navigation preset replaces every filter, once per navigation: `from` is the navigation's params object, which Back
   * hands over again unchanged. A preset moves the narrow list to All, where its returns show with their statuses.
   */
  applyPreset(preset: FilingsPreset, from: object): void {
    if (appliedFrom === from) return;
    appliedFrom = from;
    reset();
    const next = presetFilters(preset);
    statuses.set(next.statuses);
    updatedMonth.set(next.updatedMonth);
    filedOnly.set(next.filedOnly === true);
    tab.set("all");
  },
};
