// Masks for typed numbers: pure format and parse functions, no UI. The kit's TinInput, MoneyInput, PercentInput and
// IntInput call them on every change; screens call formatMoney / formatTin / parseTypedDate to show stored values.
//
// The caret rule (reconcile): an edit is found by comparing the text before it with the text after it (common
// prefix and suffix), the caret is put after the same number of significant characters (digits, the decimal point,
// a minus) it had in the raw text, so a separator the mask inserts or removes never moves the caret across a digit.
// Deleting a separator alone deletes the significant character before it (else the mask would put it back and the
// key would do nothing).

/** A masked text and where its caret goes (UTF-16 index). */
export interface Masked {
  text: string;
  caret: number;
}

/** What a mask does with the significant characters it is given: the kept ones (one flag each) and the text. */
export interface MaskResult {
  /** keep[i]: the i-th significant character of the input is in the output. */
  keep: boolean[];
  /** The formatted text. */
  text: string;
}

/** A mask: which characters count, and how the counted ones are formatted. */
export interface Mask {
  /** The character is part of the value (a digit, the point); everything else is dropped or is a separator. */
  significant: (ch: string) => boolean;
  /** Formats the significant characters, in order. */
  format: (sig: string) => MaskResult;
}

const isDigit = (ch: string) => ch >= "0" && ch <= "9";

/** Only the digits of a text: "000-123 453" -> "000123453". */
export function digitsOnly(text: string): string {
  let out = "";
  for (const ch of String(text ?? "")) if (isDigit(ch)) out += ch;
  return out;
}

function sigOf(text: string, mask: Mask): string {
  let out = "";
  for (const ch of text) if (mask.significant(ch)) out += ch;
  return out;
}

/** Position in `text` just after its `count`-th significant character (0: the start). */
function caretAfter(text: string, count: number, mask: Mask): number {
  if (count <= 0) return 0;
  let seen = 0;
  for (let i = 0; i < text.length; i++) {
    if (mask.significant(text[i])) {
      seen++;
      if (seen === count) return i + 1;
    }
  }
  return text.length;
}

/**
 * The masked result of an edit: `prev` is the text the field showed, `next` the text after the user's edit (typing,
 * deleting, pasting anywhere). `caret`, when known, is the raw caret in `next`; otherwise it is found from the edit.
 */
export function reconcile(prev: string, next: string, mask: Mask, caret?: number): Masked {
  prev = String(prev ?? "");
  next = String(next ?? "");
  let raw = next;
  // Where the edit ended in `next`: after the common prefix, before the common suffix.
  let p = 0;
  while (p < prev.length && p < next.length && prev[p] === next[p]) p++;
  let s = 0;
  while (s < prev.length - p && s < next.length - p && prev[prev.length - 1 - s] === next[next.length - 1 - s]) s++;
  let rawCaret = caret ?? next.length - s;
  // A separator deleted alone: delete the significant character before it instead.
  if (next.length < prev.length && sigOf(next, mask) === sigOf(prev, mask)) {
    let i = p - 1;
    while (i >= 0 && !mask.significant(next[i])) i--;
    if (i >= 0) {
      raw = next.slice(0, i) + next.slice(i + 1);
      rawCaret = i;
    }
  }
  const sig = sigOf(raw, mask);
  let before = 0;
  for (let i = 0; i < Math.min(rawCaret, raw.length); i++) if (mask.significant(raw[i])) before++;
  const out = mask.format(sig);
  let kept = 0;
  for (let i = 0; i < before && i < out.keep.length; i++) if (out.keep[i]) kept++;
  return { text: out.text, caret: caretAfter(out.text, kept, mask) };
}

/** A whole text through a mask, as a paste into an empty field: the caret at the end. */
export function applyMask(text: string, mask: Mask): string {
  return mask.format(sigOf(String(text ?? ""), mask)).text;
}

// ---------------------------------------------------------------- TIN

