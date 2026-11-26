// The taxpayer screens' pure rules (the old shell's taxpayer.js): what a typed value is worth, what each sentence says.
// No UI and no host calls; the clock comes in as a parameter where a rule needs today.
import type { BranchFieldName, FormTemplateSummary, InboxState, SendGrant, SetupApplied, Taxpayer } from "../../bir";
import { DATE_ERROR, formatIso, formatMDY, isTwoDigitDate, parseTypedDate } from "../../lib/dates";
import { FACT_LABELS, joinNames, type FactName } from "../../lib/facts";
import { isOpenDeadline } from "../../lib/statusWord";

/** The taxpayer types the Taxpayer type dropdown offers. */
export const TAXPAYER_TYPES: readonly Taxpayer["taxpayer_type"][] = ["Individual", "Corporation", "Partnership", "Cooperative", "Estate", "Trust"];
/** The Forms tab's frequency filter. */
export const FREQS = ["All", "Monthly", "Quarterly", "Annual", "As needed"] as const;
/** The address and contact fields: field, label, full width. */
export const ADDRESS_FIELDS: readonly [Exclude<BranchFieldName, "rdo_code">, string, boolean][] = [
  ["registered_address", "Registered address", true],
  ["zip_code", "ZIP code", false],
  ["phone", "Phone", false],
  ["email", "Email", true],
];
/** Every branch field the New branch page sets, in order, with its label. */
export const BRANCH_FIELDS: readonly [BranchFieldName, string][] = [
  ["rdo_code", "RDO"],
  ["registered_address", "Registered address"],
  ["zip_code", "ZIP code"],
  ["phone", "Phone"],
  ["email", "Email"],
];
/** The profile's seven sections, in order. */
export const SECTIONS = ["taxpayer", "branch", "taxtypes", "facts", "address", "inbox", "agent"] as const;
export type SectionId = (typeof SECTIONS)[number];
/** The sections with an Edit. */
export type EditableSection = "taxpayer" | "branch" | "address";

export const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
export const TIN_ERROR = "TIN is 9 digits, like 123-456-789.";
export const BRANCH_CODE_ERROR = "Branch code is 5 digits, like 00001.";

/** Today as "YYYY-MM-DD" in local time. */
export function todayText(now: Date = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

/** "YYYY-MM-DD" that names a real day. */
export function realDate(text: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(text)) return false;
  const [y, m, d] = text.split("-").map(Number);
  const day = new Date(y, m - 1, d);
  return day.getFullYear() === y && day.getMonth() === m - 1 && day.getDate() === d;
}

/** What is wrong with a typed date that must not be after today, or null. Empty is fine. */
export function pastDateError(text: string, now: Date = new Date()): string | null {
  if (!text) return null;
  if (!realDate(text)) return isTwoDigitDate(text) ? TWO_DIGIT_YEAR : DATE_ERROR;
  if (text > todayText(now)) return "That date is in the future.";
  return null;
}

/** The sentence of the form's date boxes for a year written with two digits (ui/dateSheet.tsx says the same). */
const TWO_DIGIT_YEAR = "Type the four-digit year, for example 08/17/1988.";

// The profile keeps its dates as ISO (YYYY-MM-DD: the host's birth_date and business_start_date), the date boxes speak MM/DD/YYYY as
// the form page's do. The profile's date box (ProfileTab.tsx EditDate) converts at its edge with these two, and nowhere else.

/** The text a profile date box shows for the stored value: MM/DD/YYYY for an ISO day, anything else (a part-typed date) as it is. */
export function profileDateShown(stored: string): string {
  return realDate(stored) ? formatMDY(parseTypedDate(stored)!) : stored;
}

/** What a profile date stores for the box's text: the ISO day when the text reads as a date, else the text as typed (refused on Save). */
export function profileDateStored(typed: string): string {
  const day = parseTypedDate(typed);
  return day ? formatIso(day) : typed;
}

/** A date box's refusal of a day after today: "A birth date can't be in the future. The box kept 05/17/1990." */
export function futureDateNote(noun: string, kept: string): string {
  const said = realDate(kept) ? `The box kept ${profileDateShown(kept)}.` : "The box stays empty.";
  return `${noun} can't be in the future. ${said}`;
}

/** What the phone's date sheet says before it takes a date ("" when it takes it): the form's sentences, and no day after today. */
export function profileDateProblem(text: string, noun: string, now: Date = new Date()): string {
  const t = text.trim();
  if (!t) return "";
  const day = parseTypedDate(t);
  if (!day) return isTwoDigitDate(t) ? TWO_DIGIT_YEAR : DATE_ERROR;
  return formatIso(day) > todayText(now) ? `${noun} can't be in the future.` : "";
}

