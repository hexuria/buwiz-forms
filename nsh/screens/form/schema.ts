// Pure helpers over the form schema, ported from app/shell/form_page.js (no door, no signals).
import type { ChoiceOption, FieldType, FormSchema, SchemaField, SchemaItem, SchemaPart } from "./contract";
import { shortIsoDate } from "../../lib/dates";
import type { ScreenFormExisting } from "../../bir";

/**
 * A listed dropdown option as { value, label }. The schema writes many as "CODE - Label" strings ("03 - March"): the
 * host takes the code, so that is the value and the whole string the label. Template text the extractor left
 * ("\" + rdo.description + \"") is dropped.
 */
export function optionOf(o: unknown): ChoiceOption | null {
  if (o && typeof o === "object") return o as ChoiceOption;
  const text = String(o);
  if (/^"/.test(text)) return null;
  const m = /^(\S+) - (.+)$/.exec(text);
  return m ? { value: m[1], label: text } : { value: text, label: text };
}

/** Items whose three to five text boxes are the TIN (3-3-3-5). */
export function isTinItem(it: SchemaItem): boolean {
  const fs = it.f || [];
  const named = /\bTIN\b|Identification/i.test(it.l) || fs.some((x) => /\bTIN\d?\b|txtTIN/i.test(`${x.l} ${x.k}`));
  return named && fs.length >= 3 && fs.length <= 5 && fs.every((x) => x.t === "text" || x.t === "int");
}

/** Drops hidden fields, tables whose every column is hidden, and items left with no field (unless they carry a note). */
export function withoutHidden(schema: FormSchema): FormSchema {
  const parts = schema.parts
    .map((p) => ({
      ...p,
      items: p.items
        .filter((it) => !it.rows || (it.cols || []).some((c) => c.t !== "hidden"))
        .map((it) => (it.f ? { ...it, f: it.f.filter((f) => (f.t as string) !== "hidden") } : it))
        .filter((it) => it.rows || (it.f && it.f.length) || it.note),
    }))
    .filter((p) => p.items.length);
  return { ...schema, parts };
}

/** The first part's name: "Return details" for an untitled "Page 1". */
export function partName(p: SchemaPart, i: number): string {
  return i === 0 && /^Page \d+$/.test(p.n) ? "Return details" : p.n;
}

/** The navigator's one-line name: filing instructions in parentheses go, "Schedule" becomes "Sch.". */
export function compactPartName(name: string): string {
  const short = name
    .replace(/\s*\((?:attach|if graduated|except)[^)]*\)/gi, "")
    .replace(/^Schedule\b/, "Sch.")
    .trim();
  return short || name;
}

/** A part's place in the pill and its list: tag "Part II", short "Computation of Tax". */
export function sectionNames(p: SchemaPart, i: number): { tag: string; short: string } {
  const name = partName(p, i);
  const strip = (text: string) => text.replace(/\s*\((?:attach|if graduated|except)[^)]*\)/gi, "").trim() || text;
  const at = name.indexOf(" · ");
  if (at < 0) return { tag: name === p.n ? `Section ${i + 1}` : p.n, short: strip(name) };
  return { tag: name.slice(0, at), short: strip(name.slice(at + 3)) };
}

/** The tag on a box another answer turns off, cut if it ever runs long. */
export function needsText(tag: string): string {
  const text = String(tag || "").trim();
  return text.length > 90 ? `${text.slice(0, 89).trimEnd()}…` : text;
}

/** The month, day and year boxes of an item (a printed date split in boxes): at least a month and a year. */
export interface DateParts {
  month: SchemaField;
  day: SchemaField | null;
  year: SchemaField;
}

export function dateParts(fs: SchemaField[]): DateParts | null {
  const box = (f: SchemaField) => f.t !== "radio" && f.t !== "chk";
  const find = (re: RegExp, not?: RegExp, also: (f: SchemaField) => boolean = () => true) =>
    fs.find((f) => box(f) && also(f) && re.test(`${f.l} ${String(f.k).split(":").pop()}`) && !(not && not.test(`${f.l} ${f.k}`)));
  // A box captioned for a day is the day, never the year, whatever else its caption says: 1707A's "Year end day" sits before "Year ended"
  // and was taken for the year (a picked day wrote the year into it, and the calendar opened on year 20DD).
  const namesDay = (f: SchemaField) => /\bday\b/i.test(f.l);
  const month = find(/month|\bmm\b/i, /months|no\.? of/i);
  const year = find(/year|yyyy|\byr\b/i, /month|years ended|no\.? of/i, (f) => !namesDay(f));
  const day = fs.find((f) => box(f) && namesDay(f) && !/days|month/i.test(`${f.l} ${f.k}`)) ?? find(/\bday\b|\bdd\b|[a-z]day\b|day\d*$/i, /days|month|year/i);
  if (!month || !year || month === year) return null;
  return { month, day: day && day !== month && day !== year ? day : null, year };
}

