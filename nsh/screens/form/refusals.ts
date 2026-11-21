// The one sentence table for what the host refuses (B3/B4): every refusal kind the host can answer
// becomes a plain sentence that begins with the item number when the box has one, else the box
// label. The raw host text (keys, JSON paths, backticks) is for the log only; it never reaches
// the page. Pure: no signals, no door.

import { formatLongDate, parseTypedDate } from "../../lib/dates";

/** What a refusal sentence is built from. `item` is the printed item number ("4") or null. */
export interface RefusalContext {
  /** The host's raw refusal or validation text. */
  raw: string;
  /** The draft key the message is about. */
  key: string;
  /** The printed item number ("4", "12A"), or null when the box has none. */
  item: string | null;
  /** The box's label, without trailing format hints. */
  label: string;
  /** The box's kind (money, int, date, sel, text, ...). */
  kind: string;
  /** The form code ("0605"). */
  form: string;
  /** A split date's part name ("month", "day", "year") when the box is one. */
  part?: string;
}

/** "Item 4" or the box's label, as a sentence starts. */
function subject(ctx: RefusalContext): string {
  return ctx.item ? `Item ${ctx.item}` : ctx.label;
}

/** The printed item number with leading zeros dropped ("04" -> "4"); null when the item prints none. */
export function refusalItem(no: string | undefined): string | null {
  const n = String(no ?? "").trim().replace(/^0+/, "");
  return n || null;
}

/** What a sentence calls the box when it has no item number: the field's label, else the row's, else "This box". A draft key never reaches the page. */
export function refusalLabel(field: string | undefined, row: string | undefined, key: string): string {
  const l = String(field || row || "").replace(/\s*\([^)]*\)\s*$/, "").trim();
  return l && !l.includes(key) ? l : "This box";
}

/** A refusal retyping cannot fix: the box is shut by another answer, or locked/computed. */
export function unfixableRefusal(raw: string): boolean {
  const r = String(raw ?? "").trim();
  return (
    /^needs\b/i.test(r) ||
    /^item\s+\S+\s+is checked\.?$/i.test(r) ||
    /comes from the (taxpayer )?profile/i.test(r) ||
    /computed by BIR|is computed by the form/i.test(r) ||
    /reopen the form for another TIN|set by the return's period|the return's period/i.test(r) ||
    /cannot be filled (here|from here)/i.test(r)
  );
}

/** "Item 4: ..." / "TIN: ..." — the colon form for typed-answer refusals. */
function colon(ctx: RefusalContext): string {
  return `${subject(ctx)}:`;
}

/** The gate tag the host/off map gives ("Needs item 4 = Yes") as the used-when sentence. */
function gateSentence(ctx: RefusalContext, raw: string): string {
  const at = subject(ctx);
  // "Needs item 4 = Yes" -> "Item 14 is used when Item 4 is Yes."
  const needs = /^needs\s+(.+)$/i.exec(raw.trim());
  const rest = (needs ? needs[1] : raw).trim().replace(/\.+$/, "");
  const eq = /^(.*?)\s*=\s*(.+)$/.exec(rest);
  if (eq) {
    const what = eq[1].trim();
    const answer = eq[2].trim();
    const parent = /^item\s+(.+)$/i.exec(what);
    const when = parent ? `Item ${parent[1].trim()}` : what;
    return `${at} is used when ${when} is ${answer}.`;
  }
  return `${at} is used when ${rest.charAt(0).toLowerCase()}${rest.slice(1)}.`;
}

/**
 * The plain sentence for one host refusal. `raw` is matched against the kinds the host
 * answers; anything unmatched gets the fallback, so a key, path or backtick can never leak.
 */