/** ATC codes as typed ("wc160, WI010"): the codes, upper case, or null when one is not a code. */
export function parseAtcCodes(text: string): string[] | null {
  const codes = text
    .split(/[\s,;]+/)
    .map((c) => c.trim().toUpperCase())
    .filter(Boolean);
  return codes.every((c) => /^[A-Z]{2}\d{3}[A-Z0-9]*$/.test(c)) ? codes : null;
}

/** "is" or "are" for a count of names. */
export const isAre = (n: number) => (n === 1 ? "is" : "are");
/** "1 form", "3 forms". */
export const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

/**
 * What is wrong with an RDO, ZIP code or email as typed, or null. Empty is fine: none is required. `knownRdo` says whether
 * an RDO is one of the reference list's.
 */
export function factError(field: string, raw: string, knownRdo?: (code: string) => boolean): string | null {
  const v = raw.trim();
  if (!v) return null;
  if (field === "rdo_code" && knownRdo && !knownRdo(v)) return "Pick an RDO from the list. Type its code or district to search.";
  if (field === "zip_code" && !/^\d{4}$/.test(v)) return "ZIP code is 4 digits, like 1226. Type a code or a place to search.";
  if (field === "email" && !EMAIL_RE.test(v)) return "Type an email like name@example.com.";
  return null;
}

/** The errors to show now: a field's once it was left (touched), every one after the first submit, and the host's. */
export function shownErrors(problems: Record<string, string>, touched: readonly string[], submitted: boolean, host: Record<string, string> = {}): Record<string, string> {
  const out: Record<string, string> = { ...host };
  for (const [k, v] of Object.entries(problems)) if (submitted || touched.includes(k)) out[k] = out[k] || v;
  return out;
}

// ---------------------------------------------------------------- new taxpayer

/** What the New taxpayer page holds while it is filled in. */
export interface NewTaxpayerDraft {
  tin: string;
  name: string;
  type: Taxpayer["taxpayer_type"];
  registered_address: string;
  zip_code: string;
  rdo_code: string;
  phone: string;
  email: string;
  /** "none", "setup:<id>" or "copy" (with `from`, a branch TIN). */
  start: string;
  from: string | null;
  touched: string[];
  submitted: boolean;
  /** Errors the host answered on Create, by field. */
  hostErrors: Record<string, string>;
  /** A host error that names no field. */
  error: string | null;
}

export function emptyNewTaxpayer(): NewTaxpayerDraft {
  return {
    tin: "", name: "", type: "Individual", registered_address: "", zip_code: "", rdo_code: "", phone: "", email: "",
    start: "none", from: null, touched: [], submitted: false, hostErrors: {}, error: null,
  };
}

/** Every problem with the new taxpayer as typed, by field. */
export function newTaxpayerProblems(n: NewTaxpayerDraft, taxpayers: readonly Taxpayer[], knownRdo: (code: string) => boolean): Record<string, string> {
  const e: Record<string, string> = {};
  if (n.tin.length !== 9) e.tin = TIN_ERROR;
  else {
    const taken = taxpayers.find((t) => t.tin_root === n.tin);
    if (taken) e.tin = `TIN ${formatRoot(n.tin)} is already ${taken.name}.`;
  }
  if (!n.name.trim()) e.name = "Type the registered name.";
  if (n.start === "copy" && !n.from) e.from = "Choose the taxpayer and branch to copy.";
  for (const f of ["zip_code", "rdo_code", "email"] as const) {
    const error = factError(f, n[f], knownRdo);
    if (error) e[f] = error;
  }
  return e;
}

/** The errors the New taxpayer page shows now; a TIN already in use is said as soon as all 9 digits are in. */
export function newTaxpayerErrors(n: NewTaxpayerDraft, problems: Record<string, string>): Record<string, string> {
  const touched = n.tin.length === 9 ? [...n.touched, "tin"] : n.touched;
  return shownErrors(problems, touched, n.submitted, n.hostErrors);
}

/** "000-123-453". */
export function formatRoot(digits: string): string {
  return digits.replace(/^(\d{3})(\d{3})(\d{3})$/, "$1-$2-$3");
}

/** The tax year a new taxpayer starts in: this year, or the copied branch's latest when it lacks this year. */
export function startYear(start: string, fromYears: readonly number[] | null, now: Date = new Date()): number {
  const y = now.getFullYear();
  if (start !== "copy" || !fromYears) return y;
  return fromYears.includes(y) || !fromYears.length ? y : fromYears[fromYears.length - 1];
}

