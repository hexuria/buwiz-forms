// The one sentence table for what the host refuses (B3/B4): every refusal kind the host can answer
// becomes a plain sentence that begins with the item number when the box has one, else the box
// label. The raw host text (keys, JSON paths, backticks) is for the log only; it never reaches
// the page. Pure: no signals, no door.

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

  // A gate: the box is shut by another answer ("Needs item 4 = Yes", the off tag).
  if (/^needs\b/i.test(raw) || /^item\s+\S+\s+is checked\.?$/i.test(raw)) return gateSentence(ctx, raw);

  // A locked box (the host's fill refusals name the lock's reason).
  if (/comes from the (taxpayer )?profile/i.test(raw)) return `${subject(ctx)} comes from the tax profile.`;
  if (/computed by BIR|is computed by the form/i.test(raw)) return `${subject(ctx)} is computed.`;
  if (/reopen the form for another TIN|set by the return's period|the return's period/i.test(raw)) {
    return `${subject(ctx)} is set by the return's period.`;
  }

  // A pick off the box's list (the official-list writers, B2/B17).
  if (/pick an ATC|pick a tax type|not one of the listed|not in the box's list|is not an Item \d+A choice|Choose an ATC/i.test(raw)) {
    return `${at} pick one of the listed choices.`;
  }

  // A whole number.
  if (/must be a whole number|whole number/i.test(raw)) return `${at} type a whole number, for example 3.`;

  // An amount.
  if (/must be an amount|an amount such as/i.test(raw)) return `${at} type an amount, for example 1234.50.`;

  // A date, as typed or as a part out of range.
  if (/must be a date as|valid date as|date as MM\/DD\/YYYY/i.test(raw)) {
    return `${at} type the date as MM/DD/YYYY, for example 08/17/2023 or 08172023.`;
  }
  if (ctx.part === "month" || /month \(01-12\)|month must be 01-12|the month of a date/i.test(raw)) {
    return `${at} the month is 1 to 12.`;
  }
  if (ctx.part === "day" || /day \(01-31\)|day must be 01-31|the day of a date/i.test(raw)) {
    return `${at} the day is 1 to 31.`;
  }
  if (ctx.part === "year" || /four-digit year|year must be a four-digit/i.test(raw)) {
    return `${at} type the four-digit year.`;
  }
  if (/not a real date|no such date|there is no such|impossible date|date does not exist/i.test(raw)) {
    return `${at} there is no such date. Check the day.`;
  }

  // Rows and paths the draft cannot take.
  if (/draft has no|cannot be added here|past the last row|could not be saved/i.test(raw)) {
    return `${at} this answer could not be saved here.`;
  }

  // The safety net: anything the table does not name still never leaks a key or a path.
  return `${at} that was not accepted. Check it and type it again.`;
}

/** The four 0605 host-validation sentences the picked mockup rewrites, keyed by the error's field. */
const FORM_0605_ERRORS: Record<string, string> = {
  due_date: "Item 4 needs a due date.",
  return_period: "Item 7 needs a return period.",
  atc: "Item 6: pick the ATC, for example FP010.",
  tax_type: "Item 8: pick the tax type code.",
};

/** What still must never reach the page: a key, a JSON path, a host prefix. */
const LEAK = /`|frm\w+:|\/\w|draft has no|Other error:|\.rs\b|\{\s*"/;

/**
 * A host validation message as the page shows it: the official program's alerts stay as
 * they are, except 0605's four rewritten fields and anything that still leaks a key or a
 * path, which becomes the fallback sentence for its box.
 */
export function hostSentence(ctx: RefusalContext, field?: string): string {
  const raw = String(ctx.raw ?? "").trim();
  if (ctx.form === "0605" && field && FORM_0605_ERRORS[field]) return FORM_0605_ERRORS[field];
  if (LEAK.test(raw)) return refusalSentence(ctx);
  return raw;
}

/** `plainRefusal`'s strip, kept here so schema.ts routes through this module. */
export function stripOtherError(text: string): string {
  return String(text).replace(/^Other error:\s*/, "");
}
