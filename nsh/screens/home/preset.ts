// What a Home tile asks Filings to show (old app: app.filingsPreset, dashboard.js FilingsPreset). Home navigates with
// go("filings", { statuses }), the preset's own name as a route param; Filings (screens/filings/model.ts presetOf)
// applies it to its filters once, as it opens, and it replaces every other filter.
import { go } from "../../shell";

/** `statuses` are return statuses ("Draft", "Queued", "Submitted", "Confirmed", "Paid"). */
export interface FilingsPreset {
  statuses: string[];
}

/** Opens Filings with the preset. */
export function openFilings(preset: FilingsPreset): void {
  go("filings", { statuses: preset.statuses });
}
