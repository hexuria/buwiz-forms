// What a Home tile asks Filings to show (old app: app.filingsPreset, dashboard.js FilingsPreset). Home navigates with
// go("filings", { statuses, updatedMonth }), the preset's own names as route params; Filings (screens/filings/model.ts presetOf)
// applies them to its filters once, as it opens, and they replace every other filter.
import { go } from "../../shell";

/**
 * `statuses` are return statuses ("Draft", "Queued", "Submitted", "Confirmed", "Paid"); `updatedMonth` ("YYYY-MM") keeps the
 * returns updated in that month.
 */
export interface FilingsPreset {
  statuses: string[];
  updatedMonth?: string;
}

/** The statuses that count as filed. */
export const FILED_STATUSES = ["Submitted", "Confirmed", "Paid"] as const;

/** Opens Filings with the preset. */
export function openFilings(preset: FilingsPreset): void {
  go("filings", preset.updatedMonth === undefined ? { statuses: preset.statuses } : { statuses: preset.statuses, updatedMonth: preset.updatedMonth });
}