/** The TIN's printed groups: 3-3-3 for the taxpayer, 5 for the branch. */
export const TIN_GROUPS = [3, 3, 3, 5] as const;
/** Digits in a full TIN with its branch code. */
export const TIN_LENGTH = 14;

/** Digits as the printed TIN while typing: "0001234" -> "000-123-4"; no trailing dash, at most 14 digits. */
export function formatTinDigits(digits: string): string {
  const d = digitsOnly(digits).slice(0, TIN_LENGTH);
  const parts: string[] = [];
  let at = 0;
  for (const n of TIN_GROUPS) {
    if (at >= d.length) break;
    parts.push(d.slice(at, at + n));
    at += n;
  }
  return parts.join("-");
}

/** The TIN mask: digits only, grouped 3-3-3-5 with dashes, leading zeros kept. */
export const TIN_MASK: Mask = {
  significant: isDigit,
  format(sig) {
    const keep = [...sig].map((_, i) => i < TIN_LENGTH);
    return { keep, text: formatTinDigits(sig) };
  },
};

/** A TIN from its root and branch, as the old store wrote it: "000-123-453-00000"; a short root stays as given. */
export function formatTin(root: string, branch?: string | null): string {
  const r = digitsOnly(root);
  const base = r.length >= 9 ? `${r.slice(0, 3)}-${r.slice(3, 6)}-${r.slice(6, 9)}` : r;
  return branch ? `${base}-${branch}` : base;
}

// ---------------------------------------------------------------- money

