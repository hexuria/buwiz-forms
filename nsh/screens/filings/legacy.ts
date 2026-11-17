// The group "Filed in BIR eBIRForms" on Filings, as pure rules: the returns the import brought in from the old program
// (bir.list_legacy_filed_returns) turned into the lines a person reads. They are records: nothing here opens, sends or
// changes a return. No signals and no host calls, so every rule is tested without a window (legacy.test.ts).
import type { LegacyFiledReturn } from "../../bir";
import { MONTHS, shortDate } from "../../lib/dates";

/** The group's title. */
export const LEGACY_TITLE = "Filed in BIR eBIRForms";

/** How often a form is filed; null where the host's catalog has no such form. */
export type LegacyFrequency = "Monthly" | "Quarterly" | "Annual" | "As needed" | null;

/** One line of the host's forms catalog (`forms_catalog`): the code and its frequency as the host spells it ("OpenEnded" for as needed). */
export interface CatalogLine {
  code: string;
  frequency: string;
}

/** The frequency the host files a form under (the one `get_form_filing_progress` counts it with); null for a code it does not list. */
export function catalogFrequency(catalog: readonly CatalogLine[], code: string | null): LegacyFrequency {
  const f = code === null ? undefined : catalog.find((c) => c.code === code)?.frequency;
  return f === "Monthly" || f === "Quarterly" || f === "Annual" ? f : f === "OpenEnded" ? "As needed" : null;
}

/** What a period of the old program's file names says: the year, and the month, quarter or day when it names one. */
export interface LegacyPeriod {
  year: number;
  month: number | null;
  quarter: number | null;
  day: number | null;
}

const validMonth = (m: number) => m >= 1 && m <= 12;

/**
 * The period as the old program wrote it in a file's name: `062025` (month and year), `2026Q1` (quarter), `122026Q1` (quarter of
 * a fiscal year ending in that month), `2025` (the year) or `03152026` (a date). Null for anything else (the text is shown as it is).
 */
export function parseLegacyPeriod(text: string): LegacyPeriod | null {
  let m = /^(\d{4})$/.exec(text);
  if (m) return { year: Number(m[1]), month: null, quarter: null, day: null };
  m = /^(\d{2})(\d{4})$/.exec(text);
  if (m) return validMonth(Number(m[1])) ? { year: Number(m[2]), month: Number(m[1]), quarter: null, day: null } : null;
  m = /^(\d{4})[Qq]([1-4])$/.exec(text);
  if (m) return { year: Number(m[1]), month: null, quarter: Number(m[2]), day: null };
  m = /^(\d{2})(\d{4})[Qq]([1-4])$/.exec(text);
  if (m) return validMonth(Number(m[1])) ? { year: Number(m[2]), month: Number(m[1]), quarter: Number(m[3]), day: null } : null;
  m = /^(\d{2})(\d{2})(\d{4})$/.exec(text);
  if (m) {
    const [month, day, year] = [Number(m[1]), Number(m[2]), Number(m[3])];
    return validMonth(month) && day >= 1 && day <= new Date(Date.UTC(year, month, 0)).getUTCDate() ? { year, month, quarter: null, day } : null;
  }
  return null;
}

/**
 * A period as the app writes periods elsewhere: "Jun 2025" for a month, "Q2 2025" for a quarter, "TY 2025" for a year, "Mar 15, 2026"
 * for a date. The same reading the counts make (`legacy_return_period`): a quarterly form's month is in its quarter, an annual form's
 * is the year, whatever month the file's name carries. A text that is no period is given back as it is.
 */
export function legacyPeriodLabel(period: string, freq: LegacyFrequency): string {
  const p = parseLegacyPeriod(period);
  if (!p) return period;
  if (p.day !== null && p.month !== null) return `${MONTHS[p.month - 1]} ${p.day}, ${p.year}`;
  if (p.quarter !== null) return `Q${p.quarter} ${p.year}`;
  if (p.month === null || freq === "Annual") return `TY ${p.year}`;
  if (freq === "Quarterly") return `Q${Math.ceil(p.month / 3)} ${p.year}`;
  return `${MONTHS[p.month - 1]} ${p.year}`;
}

/** Where a period falls in time, for newest first: the year, then the month (a quarter's last month, a whole year after December), then the day. Unreadable: 0. */
export function legacyRank(period: string): number {
  const p = parseLegacyPeriod(period);
  if (!p) return 0;
  const place = p.quarter !== null ? p.quarter * 3 : (p.month ?? 13);
  return p.year * 10000 + place * 100 + (p.day ?? 0);
}

/** One imported return and the branch it was filed for ("" for the head office, the branch's name for another). */
export interface LegacyEntry {
  row: LegacyFiledReturn;
  branch: string;
}

/** A line of the group. */
export interface LegacyLine {
  id: number;
  /** "1601C Jun 2025": the form's code (the old program's name for it where this app has none) and the period, as Filings names a return. */
  title: string;
  /** "Amended (v2)" for an amendment, else "". */
  amended: string;
  /** The copy that was e-mailed to BIR (the same period's original and amendment are listed too). */
  emailCopy: boolean;
  /** "Imported Nov 17, 2026". */
  imported: string;
  /** The branch's name when it is not the head office, else "". */
  branch: string;
}

/** "Amended (v2)" when the return is an amendment, else "". */
export const amendedText = (version: number | null): string => (version === null ? "" : `Amended (v${version})`);

/** "Imported Nov 17, 2026" of the database's timestamp (UTC, shown as the local day). */
export const importedText = (at: string): string => `Imported ${shortDate(at)}`;

/** The group's lines, newest period first; of one period an amendment before its original, the later import first. */
export function legacyLines(entries: readonly LegacyEntry[], catalog: readonly CatalogLine[]): LegacyLine[] {
  return [...entries]
    .sort(
      (a, b) =>
        legacyRank(b.row.period) - legacyRank(a.row.period) ||
        (b.row.amendment_version ?? 0) - (a.row.amendment_version ?? 0) ||
        (a.row.imported_at < b.row.imported_at ? 1 : a.row.imported_at > b.row.imported_at ? -1 : 0) ||
        b.row.id - a.row.id,
    )
    .map(({ row, branch }) => ({
      id: row.id,
      title: `${row.form_code ?? row.form} ${legacyPeriodLabel(row.period, catalogFrequency(catalog, row.form_code))}`,
      amended: amendedText(row.amendment_version),
      emailCopy: row.email_copy,
      imported: importedText(row.imported_at),
      branch,
    }));
}