/** An item the host marks one_date: its month, day and year boxes are one date box, keyed "<month key>#date". */
export interface OneDate extends DateParts {
  oneKey: string;
}

export function oneDate(it: SchemaItem): OneDate | null {
  if (!it.one_date) return null;
  const date = dateParts(it.f || []);
  return date ? { ...date, oneKey: `${date.month.k}#date` } : null;
}

/** BIR's own words for the income-tax ATCs the forms list as bare codes. */
const ATC_TITLES: Record<string, string> = {
  II011: "Compensation Income",
  II012: "Business Income – Graduated IT Rates",
  II013: "Mixed Income – Graduated IT Rates",
  II014: "Income from Profession – Graduated IT Rates",
  II015: "Business Income – 8% IT Rate",
  II016: "Mixed Income – 8% IT Rate",
  II017: "Income from Profession – 8% IT Rate",
};

/** A radio's label as the page shows it ("Fiscal start month: 1" shows "1"; "ATC II012" its title). */
export function choiceLabel(f: SchemaField): string {
  const text = f.l.replace(/^.*?:\s*/, "");
  const code = /^A?TC\s+([A-Z]{2}\d{3})$/.exec(text)?.[1];
  return code ? (ATC_TITLES[code] ? `${code}  ${ATC_TITLES[code]}` : code) : text;
}

/** The width a box gets (old controlWidth). */
export function controlWidth(f: { t: FieldType | "hidden"; m?: number }, available: number): number {
  if (f.t === "money") return Math.min(230, available);
  if (f.t === "pct") return 88;
  if (f.t === "date") return 140;
  if (f.t === "int") return f.m && f.m <= 4 ? 88 : 140;
  if (f.t === "sel") return Math.min(260, available);
  if (f.t === "text" && f.m && f.m <= 4) return 88;
  return Math.min(320, available);
}

/**
 * How a read-only return says it was sent: the worker's dry run ("Dry run Nov 18, 2026, not sent to BIR"), the local mock relay
 * (`viaRelay`: the door sends through it, docs/relay.md) or a real send ("Sent to BIR Nov 18, 2026").
 */
export function sentWords(submittedAt: string, dryRun: boolean, viaRelay = false): string {
  const when = shortIsoDate(submittedAt);
  if (dryRun) return `Dry run ${when}, not sent to BIR`;
  return viaRelay ? `Sent to the mock relay ${when}, not to BIR` : `Sent to BIR ${when}`;
}

/** The strip on an amended return's filed version, opened from its history. */
export function filedOriginalStrip(view: { filed_on: string | null }): string {
  const when = view.filed_on ? ` on ${shortIsoDate(view.filed_on)}` : "";
  return `This is the version filed${when}, kept in this return's history; the current return is its amendment. Nothing here can be changed.`;
}

/** Where an amended return's filed version goes. */
export const FILED_KEPT = "The filed version is kept in this return's history.";

/** A filed return whose tax profile changed after it. */
export const PROFILE_CHANGED_NOTE = "The tax profile changed after this return was filed. This return keeps what was filed.";

export function profileUpdatesNote(updates: string[]): string {
  return `Updated from your tax profile: ${updates.join("; ")}`;
}

export function profileDiffersNote(differs: string[]): string {
  return `Differs from your profile: ${differs.join("; ")}`;
}

/** The note at the top of a Form library sample (Save is explicit until its period is set; then it autosaves). */
export const SAMPLE_NOTE =
  "A sample from the Form library. Set its period and it becomes your return for that period, saved as you type; if that period already has a return, you choose what to do with it. Save as return asks for the period. New return (the + button or Cmd/Ctrl+N) starts a blank one.";

/** A sample's period parts in the period dialog, labelled. */
export const PERIOD_PART_LABELS: Record<string, string> = {
  year: "Year (YYYY)",
  month: "Month (MM)",
  year_end_month: "Year-end month (MM)",
  day: "Day (DD)",
  date: "Date (MM/DD/YYYY)",
  quarter: "Quarter",
};

