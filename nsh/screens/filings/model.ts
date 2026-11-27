// Filings' pure model: the statuses and board columns, how a return is named and dated, the period and month keys the
// filters use, and which returns the search and the four filters keep. No signals and no host calls here, so every
// rule is tested without a window (model.test.ts). Ported from the old shell's filings.js (STATUSES, COLUMNS, periodKey,
// localMonth, returnTitle, namedBranch, whoLine, dateLine, FilingsFilters.rows and applyPreset).
import type { ReturnRow } from "../../bir";
import { MONTHS, shortDate } from "../../lib/dates";
import { countsAsFiled, statusWord } from "../../lib/statusWord";
import { type FormIndexEntry, frequencyOf, periodLabel } from "../../lib/periods";

/** The real statuses, in board order. */
export const STATUSES = ["Draft", "Queued", "Submitted", "Confirmed", "Paid"] as const;

/** The dot colour of a status in the statuses filter: the accent, the quiet grey, or success. */
export type DotTone = "accent" | "quiet" | "success";

/** The dot's tone per status. */
export const STATUS_TONE: Record<string, DotTone> = { Draft: "accent", Queued: "accent", Submitted: "quiet", Confirmed: "success", Paid: "success" };

/** A board column: its test id, its title and the statuses it holds (Paid joins Confirmed). */
export interface Column {
  id: string;
  title: string;
  statuses: readonly string[];
}

/** The four board columns, in order. */
export const COLUMNS: readonly Column[] = [
  { id: "draft", title: "Draft", statuses: ["Draft"] },
  { id: "queued", title: "Queued", statuses: ["Queued"] },
  { id: "submitted", title: "Submitted", statuses: ["Submitted"] },
  { id: "confirmed", title: "Confirmed", statuses: ["Confirmed", "Paid"] },
];

/** "1 return", "3 returns". */
export const plural = (n: number, word: string): string => `${n} ${word}${n === 1 ? "" : "s"}`;

/** The form library's index as Filings reads it: a code, its title and how often it is filed (the host's entries carry more). */
export type FormIndex = readonly { c: string; freq: string; t?: string }[];

/** Newest change first. A SQLite timestamp sorts as text. */
export function newestFirst(rows: readonly ReturnRow[]): ReturnRow[] {
  return [...rows].sort((a, b) => (a.updated_at < b.updated_at ? 1 : a.updated_at > b.updated_at ? -1 : 0));
}

/** "Q3 2026" for a monthly or quarterly return, "TY 2026" for the rest. */
export function periodKey(index: FormIndex, r: ReturnRow): string {
  const freq = frequencyOf(index as readonly FormIndexEntry[], r.form_code);
  const q = freq === "Monthly" ? Math.ceil(r.period / 3) : freq === "Quarterly" ? r.period : 0;
  return q >= 1 && q <= 4 ? `Q${q} ${r.taxable_year}` : `TY ${r.taxable_year}`;
}

/** Sort rank of a period key: newer years first, then later quarters, TY after Q4. */
export function periodRank(key: string): number {
  const m = /^(?:Q(\d)|TY) (\d{4})$/.exec(key);
  return m ? Number(m[2]) * 10 + (m[1] ? Number(m[1]) : 5) : 0;
}

/** "2026-11" of a SQLite UTC timestamp, in local time. */
export function localMonth(text: string | null | undefined): string {
  const ts = /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2})/.exec(String(text || ""));
  if (!ts) return String(text || "").slice(0, 7);
  const d = new Date(Date.UTC(+ts[1], +ts[2] - 1, +ts[3], +ts[4], +ts[5]));
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

/** "Nov 2026" of "2026-11". */
export const monthLabel = (ym: string): string => `${MONTHS[Number(ym.slice(5, 7)) - 1]} ${ym.slice(0, 4)}`;

/** Head office is the taxpayer itself: only another branch is worth naming. */
export const namedBranch = (r: ReturnRow): string => (/^0*$/.test(String(r.branch_code || "")) ? "" : r.branch_name || "");

/** "Juana Santos" or "Juana Santos · Cebu branch". */
export const whoLine = (r: ReturnRow): string => (namedBranch(r) ? `${r.taxpayer_name} · ${namedBranch(r)}` : r.taxpayer_name);

/** "2551Q Q3 2026", "1601C Jan 2026", "1702RT TY 2026": a return as Recent names it. */
export function returnTitle(index: FormIndex, r: ReturnRow): string {
  const freq = frequencyOf(index as readonly FormIndexEntry[], r.form_code);
  const period = freq === "As needed" ? r.period_label : periodLabel(freq, r.taxable_year, r.period, true);
  return `${r.form_code} ${period}`;
}