/** Thousands separators on a run of digits: "1250000" -> "1,250,000". */
export function groupThousands(int: string): string {
  return int.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

/** Options of the money mask. */
export interface MoneyMaskOptions {
  /** A leading minus is kept (amounts that may go below zero, as the old formatMoney allowed). Default false. */
  negative?: boolean;
}

/**
 * The money mask while typing: digits, one decimal point and (with `negative`) a leading minus; thousands separators
 * on the whole part; leading zeros of the whole part dropped ("007" -> "7", "0.5" stays). The decimals stay as typed:
 * formatMoney on leaving the box writes two of them.
 */
export function moneyMask(o?: MoneyMaskOptions): Mask {
  return {
    significant: (ch) => isDigit(ch) || ch === "." || (!!o?.negative && ch === "-"),
    format(sig) {
      const keep: boolean[] = new Array(sig.length).fill(false);
      let neg = false;
      let int = "";
      const intAt: number[] = [];
      let dot = false;
      let frac = "";
      for (let i = 0; i < sig.length; i++) {
        const ch = sig[i];
        if (ch === "-") {
          if (i === 0 && o?.negative) {
            neg = true;
            keep[i] = true;
          }
        } else if (ch === ".") {
          if (!dot) {
            dot = true;
            keep[i] = true;
          }
        } else if (dot) {
          frac += ch;
          keep[i] = true;
        } else {
          int += ch;
          intAt.push(i);
          keep[i] = true;
        }
      }
      // Leading zeros of the whole part: one stays while it is all zeros.
      let z = 0;
      while (z < int.length - 1 && int[z] === "0") z++;
      for (let k = 0; k < z; k++) keep[intAt[k]] = false;
      int = int.slice(z);
      const text = `${neg ? "-" : ""}${groupThousands(int)}${dot ? `.${frac}` : ""}`;
      return { keep, text };
    },
  };
}

/** The money mask with no minus. */
export const MONEY_MASK = moneyMask();

/** The plain numeric string of a money text: "1,250,000.50" -> "1250000.50", "₱ -12" -> "-12"; "" stays "". */
export function moneyPlain(text: string): string {
  return String(text ?? "").replace(/[₱,\s]/g, "");
}

/**
 * "1,234.50" from a number-ish string, the old app's formatMoney (app/shell/form_page.js): empty stays empty, text
 * that is not a number stays as it is. `whole`: whole pesos ("1,234"); a typed fraction stays as typed so the box's
 * own message explains it.
 */
export function formatMoney(text: string, whole = false): string {
  const n = Number(String(text).replace(/[₱,\s]/g, ""));
  if (String(text).trim() === "" || Number.isNaN(n)) return String(text);
  if (whole && !Number.isInteger(n)) return String(text);
  const [int, frac] = Math.abs(n).toFixed(whole ? 0 : 2).split(".");
  const grouped = `${n < 0 ? "-" : ""}${groupThousands(int)}`;
  return whole ? grouped : `${grouped}.${frac}`;
}

// ---------------------------------------------------------------- percent and integers

/** Percent while typing: digits and one decimal point, no grouping. */
export const PERCENT_MASK: Mask = {
  significant: (ch) => isDigit(ch) || ch === ".",
  format(sig) {
    let dot = false;
    const keep = [...sig].map((ch) => {
      if (ch !== ".") return true;
      if (dot) return false;
      dot = true;
      return true;
    });
    return { keep, text: [...sig].filter((_, i) => keep[i]).join("") };
  },
};

/** Whole numbers while typing: digits only, at most `max` of them (none: no limit). Leading zeros stay (codes). */
export function intMask(max?: number): Mask {
  return {
    significant: isDigit,
    format(sig) {
      const keep = [...sig].map((_, i) => max === undefined || i < max);
      return { keep, text: [...sig].filter((_, i) => keep[i]).join("") };
    },
  };
}

// ---------------------------------------------------------------- dates

/** A calendar day. */
export interface Ymd {
  y: number;
  m: number;
  d: number;
}

const MONTH_SHORT = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/**
 * A date the filer typed, read as the old app read it (form_page.js parseTypedDate): 12012020, 12/01/2020,
 * 12-01-2020, 2020-12-01 or Dec 01 2020 all mean December 1, 2020. Null when it is not a real day.
 */
export function parseTypedDate(text: string): Ymd | null {
  const v = String(text || "").trim();
  if (!v) return null;
  const MON = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
  let y: string | number, m: string | number, d: string | number;
  let hit: RegExpExecArray | null;
  if ((hit = /^(\d{2})(\d{2})(\d{4})$/.exec(v))) [, m, d, y] = hit;
  else if ((hit = /^(\d{4})-(\d{1,2})-(\d{1,2})$/.exec(v))) [, y, m, d] = hit;
  else if ((hit = /^(\d{1,2})[/-](\d{1,2})[/-](\d{4})$/.exec(v))) [, m, d, y] = hit;
  else if ((hit = /^([a-z]{3})[a-z]*\.?\s+(\d{1,2}),?\s+(\d{4})$/i.exec(v))) {
    m = MON.indexOf(hit[1].toLowerCase()) + 1;
    d = hit[2];
    y = hit[3];
  } else return null;
  const Y = Number(y);
  const M = Number(m);
  const D = Number(d);
  if (Y < 1900 || Y > 2100 || M < 1 || M > 12 || D < 1) return null;
  if (D > new Date(Date.UTC(Y, M, 0)).getUTCDate()) return null;
  return { y: Y, m: M, d: D };
}

/** The form's MM/DD/YYYY text: "12/01/2020". */
export function formatMDY(date: Ymd): string {
  return `${String(date.m).padStart(2, "0")}/${String(date.d).padStart(2, "0")}/${date.y}`;
}

/** How a date box shows a date: "Dec 10, 2020". */
export function formatLongDate(date: Ymd): string {
  return `${MONTH_SHORT[date.m - 1]} ${date.d}, ${date.y}`;
}

/** An ISO day, "2020-12-01", as the calendar speaks. */
export function formatIso(date: Ymd): string {
  return `${date.y}-${String(date.m).padStart(2, "0")}-${String(date.d).padStart(2, "0")}`;
}

/** What a date box shows for a stored text: "Dec 10, 2020" when it reads as a date, else the text as it is. */
export function shownDate(text: string): string {
  const date = parseTypedDate(text);
  return date ? formatLongDate(date) : String(text ?? "");
}

/** The message under a date box that holds no date. */
export const DATE_ERROR = "Enter a valid date as MM/DD/YYYY (e.g. 12/01/2020 or 12012020)";