export function refusalSentence(ctx: RefusalContext): string {
  const raw = String(ctx.raw ?? "").trim();
  const at = colon(ctx);

  if (/two-digit date year/i.test(raw)) return `${at} type the four-digit year, for example 08/17/1988.`;
  if (/two-digit period year/i.test(raw)) return `${at} type the four-digit year, for example 12/2023.`;
  if (/month and year as MM\/YYYY/i.test(raw)) return `${at} type the month and year as MM/YYYY, for example 12/2023.`;
  if (/end date is before the start date|end before start/i.test(raw)) return `${at} the end date is before the start date.`;
  const span = /^date outside allowed span (\d{4}-\d{2}-\d{2}) (\d{4}-\d{2}-\d{2})$/.exec(raw);
  if (span) {
    const min = parseTypedDate(span[1]);
    const max = parseTypedDate(span[2]);
    if (min && max) return `${at} choose a date from ${formatLongDate(min)} to ${formatLongDate(max)}.`;
  }
  if (raw === "date conflicts with a locked part") return `${at} use the date set by the return.`;
  if (raw === "calendar year ends in December") return "A calendar year ends in December. Choose Fiscal to pick another month.";

  // A gate: the box is shut by another answer ("Needs item 4 = Yes", the off tag).
  if (/^needs\b/i.test(raw) || /^item\s+\S+\s+is checked\.?$/i.test(raw)) return gateSentence(ctx, raw);

  // A locked box (the host's fill refusals name the lock's reason).
  if (/comes from the (taxpayer )?profile/i.test(raw)) return `${subject(ctx)} comes from the tax profile.`;
  if (/computed by BIR|is computed by the form/i.test(raw)) return `${subject(ctx)} is computed.`;
  if (/reopen the form for another TIN|set by the return's period|the return's period/i.test(raw)) {
    return `${subject(ctx)} is set by the return's period.`;
  }

  // A pick off the box's list (the official-list writers, B2/B17; the
  // binding's own "must be one of ..." answers the same way).
  if (/pick an ATC|pick a tax type|not one of the listed|not in the box's list|must be one of|is not an Item \d+A choice|Choose an ATC/i.test(raw)) {
    return `${at} pick one of the listed choices.`;
  }

  // A whole number.
  if (/must be a whole number|whole number/i.test(raw)) return `${at} type a whole number, for example 3.`;

  // An amount.
  if (/must be an amount|an amount such as/i.test(raw)) return `${at} type an amount, for example 1234.50.`;

  // A date, as typed.
  if (/must be a date as|valid date as|date as MM\/DD\/YYYY/i.test(raw)) {
    return `${at} type the date as MM/DD/YYYY, for example 08/17/2023 or 08172023.`;
  }
  // An impossible complete date is judged before the part ranges (bir-core's "Date is not a real calendar date").
  if (/not a real (calendar )?date|no such date|there is no such|impossible date|date does not exist/i.test(raw)) {
    return `${at} there is no such date. Check the day.`;
  }
  // A date part out of range: the raw must say it is a range problem; the part only picks which sentence.
  const named =
    /month \(01-12\)|month must be 01-12|the month of a date|the month is 1 to 12/i.test(raw) ? "month"
    : /day \(01-31\)|day must be 01-31|the day of a date|the day is 1 to 31/i.test(raw) ? "day"
    : /four-digit year|year must be a four-digit|the year is four digits/i.test(raw) ? "year"
    : null;
  const rangeSays = /\b(range|between|out of)\b|01-12|01-31|1 to 12|1 to 31|four-digit/i.test(raw);
  const part =
    named ??
    (rangeSays && (ctx.part === "month" || ctx.part === "day" || ctx.part === "year") ? ctx.part : null);
  if (part === "month") return `${at} the month is 1 to 12.`;
  if (part === "day") return `${at} the day is 1 to 31.`;
  if (part === "year") return `${at} type the four-digit year.`;

  // Rows and paths the draft cannot take.
  if (/draft has no|cannot be added here|past the last row|could not be saved/i.test(raw)) {
    return `${at} this answer could not be saved here.`;
  }

  // A value the box's type cannot hold ("= `-5` does not fit this box: … expected u16").
  if (/does not fit this box/i.test(raw)) {
    if (ctx.kind === "int") return `${at} type a whole number, for example 3.`;
    if (ctx.kind === "money" || ctx.kind === "pct") return `${at} type an amount, for example 1234.50.`;
  }

  // The host's own plain sentence ("Enter the month as MM/YYYY.", "Selecting an ATC is not necessary when item no. 4
  // is set to ' NO '") says more than any fallback: it shows as it is once the box's key is taken off its front,
  // unless what is left still names a key or a path.
  const own = ownSentence(ctx, raw);
  if (own) return own;

  // The safety net: anything the table does not name still never leaks a key or a path.
  return `${at} that was not accepted. Check it and type it again.`;
}

/** The host's sentence without the key it starts with ("`k`: …", "`k` is …"), or "" when it would leak or says nothing. */
function ownSentence(ctx: RefusalContext, raw: string): string {
  const keyed = /^`[^`]*`\s*(:|is\b)?\s*/.exec(raw);
  const rest = (keyed ? raw.slice(keyed[0].length) : raw).trim();
  if (rest.length < 8 || !/\s/.test(rest) || LEAK.test(rest)) return "";
  const said = /[.!?]$/.test(rest) ? rest : `${rest}.`;
  if (!keyed) return `${said.charAt(0).toUpperCase()}${said.slice(1)}`;
  return keyed[1] === "is" ? `${subject(ctx)} is ${said}` : `${colon(ctx)} ${said.charAt(0).toLowerCase()}${said.slice(1)}`;
}

/** The four 0605 host-validation sentences the picked mockup rewrites, keyed by the error's field. */
const FORM_0605_ERRORS: Record<string, string> = {
  due_date: "Item 4 needs a due date.",
  return_period: "Item 7 needs a return period.",
  atc: "Item 6: pick the ATC, for example FP010.",
  tax_type: "Item 8: pick the tax type code.",
};

/** What still must never reach the page: a key, a JSON path, a host prefix. A path is a slash before a lowercase name ("/due_date/month"), so official "MM/DD/YYYY" and "and/or" pass. */
const LEAK = /`|frm\w+:|(^|[\s"'(`=])\/[a-z_][a-z0-9_]*|draft has no|Other error:|\.rs\b|\{\s*"/;

/**
 * A host validation message as the page shows it: the official program's alerts stay as
 * they are, except 0605's four rewritten fields and anything that still leaks a key or a
 * path, which becomes the fallback sentence for its box.
 */
export function hostSentence(ctx: RefusalContext, field?: string): string {
  const raw = String(ctx.raw ?? "").trim();
  if (ctx.form === "0605" && field && FORM_0605_ERRORS[field]) return FORM_0605_ERRORS[field];
  // A validation message that still leaks a key or a path falls back to the plain needs-a-valid-answer sentence.
  if (LEAK.test(raw)) return `${subject(ctx)} needs a valid answer.`;
  return raw;
}

/** `plainRefusal`'s strip, kept here so schema.ts routes through this module. */
export function stripOtherError(text: string): string {
  return String(text).replace(/^Other error:\s*/, "");
}