/** The line under "Start from". */
export function startHint(chosen: { adds_forms: boolean; forms: string[] } | null, start: string, year: number): string {
  if (chosen) return `${chosen.adds_forms ? "Adds" : "Starts"} ${year} with ${chosen.forms.join(", ")} and the facts they use.`;
  if (start === "copy") return `Copies the forms, facts and templates of ${year}. Never the name, TIN or address.`;
  return "Add a tax year after, and choose its forms.";
}

// ---------------------------------------------------------------- setups and copies

/** The question after a setup or copy left set things as they were. */
export interface ReplaceAsk {
  kind: "setup" | "copy";
  tin: string;
  year: number;
  id?: string;
  from?: string;
  label: string;
  kept: FactName[];
  templates: string[];
  forms: string[];
}

/** What a setup or copy did: the toast, and the question to keep (null when nothing set was left). */
export function afterApply(r: SetupApplied, ask: Omit<ReplaceAsk, "kept" | "templates" | "forms">): { toast: string; ask: ReplaceAsk | null } {
  const kept = (r.kept || []).filter((f) => !(f === "income_tax_election" && r.election_locked)) as FactName[];
  const templates = r.templates_kept || [];
  const forms = r.forms_kept || [];
  const n = (r.forms || []).length;
  let text = `${ask.label}: ${plural(n, "form")} for ${ask.year}.`;
  if (r.election_locked && (r.kept || []).includes("income_tax_election")) {
    text += ` The 8% or graduated choice for ${ask.year} stays: a return that carries it was filed.`;
  }
  return { toast: text, ask: kept.length || templates.length || forms.length ? { ...ask, kept, templates, forms } : null };
}

/** The banner's sentence: what is already set, what stays on the list, then the question. */
export function replaceSentence(a: ReplaceAsk): string {
  const names: string[] = a.kept.map((f) => FACT_LABELS[f]);
  if (a.templates.length) names.push(`the ${joinNames(a.templates)} template${a.templates.length === 1 ? "" : "s"}`);
  const lines: string[] = [];
  if (names.length) lines.push(`${joinNames(names)} ${isAre(names.length)} already set. ${a.label} has something else.`);
  if (a.forms.length) {
    lines.push(`${joinNames(a.forms)} ${isAre(a.forms.length)} on your forms list and not in ${a.label}; ${a.forms.length === 1 ? "it stays" : "they stay"} unless you replace.`);
  }
  return `${lines.join(" ")} Replace what is set?`;
}

/** Two or more annual income tax returns of one kind on a year's list: the warning. */
export function annualConflictSentence(group: readonly string[], year: number): string {
  const both = group.length === 2 ? "both" : "all";
  const other = group.length === 2 ? "other" : "others";
  return `${joinNames(group)} are ${both} annual income tax returns. A taxpayer files only one of them for ${year}. Keep the one on your Certificate of Registration and remove the ${other}.`;
}

// ---------------------------------------------------------------- tax years and forms

/** The years that may be added, newest first. */
export function allowedYears(bounds: { min: number; max: number }, have: readonly number[]): number[] {
  const out: number[] = [];
  for (let y = bounds.max; y >= bounds.min; y--) if (!have.includes(y)) out.push(y);
  return out;
}

/** The year the Add year band offers first: the one after the latest, else this year, else the newest allowed. */
export function defaultAddYear(allowed: readonly number[], have: readonly number[], now: Date = new Date()): number | null {
  if (!allowed.length) return null;
  const last = have[have.length - 1];
  const y = now.getFullYear();
  if (last) return allowed.includes(last + 1) ? last + 1 : allowed[0];
  return allowed.includes(y) ? y : allowed[0];
}

/** A deadline as the Forms tab reads it. */
export interface DeadlineLike {
  status: string;
  /** Its Submitted return was only a dry run: the period is still open. */
  submitted_dry_run?: boolean | null;
  due_date: string;
  period_label: string;
}

/** The next open period of a form in the year and its due date, from its deadlines (`daysUntil` given). */
export function schedule(rows: readonly DeadlineLike[], daysUntil: (iso: string) => number, shortDate: (iso: string) => string): { next: string | null; due: string; none?: boolean } {
  if (!rows.length) return { next: null, due: "", none: true };
  // Still to do: no return, a Draft, or a dry run (it sent nothing).
  const open = rows.filter(isOpenDeadline);
  const upcoming = open.find((d) => daysUntil(d.due_date) >= 0) || open[open.length - 1];
  if (!upcoming) return { next: null, due: "" };
  return { next: upcoming.period_label, due: shortDate(upcoming.due_date) };
}

