// Masks for typed numbers: pure format and parse functions, no UI. The kit's TinInput, MoneyInput, PercentInput and
// IntInput call them on every change; screens call formatMoney / formatTin / parseTypedDate to show stored values.
//
// The caret rule (reconcile): an edit is found by comparing the text before it with the text after it. When the input
// reported the selection the edit started from (NSH's onKeyPress comes before the text changes, onSelectionChange gives
// the selection: fields.tsx keeps the last one), the edit is exact; without it the common prefix and suffix are the
// guess, and a typed digit equal to its neighbour is ambiguous. The caret is put after the same number of significant
// characters (digits, the decimal point, a minus) it had in the raw text, so a separator the mask inserts or removes never
// moves the caret across a digit. Deleting a separator alone deletes the significant character beside it (Backspace: the
// one before, Delete: the one after; else the mask would put the separator back and the key would do nothing).
//
// Signs: a minus is part of an amount's text and is always shown (a stored or computed negative is never shown
// positive). Typing or pasting one is allowed only where the field takes negatives; elsewhere the edit is refused whole
// (Masked.rejected) rather than silently made positive.
//
// Money never goes through Number: sign, digits and decimals are handled as strings, so amounts of any length keep every
// digit and rounding is exact decimal rounding (half up, away from zero for negatives).

/** A masked text and where its caret goes (UTF-16 index). */
export interface Masked {
  text: string;
  caret: number;
  /** The edit was refused: text and caret are as they were before it. */
  rejected?: boolean;
}

