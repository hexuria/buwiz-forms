// The controls' pure rules: which options a list has, what a shown value is, how a TIN box is told, how a search matches.
// No UI here, so it is tested on its own (model.test.ts). The rules are the old page's (the old shell's form_page.js: optionOf,
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

/** How a one-answer group of `count` choices draws: a segmented bar (2–4), a dropdown (5–12), a search list (more). */
export function radioKind(count: number): "segmented" | "dropdown" | "search" {
  return count <= 4 ? "segmented" : count <= SEARCH_FROM ? "dropdown" : "search";
}

/** A tick box reads as ticked. */
export const isOn = (value: string): boolean => value === "true" || value === "1" || value === "Y";

/** A box named as part of a TIN: a segment (TIN1, TIN segment 1, tinA), the branch code, or the whole TIN. */
function tinNamed(f: Pick<SchemaField, "k" | "l">): boolean {
  const key = String(f.k).split(":").pop() ?? "";
  return (
    /(^|[a-z0-9_])(TIN|Tin|tin)(?![a-z])/.test(key) || /(^|[^A-Za-z])TIN(?![a-z])/.test(f.l) || /\bTI nof\b/i.test(f.l) ||
    /branch ?code/i.test(f.l) || /BranchCode/i.test(key)
  );
}

/** What a text box that holds a TIN is: its whole 14 digits ("full"), one of the 3-3-3-5 boxes ("part"), or no TIN at all. */
export function tinKind(f: Pick<SchemaField, "k" | "l" | "t" | "m">): "full" | "part" | null {
  if (f.t !== "text" && f.t !== "int") return null;
  if (!tinNamed(f) || !f.m) return null;
  if (f.t === "text" && f.m >= 14 && f.m <= 17) return "full";
  if (f.m <= 5) return "part";
  return null;
}

/**
 * A box of a TIN item that takes digits: a segment, the branch code or the whole TIN, named like tinKind but with no
 * max length (several forms leave `m` off their TIN boxes). The phone grid's cells take digits only through it.
 */
export function tinSegment(f: Pick<SchemaField, "k" | "l" | "t">): boolean {
  return (f.t === "text" || f.t === "int") && tinNamed(f);
}

/**
 * A read-only box's side padding in points: 10 (`px-2.5`), but 4 (`px-1`) in a cell of the phone TIN grid, whose
 * 47 pt column on a 320 pt phone has no room for three digits between 10 pt sides.
 */
export function readBoxPad(tinCell: boolean): number {
  return tinCell ? 4 : 10;
}

/**
 * The room for digits in each cell of the phone TIN grid: the grid's width shared by `weights` over 6 pt gaps, less a
 * 1 pt border and `pad` on each side.
 */
export function tinCellRoom(gridWidth: number, weights: readonly number[], pad: number): number[] {
  const total = weights.reduce((a, b) => a + b, 0);
  const share = (gridWidth - 6 * (weights.length - 1)) / total;
  return weights.map((w) => share * w - 2 - 2 * pad);
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

/**
 * The phone row's head (under 600 pt): 6 pt between the number, the label and what follows, and a label that keeps at
 * least 160 pt of the line — a button beside it ("Edit", "Edit penalties") that leaves it less goes to the next line
 * instead of squeezing the label into a column of single words. Wide rows keep 8 pt and no minimum.
 */
export function rowHead(narrow: boolean): { gap: number; labelMin: number | undefined } {
  return narrow ? { gap: 6, labelMin: 160 } : { gap: 8, labelMin: undefined };
}

/** Whether a head of `width` points still has room beside the number (`no` wide) and the label's minimum for `extra`. */
export function headFits(width: number, no: number, extra: number, narrow = true): boolean {
  const head = rowHead(narrow);
  return no + head.gap + (head.labelMin ?? 0) + head.gap + extra <= width;
}

/** A schedule row's columns as the drawer stacks them: a run of two to five TIN digit boxes is one group (one grid line). */
export function drawerGroups<C extends { col: { l: string; t: string } }>(columns: readonly C[], keyOf: (c: C) => string): C[][] {
  const out: C[][] = [];
  let run: C[] = [];
  const close = () => {
    if (run.length >= 2 && run.length <= 5) out.push(run);
    else for (const c of run) out.push([c]);
    run = [];
  };
  for (const c of columns) {
    const digits = (c.col.t === "text" || c.col.t === "int") && tinSegment({ k: keyOf(c), l: c.col.l, t: c.col.t });
    // A whole-TIN column ("TIN (Including Branch Code)") is one box of its own, never a cell of a grid.
    if (digits && !/including/i.test(c.col.l)) run.push(c);
    else {
      close();
      out.push([c]);
    }
  }
  close();
  return out;
}

/** A drawer TIN group's weights and title: the branch code's cell is 1.4, the title is the first column's words without "(1-3)". */
export function drawerTin(labels: readonly string[]): { weights: number[]; title: string } {
  return {
    weights: labels.map((l, i) => (i === labels.length - 1 && /branch/i.test(l) ? 1.4 : 1)),
    title: (labels[0] ?? "").replace(/\s*\(\d-\d\)\s*$/, ""),
  };
}