/** The line under a form's name on the Forms tab. */
export function formSub(on: boolean, editing: boolean, sc: { next: string | null; none?: boolean }, year: number): string {
  if (editing && !on) return "Not filed this year";
  if (sc.next) return `Next: ${sc.next}`;
  return sc.none ? `Filed in ${year}` : "Every period filed";
}

/** The form library's search text of a form: code, code without dashes, title, and "vat" for a value-added tax form. */
export function searchText(f: { c: string; t: string }): string {
  const vat = /Value-Added/i.test(f.t) ? " vat" : "";
  return `${f.c} ${f.c.replace(/-/g, "")} ${f.t}${vat}`.toLowerCase();
}

/** The toast after Save forms. */
export function formsSavedText(year: number, before: readonly string[], after: readonly string[]): string {
  const added = after.filter((c) => !before.includes(c)).length;
  const removed = before.filter((c) => !after.includes(c)).length;
  return `${year} forms updated: ${added} added, ${removed} removed.`;
}

/** The toast after Add year. */
export function yearAddedText(year: number, r: { copied_from: number | null; forms: string[] }): string {
  const n = (r.forms || []).length;
  return r.copied_from ? `${year} added with ${plural(n, "form")} from ${r.copied_from}.` : `${year} added. Choose the forms it files.`;
}

// ---------------------------------------------------------------- templates

/** "3 answers · 2 stops": what a template fixes and how many "Stop here" switches are on. */
export function templateCounts(tpl: Pick<FormTemplateSummary, "answers_kept" | "marked">): string {
  const answers = tpl.answers_kept ?? 0;
  const stops = tpl.marked ?? 0;
  return `${answers === 1 ? "1 answer" : `${answers} answers`} · ${stops === 1 ? "1 stop" : `${stops} stops`}`;
}

/** A failed "template from the newest return", in words. */
export function templateErrorText(code: string, text: string): string {
  return /no return|not ?found|no .*draft|first/i.test(text) ? `Open and save a ${code} return first. The template starts from its answers.` : text;
}

// ---------------------------------------------------------------- section metas

/** The Confirmation inbox section's summary. */
export function inboxMeta(ib: InboxState | null): string {
  if (!ib) return "Not connected";
  if (ib.tracking_enabled) return `Tracking on · ${ib.inbox_email}`;
  if (ib.oauth_connected || ib.has_app_password) return `Connected · ${ib.inbox_email || ib.oauth_inbox_email || ""}`;
  return "Not connected";
}

/** The Agent filing section's summary. */
export function agentMeta(grants: readonly SendGrant[]): string {
  return grants.length ? grants.map((g) => g.form).join(", ") : "Assistant must ask before filing";
}

/** The Address and contact section's summary. */
export function addressMeta(isHead: boolean, inheritedCount: number): string {
  if (isHead) return "Used on every return";
  return inheritedCount ? `${inheritedCount} of 4 from head office` : "All set for this branch";
}

/** The Facts section's summary. */
export function factsMeta(hasYear: boolean, shown: number, notSet: number): string {
  if (!hasYear) return "Add a tax year first";
  if (!shown) return "No form on your list reads one";
  return notSet ? `${notSet} not set` : "All set";
}

/** The line under the facts: how many facts no form on the list reads. */
export function hiddenFactsText(hidden: number): string | null {
  if (!hidden) return null;
  return hidden === 1 ? "1 fact is not shown: no form on your list uses it." : `${hidden} facts are not shown: no form on your list uses them.`;
}

/** The result of a "check now", in words. */
export function checkText(r: { ok: boolean; still_pending: boolean; error: string | null }): string {
  if (!r.ok) return r.error || "The check failed.";
  return r.still_pending ? "Checked. A submitted return is still waiting for its confirmation." : "Checked. Everything submitted is confirmed.";
}

/** The tracking line at the foot of the inbox section. */
/** A build with no Google client id offers the app password only, and says so in one line. */
export const GOOGLE_UNAVAILABLE = "Google sign-in is not available in this version. Use an app password.";

export function trackingLine(s: Pick<InboxState, "tracking_enabled" | "still_pending">): string {
  if (!s.tracking_enabled) return "○ Verify your connection to activate email tracking.";
  return s.still_pending
    ? "● Automated BIR receipt tracking is active — a submitted return is still waiting for its confirmation."
    : "● Automated BIR receipt tracking is active.";
}