/** A selection in a text (UTF-16 units), as NSH's onSelectionChange reports it; a caret has start equal to end. */
export interface TextSelection {
  start: number;
  end: number;
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
  /**
   * Looks at the text an edit inserted before the mask does: the text to take in its place, or null to refuse the whole
   * edit (the box keeps what it showed). `typed`: it came from key presses, so it is only cleaned (a minus nobody may type
   * is dropped, the digits beside it stay); otherwise it is a paste and judged as a whole. Only reconcile calls it, so
   * applyMask (a stored value on its way to the screen) is never refused.
   */
  insert?: (inserted: string, typed: boolean) => string | null;
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

/** What an edit did to the text before it: `removed` characters from `at` became `ins`. */
interface Edit {
  at: number;
  removed: number;
  ins: string;
  /** A delete that took the characters before the caret (Backspace) or after it (Delete). */
  dir: "back" | "forward";
}

function findEdit(prev: string, next: string, caret?: number, before?: TextSelection): Edit {
  // The selection the edit started from says exactly what it did, when the texts agree with it.
  if (before && Number.isInteger(before.start) && Number.isInteger(before.end)) {
    const a = Math.min(before.start, before.end);
    const b = Math.max(before.start, before.end);
    if (a >= 0 && b <= prev.length) {
      const grew = next.length - (prev.length - (b - a)); // the length of what replaced the selection
      if (grew >= 0 && next.slice(0, a) === prev.slice(0, a) && next.slice(a + grew) === prev.slice(b)) {
        return { at: a, removed: b - a, ins: next.slice(a, a + grew), dir: "back" };
      }
      if (a === b && grew < 0) {
        // Characters went and none came, from a caret: Backspace took the ones before it, Delete the ones after it.
        const n = -grew;
        if (a - n >= 0 && next === prev.slice(0, a - n) + prev.slice(a)) return { at: a - n, removed: n, ins: "", dir: "back" };
        if (a + n <= prev.length && next === prev.slice(0, a) + prev.slice(a + n)) return { at: a, removed: n, ins: "", dir: "forward" };
      }
    }
  }
  // The guess: what is left after the common prefix and the common suffix.
  let p = 0;
  while (p < prev.length && p < next.length && prev[p] === next[p]) p++;
  let s = 0;
  while (s < prev.length - p && s < next.length - p && prev[prev.length - 1 - s] === next[next.length - 1 - s]) s++;
  // The caret after the edit settles where an inserted run of equal characters began.
  if (caret !== undefined && Number.isInteger(caret) && caret >= 0 && caret <= next.length) {
    const tail = next.length - caret;
    if (tail <= prev.length && prev.slice(prev.length - tail) === next.slice(caret)) {
      const j = prev.length - tail;
      const at = Math.min(p, j, caret);
      return { at, removed: j - at, ins: next.slice(at, caret), dir: "back" };
    }
  }
  return { at: p, removed: prev.length - p - s, ins: next.slice(p, next.length - s), dir: "back" };
}

/**
 * The masked result of an edit: `prev` is the text the field showed, `next` the text after the user's edit (typing,
 * deleting, pasting anywhere). `caret`, when known, is the raw caret in `next`; `before`, when known, is the selection the
 * edit started from (in `prev`), which makes the edit exact: typing a digit beside an equal one, and Delete against
 * Backspace on a separator, cannot be told apart from the two texts alone. Hints that do not fit the two texts are ignored.
 * `typed`: what was inserted came from key presses (a paste, and an edit of unknown origin, is judged by Mask.insert as a
 * whole: a pasted "-1,234.00" is refused where a typed minus is only dropped).
 */
export function reconcile(
  prev: string,
  next: string,
  mask: Mask,
  caret?: number,
  before?: TextSelection,
  typed = false,
): Masked {
  prev = String(prev ?? "");
  next = String(next ?? "");
  const e = findEdit(prev, next, caret, before);
  let ins = e.ins;
  if (ins !== "" && mask.insert) {
    const taken = mask.insert(ins, typed);
    if (taken === null) return { text: prev, caret: e.at, rejected: true };
    if (taken !== ins) {
      next = next.slice(0, e.at) + taken + next.slice(e.at + ins.length);
      ins = taken;
    }
  }
  let raw = next;
  let rawCaret = e.at + ins.length;
  // Separators deleted alone: delete the significant character beside them instead.
  if (ins === "" && e.removed > 0 && sigOf(prev.slice(e.at, e.at + e.removed), mask) === "") {
    if (e.dir === "forward") {
      let i = e.at;
      while (i < raw.length && !mask.significant(raw[i])) i++;
      if (i < raw.length) {
        raw = next.slice(0, i) + next.slice(i + 1);
        rawCaret = e.at;
      }
    } else {
      let i = e.at - 1;
      while (i >= 0 && !mask.significant(next[i])) i--;
      if (i >= 0) {
        raw = next.slice(0, i) + next.slice(i + 1);
        rawCaret = i;
      }
    }
  }
  const sig = sigOf(raw, mask);
  let count = 0;
  for (let i = 0; i < Math.min(rawCaret, raw.length); i++) if (mask.significant(raw[i])) count++;
  const out = mask.format(sig);
  let kept = 0;
  for (let i = 0; i < count && i < out.keep.length; i++) if (out.keep[i]) kept++;
  return { text: out.text, caret: caretAfter(out.text, kept, mask) };
}

/** A whole text through a mask, as a paste into an empty field: the caret at the end. Never refused (see Mask.insert). */
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

/**
 * A TIN as printed: every digit given, grouped 3-3-3-5 ("0001234530" -> "000-123-453-0", "1234" -> "123-4"). With a
 * `branch` code the root (its first 9 digits) is followed by the code as written, as the old store wrote it:
 * "000-123-453-00000".
 */
export function formatTin(root: string, branch?: string | null): string {
  const r = digitsOnly(root);
  return branch ? `${formatTinDigits(r.slice(0, 9))}-${branch}` : formatTinDigits(r);
}

// ---------------------------------------------------------------- amounts (strings only)

/** Thousands separators on a run of digits: "1250000" -> "1,250,000". */
export function groupThousands(int: string): string {
  return int.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

/** A plain amount's parts: its sign, whole digits and decimals; null when the text is not an amount. */
function amountParts(text: string): { neg: boolean; int: string; frac: string } | null {
  const m = /^(-?)(\d*)(?:\.(\d*))?$/.exec(String(text ?? "").replace(/[₱,\s]/g, ""));
  if (!m || (m[2] === "" && !m[3])) return null;
  return { neg: m[1] === "-", int: m[2], frac: m[3] ?? "" };
}

/** An amount's one spelling: no separators, no leading or trailing zeros, no "-0"; null when it is not an amount. */
function canonicalAmount(text: string): string | null {
  const a = amountParts(text);
  if (!a) return null;
  const int = a.int.replace(/^0+/, "");
  const frac = a.frac.replace(/0+$/, "");
  if (!int && !frac) return "0";
  return `${a.neg ? "-" : ""}${int || "0"}${frac ? `.${frac}` : ""}`;
}

/** Whether a stored text is a plain decimal amount ("1250.5", "-12", ".5", "1,250.00"; not "1e21", "Infinity", "abc"). */
export function isPlainAmount(text: string): boolean {
  return canonicalAmount(text) !== null;
}

/** Two amounts are the same number ("1250.5" and "1,250.50"); anything that is not an amount is the same only when equal. */
export function sameAmount(a: string, b: string): boolean {
  if (a === b) return true;
  const x = canonicalAmount(a);
  return x !== null && x === canonicalAmount(b);
}

/** Options of the money mask. */
export interface MoneyMaskOptions {
  /** A minus may be typed or pasted (amounts that may go below zero). Default false. A negative amount is shown either way. */
  negative?: boolean;
}

/**
 * The sign and separators of text inserted into a money box. Typed: a minus goes unless negatives are allowed, the rest
 * stays for the mask. Pasted: null (refused) for a negative where they are not allowed, and for separators that could mean
 * two things.
 */
function amountInsert(negative: boolean): (inserted: string, typed: boolean) => string | null {
  return (inserted, typed) => {
    let t = inserted.replace(/\u2212/g, "-");
    if (typed) return negative ? t : t.replace(/-/g, "");
    // Accounting negatives: "(1,234.00)".
    const paren = /^[\s₱]*\(([^()]*\d[^()]*)\)\s*$/.exec(t);
    if (paren) t = `-${paren[1]}`;
    if (/^[\s₱]*-/.test(t) && !negative) return null;
    return amountSeparators(t);
  };
}

/**
 * Separators in a pasted amount, when it has two or more of them (a single "." or "," is left to the mask). Both kinds:
 * the last one is the decimal point when it appears once, has the other kind only before it and is followed by exactly one
 * or two digits ("1,234.56" and "1.234,56" are 1234.56). One kind repeated: it groups thousands when every group after the
 * first has three digits and the first has one to three ("1.234.567", "1,234,567" are 1234567). Anything else is ambiguous:
 * null, and the paste is refused.
 */
function amountSeparators(t: string): string | null {
  const seps = t.match(/[.,]/g);
  if (!seps || seps.length < 2) return t;
  const dots = seps.filter((c) => c === ".").length;
  const commas = seps.length - dots;
  if (dots && commas) {
    const last = Math.max(t.lastIndexOf("."), t.lastIndexOf(","));
    const point = t[last];
    const other = point === "." ? "," : ".";
    const after = /^\d*/.exec(t.slice(last + 1))![0].length;
    if ((point === "." ? dots : commas) !== 1 || after < 1 || after > 2 || t.lastIndexOf(other) > last) return null;
    const body = t.split(other).join("");
    return point === "." ? body : body.replace(",", ".");
  }
  const parts = t.split(dots ? "." : ",");
  const head = digitsOnly(parts[0]).length;
  if (head < 1 || head > 3 || !parts.slice(1).every((p) => digitsOnly(p).length === 3)) return null;
  return parts.join("");
}

/**
 * The money mask while typing: digits, one decimal point and a leading minus; thousands separators on the whole part;
 * leading zeros of the whole part dropped ("007" -> "7", "0.5" stays). The decimals stay as typed: formatMoney on
 * leaving the box writes two of them. A minus that is there is kept; with `negative` unset one cannot be typed or
 * pasted (reconcile refuses the edit).
 */
export function moneyMask(o?: MoneyMaskOptions): Mask {
  return {
    significant: (ch) => isDigit(ch) || ch === "." || ch === "-",
    insert: amountInsert(!!o?.negative),
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
          if (i === 0) {
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

/** The money mask with no minus to type. */
export const MONEY_MASK = moneyMask();

/**
 * Whether a number box shows an amount of zero ("0", "0.00", "-0.0", "₱ 0"): it has a digit and nothing but zeros and the
 * marks of an amount. An empty text is not (nothing to replace), nor is text that is not an amount ("1e21", "abc").
 */
export function isZeroAmount(text: string): boolean {
  const t = String(text ?? "");
  return /\d/.test(t) && /^[₱\s,.\-0]*$/.test(t);
}

/** The plain numeric string of a money text: "1,250,000.50" -> "1250000.50", "₱ -12" -> "-12"; "" stays "". */
export function moneyPlain(text: string): string {
  return String(text ?? "").replace(/[₱,\s]/g, "");
}

/** The digits of `int`.`frac` rounded to `places` decimals, half up, on the digits themselves. */
function roundDecimals(int: string, frac: string, places: number): [string, string] {
  if (frac.length <= places) return [int, frac.padEnd(places, "0")];
  let digits = int + frac.slice(0, places);
  if (frac[places] >= "5") {
    const d = digits.split("");
    let i = d.length - 1;
    while (i >= 0 && d[i] === "9") d[i--] = "0";
    if (i >= 0) d[i] = String(Number(d[i]) + 1);
    else d.unshift("1");
    digits = d.join("");
  }
  const cut = digits.length - places;
  return [digits.slice(0, cut), digits.slice(cut)];
}

/**
 * "1,234.50" from an amount, written on the digits themselves (no Number: any length, exact rounding): empty stays
 * empty, text that is not a plain decimal ("abc", "1e21", "Infinity") stays as it is. More than two decimals round half
 * up ("1.255" is "1.26", "2.675" is "2.68", "1.005" is "1.01"; a negative rounds away from zero); a negative that rounds
 * to zero is "0.00", never "-0.00". `whole`: whole pesos ("1,234"); a typed fraction stays as typed so the box's own message
 * explains it. `round` false: decimals past the second stay as they are ("1.255" is "1.255", "1.5" is "1.50"): how a box
 * shows a stored value without changing it. The old app's formatMoney (app/shell/form_page.js) did this through Number and
 * toFixed.
 */
export function formatMoney(text: string, whole = false, round = true): string {
  const raw = String(text ?? "");
  const a = amountParts(raw);
  if (raw.trim() === "" || !a) return raw;
  if (whole && /[1-9]/.test(a.frac)) return raw;
  let int = a.int.replace(/^0+(?=\d)/, "") || "0";
  let frac = a.frac;
  if (!whole) [int, frac] = round || frac.length <= 2 ? roundDecimals(int, frac, 2) : [int, frac];
  const neg = a.neg && /[1-9]/.test(int + frac);
  const grouped = `${neg ? "-" : ""}${groupThousands(int)}`;
  return whole ? grouped : `${grouped}.${frac}`;
}

/**
 * What a money box shows for a value, never another number than the value: a plain amount as typed while the box has the
 * keys (separators, any sign) and laid out by formatMoney (grouped, two decimals at least, none rounded away) when it does
 * not; anything else (a stored "1e21", text) exactly as it is.
 */
export function shownMoney(value: string, focused: boolean, whole = false): string {
  if (!isPlainAmount(value)) return String(value ?? "");
  return focused ? applyMask(value, moneyMask({ negative: true })) : formatMoney(value, whole, false);
}

// ---------------------------------------------------------------- percent and integers

/** A pasted minus ahead of the digits is refused, never made positive by dropping it; typed, it is dropped by the mask. */
function noMinus(inserted: string, typed: boolean): string | null {
  return !typed && /^\s*[-\u2212]/.test(inserted) ? null : inserted;
}

/** Percent while typing: digits and one decimal point, no grouping. A pasted minus or a second point refuses the paste. */
export const PERCENT_MASK: Mask = {
  significant: (ch) => isDigit(ch) || ch === ".",
  insert: (inserted, typed) => (!typed && (inserted.match(/\./g) ?? []).length > 1 ? null : noMinus(inserted, typed)),
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
    insert: noMinus,
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
