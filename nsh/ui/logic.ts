// The kit's pure helpers (no UI): avatar hues and initials, status chip wording, the filters of the pick lists.
// Ported from the old shell's ui.js; tested in logic.test.ts.

/** A stable hue (0-359) from a taxpayer id (TIN root) or a name. */
export function hueFor(key: string): number {
  const s = String(key || "");
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) % 360;
  return h;
}

/** Initials: the first and last capitalised words ("Juana Dela Cruz Santos" -> "JS"); "?" without one. */
export function initials(name: string): string {
  const words = String(name || "")
    .split(/\s+/)
    .filter((w) => /^[A-Z]/.test(w));
  if (!words.length) return "?";
  return words[0][0] + (words.length > 1 ? words[words.length - 1][0] : "");
}

/** "#rrggbb" from HSL (degrees, percent, percent). Avatar tints are data colors: derived from the key, not theme roles. */
export function hslToHex(h: number, s: number, l: number): string {
  s /= 100;
  l /= 100;
  const k = (n: number) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  const hex = (x: number) =>
    Math.round(x * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${hex(f(0))}${hex(f(8))}${hex(f(4))}`;
}

/** Avatar tones per appearance: saturation, fill lightness, initials lightness (the old app's AVATAR_TONE). */
const AVATAR_TONE = {
  light: { s: 55, bg: 90, fg: 30 },
  dark: { s: 40, bg: 26, fg: 84 },
} as const;

/** The avatar's fill and initials colors for a key in an appearance. */
export function avatarColors(key: string, scheme: "light" | "dark"): { bg: string; fg: string } {
  const tone = AVATAR_TONE[scheme];
  const hue = hueFor(key);
  return { bg: hslToHex(hue, tone.s, tone.bg), fg: hslToHex(hue, tone.s, tone.fg) };
}

/** A return's status as the chips name it. */
export type ReturnStatus = "not_started" | "draft" | "queued" | "submitted" | "dry_run" | "confirmed" | "paid" | "filed" | "due_soon" | "overdue";
/** A chip's tone. */
export type ChipTone = "neutral" | "accent" | "success" | "warning" | "danger";

/**
 * What a status chip says and its tone: "In 3 days" (warning), "2 days overdue" (danger), "Filed" (success, a check). Filings
 * names the filed states by their own words: "Submitted" (neutral, a send mark: waiting for BIR), "Confirmed" and "Paid"
 * (success, a check). A send only simulated says "Test send: not filed" (neutral, no send mark: nothing waits for BIR).
 */
export function statusChip(status: ReturnStatus | string, days?: number): { label: string; tone: ChipTone; check: boolean; send?: boolean } {
  const n = Math.abs(days ?? 0);
  switch (status) {
    case "draft":
      return { label: "Draft", tone: "accent", check: false };
    case "queued":
      return { label: "Queued", tone: "accent", check: false };
    case "submitted":
      return { label: "Submitted", tone: "neutral", check: false, send: true };
    case "dry_run":
      return { label: "Test send: not filed", tone: "neutral", check: false };
    case "confirmed":
      return { label: "Confirmed", tone: "success", check: true };
    case "paid":
      return { label: "Paid", tone: "success", check: true };
    case "filed":
      return { label: "Filed", tone: "success", check: true };
    case "due_soon":
      return { label: days === 0 ? "Due today" : `In ${days} day${days === 1 ? "" : "s"}`, tone: "warning", check: false };
    case "overdue":
      return { label: `${n} day${n === 1 ? "" : "s"} overdue`, tone: "danger", check: false };
    default:
      return { label: "Not started", tone: "neutral", check: false };
  }
}

/** One choice of a pick list: a code and its label ("050 - South Makati"), an optional second line. */
export interface PickOption {
  value: string;
  label: string;
  /** A second line: a TIN, a form's title, "3 returns". */
  sub?: string;
}

/** Options whose label (and second line) hold every typed word, ignoring case; all of them for an empty text. */
export function matchOptions<T extends PickOption>(options: readonly T[], text: string, limit = Infinity): T[] {
  const words = String(text || "")
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean);
  const out: T[] = [];
  for (const o of options) {
    if (out.length >= limit) break;
    const hay = `${o.label} ${o.sub || ""}`.toLowerCase();
    if (words.every((w) => hay.includes(w))) out.push(o);
  }
  return out;
}

/** The words of a multi-select's summary. */
export interface MultiComboWords {
  /** "All forms": every value on. */
  all: string;
  /** "forms": "3 forms", "No forms". */
  nouns: string;
}

/** A multi-select's closed summary: "All taxpayers", "Juana Santos", "3 taxpayers", "No taxpayers". `selected` null = all. */
export function multiComboLabel(words: MultiComboWords, options: readonly PickOption[], selected: ReadonlySet<string> | null): string {
  const on = selected === null ? options : options.filter((o) => selected.has(o.value));
  if (on.length === options.length) return words.all;
  if (on.length === 0) return `No ${words.nouns}`;
  if (on.length === 1) return on[0].label;
  return `${on.length} ${words.nouns}`;
}

/** A selection handed to the page: null once it holds every option (so values that appear later are in it too). */
export function normalizeSelection(next: ReadonlySet<string>, options: readonly PickOption[]): Set<string> | null {
  return options.every((o) => next.has(o.value)) ? null : new Set(next);
}
