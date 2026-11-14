// The controls' pure rules: which options a list has, what a shown value is, how a TIN box is told, how a search matches.
// No UI here, so it is tested on its own (model.test.ts). The rules are the old page's (app/shell/form_page.js: optionOf,
// selectBox, control, radioGroup, scheduleTable) unless a comment says otherwise.
import type { ChoiceOption, FieldControlProps, SchemaField, SchemaItem } from "../contract";
import { formatMDY, formatMoney, parseTypedDate, shownDate } from "../../../ui";

/** Joins class strings, skipping empty parts. */
export function cx(...parts: (string | false | null | undefined)[]): string {
  return parts.filter(Boolean).join(" ");
}

/** The choice shown first in every list: nothing chosen. */
export const CHOOSE = "Choose…";

/**
 * A listed option as { value, label }. The schema writes many as "CODE - Label" ("03 - March"): the host takes the code,
 * so that is the value and the whole string the label. Template text the extractor left (`"\" + rdo.description + \""`)
 * is dropped (the old optionOf).
 */
export function optionOf(o: string | ChoiceOption | null | undefined): ChoiceOption | null {
  if (o === null || o === undefined) return null;
  if (typeof o !== "string") return o;
  if (o.startsWith('"')) return null;
  const m = /^(\S+) - (.+)$/.exec(o);
  return m ? { value: m[1], label: o } : { value: o, label: o };
}

/** A sel box's listed choices: the host's win, else the schema's own. */
export function listedOptions(f: Pick<SchemaField, "o">, host: ChoiceOption[]): ChoiceOption[] {
  if (host.length) return host;
  return (f.o ?? []).map(optionOf).filter((o): o is ChoiceOption => o !== null);
}

/** The choices a list shows (with the empty one first) and the value it shows as chosen (the old selectBox). */
export function selectModel(listed: ChoiceOption[], value: string): { options: ChoiceOption[]; shown: string } {
  const options: ChoiceOption[] = [{ value: "", label: CHOOSE }, ...listed];
  const selected = listed.find((o) => o.selected);
  const known = options.some((o) => o.value === value);
  // Unknown all-zero codes (0, 00, 000) and a lone "-" are the official runtime's empty choices; a listed 0 or "-" stays selectable.
  const shown = selected ? selected.value : known ? value : /^(0+|-)$/.test(value) ? "" : value;
  if (shown && !selected && !known) options.push({ value: shown, label: shown });
  return { options, shown };
}

/** A list long enough to want a search box. */
export const SEARCH_FROM = 12;

/** A tick box reads as ticked. */
export const isOn = (value: string): boolean => value === "true" || value === "1" || value === "Y";

/** What a text box that holds a TIN is: its whole 14 digits ("full"), one of the 3-3-3-5 boxes ("part"), or no TIN at all. */
export function tinKind(f: Pick<SchemaField, "k" | "l" | "t" | "m">): "full" | "part" | null {
  if (f.t !== "text" && f.t !== "int") return null;
  const key = String(f.k).split(":").pop() ?? "";
  const named =
    /(^|[a-z0-9_])(TIN|Tin|tin)(?![a-z])/.test(key) || /(^|[^A-Za-z])TIN(?![a-z])/.test(f.l) || /\bTI nof\b/i.test(f.l) ||
    /branch ?code/i.test(f.l) || /BranchCode/i.test(key);
  if (!named || !f.m) return null;
  if (f.t === "text" && f.m >= 14 && f.m <= 17) return "full";
  if (f.m <= 5) return "part";
  return null;
}

/** A date text as the date box takes it: MM/DD/YYYY once it reads as a date, else as it is. */
export function mdy(text: string): string {
  const d = parseTypedDate(text);
  return d ? formatMDY(d) : text;
}

/** What a read-only box shows for the box's value: an amount grouped (a blank computed one 0.00), a date long, a choice by its label. */
export function readText(p: Pick<FieldControlProps, "field" | "box">): string {
  const v = p.box.value();
  const f = p.field;
  if (f.t === "money") return formatMoney(v || (p.box.zeroWhenBlank ? "0" : ""), !!f.w);
  if (f.t === "date") return shownDate(v);
  if (f.t === "sel") {
    const { options, shown } = selectModel(listedOptions(f, p.box.options()), v);
    return shown ? (options.find((o) => o.value === shown)?.label ?? shown) : "";
  }
  return v;
}

/** An option's code and its words: "WI010 — Professional fees" -> ["WI010", "Professional fees"]; one run of text -> [text, ""]. */
export function splitLabel(label: string): [string, string] {
  const m = /^(\S+)\s+[—–-]\s+(.+)$/.exec(label);
  return m ? [m[1], m[2]] : [label, ""];
}

/** The options whose label or code holds every typed word, in any order (the kit's ComboField matches one run of text only). */
export function matchWords(options: readonly ChoiceOption[], text: string, limit = Infinity): ChoiceOption[] {
  const words = text.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const out: ChoiceOption[] = [];
  for (const o of options) {
    if (out.length >= limit) break;
    const hay = `${o.value} ${o.label}`.toLowerCase();
    if (words.every((w) => hay.includes(w))) out.push(o);
  }
  return out;
}

/** A schedule row's number as printed, and its preprinted caption ("" when it has none). */
export function rowParts(item: Pick<SchemaItem, "start" | "row_labels">, row: number): { n: number; caption: string } {
  return { n: (item.start ?? 1) + row, caption: item.row_labels?.rows[row] ?? "" };
}

/** The drawer's title of a row: "Row 3", with its caption after a dot where the sheet prints one. */
export function rowTitleOf(item: Pick<SchemaItem, "start" | "row_labels">, row: number): string {
  const { n, caption } = rowParts(item, row);
  return caption ? `Row ${n} · ${caption}` : `Row ${n}`;
}
