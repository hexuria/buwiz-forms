// What a Home tile asks Filings to show (old app: app.filingsPreset, dashboard.js FilingsPreset). Home navigates with
// go("filings", { statuses, updatedMonth }), the preset's own names as route params; Filings (screens/filings/model.ts presetOf)
// applies them to its filters once, as it opens, and they replace every other filter.
import { go } from "../../shell";

/**
 * `statuses` are return statuses ("Draft", "Queued", "Submitted", "Confirmed", "Paid"); `updatedMonth` ("YYYY-MM") keeps the
 * returns updated in that month; `filedOnly` keeps only the returns that count as filed (`countsAsFiled`: a dry run does not).
 */
export interface FilingsPreset {
  statuses: string[];
  updatedMonth?: string;
  filedOnly?: boolean;
}

/** The stored statuses a filed return can have (a Submitted dry run is one of them but not filed: pair it with `filedOnly`). */
export const FILED_STATUSES = ["Submitted", "Confirmed", "Paid"] as const;

/** Opens Filings with the preset. */
export function openFilings(preset: FilingsPreset): void {
  go("filings", {
    statuses: preset.statuses,
    ...(preset.updatedMonth === undefined ? {} : { updatedMonth: preset.updatedMonth }),
    ...(preset.filedOnly ? { filedOnly: true } : {}),
  });
}