export interface OfferChoice {
  id: "load" | "view" | "amend" | "keep";
  label: string;
  hint: string;
  variant: "primary" | "secondary" | "danger";
}

/** What the existing-return offer says and offers for a stored return's status. */
export function existingOfferFor(existing: ScreenFormExisting): { title: string; subtitle: string; choices: OfferChoice[]; fastLane: boolean } {
  const label = existing.label;
  const has = (action: string) => (existing.actions || []).includes(action as never);
  const keep: OfferChoice = { id: "keep", label: "Keep this form", hint: "Stay in the sample; the period clears.", variant: "secondary" };
  if (existing.status === "Draft") {
    return {
      title: `You already have a ${label} draft`,
      subtitle: existing.updated_at ? `edited ${shortIsoDate(existing.updated_at)}.` : "",
      choices: [{ id: "load", label: "Load saved draft", hint: "Continue where you left off.", variant: "primary" }, keep],
      fastLane: true,
    };
  }
  if (existing.status === "Queued") {
    return {
      title: `${label} is queued to send to BIR`,
      subtitle: "",
      choices: [{ id: "view", label: "View it", hint: "Read-only while it waits to send.", variant: "primary" }, keep],
      fastLane: false,
    };
  }
  const filed = existing.status === "Submitted" ? "submitted" : "filed";
  const choices: OfferChoice[] = [{ id: "view", label: "View read-only", hint: `See exactly what was ${filed}.`, variant: "primary" }];
  if (has("amend")) choices.push({ id: "amend", label: "Amend this return", hint: `New amended return. ${FILED_KEPT}`, variant: "danger" });
  choices.push(keep);
  return { title: `${label} was ${filed}`, subtitle: "", choices, fastLane: false };
}

/** A host refusal in plain words: bir-core's "Other error: " prefix is not for the filer. */
export function plainRefusal(text: string): string {
  return String(text).replace(/^Other error:\s*/, "");
}

/**
 * How many items a host refusal lists: bir-core writes a refused Queue as "field: message; field: message", with the host's own names for
 * the boxes ("taxes_withheld", "tin"). 0 for any other sentence, which is said as it is.
 */
export function listedFields(text: string): number {
  const entry = /^[a-z][a-z0-9_.]*: \S/;
  const parts = String(text).split(/;\s+(?=[a-z][a-z0-9_.]*: )/);
  return parts.every((p) => entry.test(p)) ? parts.length : 0;
}

/**
 * The host's page notices (screen_form_module.rs `evaluate_here`) are listed beside the validation messages but are not validation: a Queue is
 * never refused for one. They sit on these fields (a stored 2553 row's ATC, a 1604E legacy total's penalties, 1606's sale date, 1701's imported
 * spouse RDO, 1702EX's attached rows, 1702RT's MCIT, "one_date": a partial date on an as-needed form).
 */
const PAGE_NOTICES: Record<string, RegExp> = {
  "1604E": /^schedule\d+\[\d+\]\.penalties$/,
  "1606": /^transaction_date$/,
  "1701": /^spouse_rdo_code$/,
  "1702EX": /^(other_credits|other_deductions|special_rows)\.\d+$/,
  "1702RT": /^schedule_4\.rows\[\d+\]\.mcit$/,
  "2553": /^schedule\[\d+\]\.atc_code$/,
};

/** Whether the message the host put on `field` of form `form` is a page notice (never a reason for a refused Queue). */
export function isPageNotice(form: string, field: string): boolean {
  return field === "one_date" || Boolean(PAGE_NOTICES[form]?.test(field));
}

/** Every key an item binds (a schedule's every cell, a pooled line's pool). */
export function keysOf(it: SchemaItem): string[] {
  if (it.rows) return (it.keys ?? []).flat();
  return (it.f ?? []).flatMap((f) => (f.pool ? f.pool.keys : [f.k]));
}

/** An empty schedule cell, as the host reads a row (bir-core's row is_blank): blank text, a zero amount, an unticked box. */
export function emptyCell(value: string, type: string): boolean {
  const v = String(value ?? "").trim();
  if (v === "") return true;
  if (type === "money" || type === "pct" || type === "int") return /^[-+]?0*\.?0*$/.test(v.replace(/[,₱%\s]/g, ""));
  if (type === "chk" || type === "radio") return !(v === "true" || v === "1" || v === "Y");
  return false;
}