/** A draft's date is its last save; any other return's is its last change. */
export const dateLine = (r: ReturnRow): string => `${r.status === "Draft" ? "Saved" : "Updated"} ${shortDate(r.updated_at)}`;

/** The send states a card calls out, with its chip's words and tone (the host's `send.stage`). */
export const SEND_CHIPS: Readonly<Record<string, { label: string; tone: "neutral" | "warning" | "danger" }>> = {
  checking: { label: "Checking", tone: "neutral" },
  held: { label: "Held", tone: "neutral" },
  waiting: { label: "Retrying", tone: "warning" },
  needs_attention: { label: "Needs attention", tone: "warning" },
  refused: { label: "Not sent", tone: "danger" },
};

/** A card's send chip and its line (the host's `send.line`), or null where the card says only its status. */
export function sendChip(r: ReturnRow): { label: string; tone: "neutral" | "warning" | "danger"; line: string } | null {
  const chip = r.send ? SEND_CHIPS[r.send.stage] : undefined;
  return chip && r.send ? { ...chip, line: r.send.line } : null;
}

/** What a screen reader says of a card. */
export const cardLabel = (r: ReturnRow): string => `${r.form_code} ${r.period_label}, ${whoLine(r)}, ${statusWord(r)}. Open`;

/** The search and the four filters; null in a set means every value. */
export interface FilterValues {
  query: string;
  tps: ReadonlySet<string> | null;
  forms: ReadonlySet<string> | null;
  periods: ReadonlySet<string> | null;
  statuses: ReadonlySet<string> | null;
  /** "YYYY-MM": only returns last changed in that month (a navigation preset). */
  updatedMonth: string | null;
  /** Only returns that count as filed (`countsAsFiled`: no dry run). */
  filedOnly?: boolean;
}

/** The returns the search and every filter keep. */
export function filterRows(rows: readonly ReturnRow[], index: FormIndex, f: FilterValues): ReturnRow[] {
  const words = f.query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  return rows.filter((r) => {
    if (f.tps && !f.tps.has(r.tin_root)) return false;
    if (f.forms && !f.forms.has(r.form_code)) return false;
    if (f.statuses && !f.statuses.has(r.status)) return false;
    if (f.periods && !f.periods.has(periodKey(index, r))) return false;
    if (f.updatedMonth && localMonth(r.updated_at) !== f.updatedMonth) return false;
    if (f.filedOnly && !countsAsFiled(r)) return false;
    if (!words.length) return true;
    const hay = `${r.form_code} ${r.period_label} ${r.taxpayer_name} ${r.branch_name} ${r.status} ${r.tin}`.toLowerCase();
    return words.every((w) => hay.includes(w));
  });
}

/** What a navigation preset asks Filings to show: statuses ("Draft", "Queued", ...) and a month ("YYYY-MM"), as dashboard.js FilingsPreset. */
export interface FilingsPreset {
  statuses?: readonly string[];
  updatedMonth?: string;
  /** Only returns that count as filed (a dry run is left out). */
  filedOnly?: boolean;
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

/**
 * The preset in a navigation's params, or null. Home sends `go("filings", { statuses })`; a preset can also carry
 * updatedMonth and filedOnly, the old preset's own names. The same keys inside a `preset` object are read too.
 */
export function presetOf(params: Readonly<Record<string, unknown>>): FilingsPreset | null {
  const src = isRecord(params.preset) ? params.preset : params;
  const statuses = Array.isArray(src.statuses) ? src.statuses.filter((s): s is string => typeof s === "string") : undefined;
  const updatedMonth = typeof src.updatedMonth === "string" ? src.updatedMonth : undefined;
  const filedOnly = src.filedOnly === true ? true : undefined;
  if (statuses === undefined && updatedMonth === undefined && filedOnly === undefined) return null;
  return filedOnly ? { statuses, updatedMonth, filedOnly } : { statuses, updatedMonth };
}

/** The filters a preset sets (it replaces every filter): the real statuses it names, and a well-formed month. */
export function presetFilters(preset: FilingsPreset): Pick<FilterValues, "statuses" | "updatedMonth" | "filedOnly"> {
  let statuses: ReadonlySet<string> | null = null;
  if (preset.statuses) {
    const on = new Set(preset.statuses.filter((s) => (STATUSES as readonly string[]).includes(s)));
    statuses = on.size === STATUSES.length ? null : on;
  }
  const updatedMonth = typeof preset.updatedMonth === "string" && /^\d{4}-\d{2}$/.test(preset.updatedMonth) ? preset.updatedMonth : null;
  return preset.filedOnly === true ? { statuses, updatedMonth, filedOnly: true } : { statuses, updatedMonth };
}
