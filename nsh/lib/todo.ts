// The form page's to-do list (todo.js, decision 12): the rows the pill's ‹ and › visit, in form order, each once
// with the strongest reason it needs the user. Pure: the page hands in what it knows and gets rows back.
//
// - message: the row holds a validation message.
// - required: a required box (field flag r) is empty, not computed and not shut (blocked(k): locked, turned off by
//   another answer, or a read-only return). A radio group counts as one item.
// - stop: one of the row's boxes is a stop (effectiveStops), not all of its stop boxes hold a value, and the user has
//   not stepped past it on this visit (passed).

/** One box of an item. */
export interface TodoField {
  l?: string;
  /** The key the box's value is stored under. */
  k: string;
  /** The box's type. "radio" and "chk" are the tick kinds; any other is a typed box. */
  t: string;
  /** Required (field flag r). */
  r?: boolean;
  /** Computed by the form: never to do. */
  c?: boolean;
  /** The radio group the box belongs to. */
  grp?: string;
  /** A pooled line: the keys of its pool, drawn by the pool's first line. */
  pool?: { keys: string[] } | null;
}

/** One row of a part: its boxes (f), or a table of rows (rows, with the keys of each row's cells in keys). */
export interface TodoItem {
  l?: string;
  rows?: number;
  cols?: { l?: string }[];
  keys?: string[][];
  f?: TodoField[];
}

/** A part of the form: its name and its items, in order. */
export interface TodoPart {
  n?: string;
  items: TodoItem[];
}

/** Why a row needs the user. Strongest first: message, then required, then stop. */
export type TodoReason = "message" | "required" | "stop";

/** A row of the to-do list: its id ("part:item"), its strongest reason, and the stop boxes it holds. */
export interface TodoRow {
  id: string;
  reason: TodoReason;
  stops: string[];
}

/** What todoRows is given: the schema's parts and what the page knows about the draft. */
export interface TodoRowsOptions {
  parts?: TodoPart[] | null;
  /** Row id to its validation message. */
  messages?: Readonly<Record<string, string>>;
  /** Box key to the value the draft holds. */
  values?: Readonly<Record<string, unknown>>;
  /** The keys of the boxes the pill stops on. */
  stops?: ReadonlySet<string>;
  /** Row ids the user has stepped past on this visit. */
  passed?: ReadonlySet<string>;
  blocked?: (k: string) => boolean;
}

/** Strongest first. */
const RANK: Record<TodoReason, number> = { message: 0, required: 1, stop: 2 };

/**
 * The boxes the pill stops on: the automatic stops less the ones switched off, plus the ones switched on (I-2).
 * Absent fields count as empty.
 */
export function effectiveStops(
  autoStops: readonly string[] | null | undefined,
  stopEdits: Readonly<Record<string, boolean>> | null | undefined,
): Set<string> {
  const edits = stopEdits || {};
  const out = new Set<string>();
  for (const k of autoStops || []) if (edits[k] !== false) out.add(k);
  for (const [k, on] of Object.entries(edits)) if (on === true) out.add(k);
  return out;
}

/** Where Stop here should send box k: back to automatic (null) when that already says so. */
export function stopCall(k: string, on: boolean, autoStops: readonly string[] | null | undefined): boolean | null {
  const auto = (autoStops || []).includes(k);
  return on === auto ? null : on;
}

/** The keys an item's boxes write (a schedule's every cell, a pooled line's pool). */
export function itemKeys(it: TodoItem): string[] {
  if (it.rows) return (it.keys ?? []).flat();
  return (it.f ?? []).flatMap((f) => (f.pool ? f.pool.keys : [f.k]));
}

function ticked(v: unknown): boolean {
  return v === "true" || v === "1" || v === "Y";
}

function holds(v: unknown): boolean {
  return v != null && String(v).trim() !== "";
}

/** Whether box k of item it holds an answer: a tick group by any tick, any other box by any text. */
function answered(it: TodoItem, k: string, values: Readonly<Record<string, unknown>>): boolean {
  const f = (it.f ?? []).find((x) => x.k === k);
  if (f && (f.t === "radio" || f.t === "chk")) {
    const group = f.t === "radio" ? (it.f ?? []).filter((x) => x.t === "radio" && (!f.grp || x.grp === f.grp)) : [f];
    return group.some((x) => ticked(values[x.k]));
  }
  return holds(values[k]);
}

/** Whether a required box of a plain item is still empty. */
function requiredBlank(it: TodoItem, values: Readonly<Record<string, unknown>>, blocked: (k: string) => boolean): boolean {
  const fs = (it.f ?? []).filter((f) => f.r && !f.c && !blocked(f.k));
  if (!fs.length) return false;
  const radios = fs.filter((f) => f.t === "radio" || f.t === "chk");
  // A tick group is one item: answered by any tick in it, shut ones too.
  if (radios.length) {
    const all = (it.f ?? []).filter((f) => f.t === "radio" || f.t === "chk");
    if (!all.some((f) => ticked(values[f.k]))) return true;
  }
  return fs.some((f) => f.t !== "radio" && f.t !== "chk" && !holds(values[f.k]));
}

/** The rows needing the user, in form order (part, then item), each with its strongest reason. */
export function todoRows({
  parts,
  messages = {},
  values = {},
  stops = new Set<string>(),
  passed = new Set<string>(),
  blocked = () => false,
}: TodoRowsOptions): TodoRow[] {
  const out: TodoRow[] = [];
  (parts ?? []).forEach((p, pi) =>
    p.items.forEach((it, ii) => {
      const id = `${pi}:${ii}`;
      const reasons: TodoReason[] = [];
      if (messages[id]) reasons.push("message");
      if (!it.rows && requiredBlank(it, values, blocked)) reasons.push("required");
      const own = stops.size ? itemKeys(it).filter((k) => stops.has(k)) : [];
      if (own.length && !passed.has(id) && !own.every((k) => answered(it, k, values))) reasons.push("stop");
      if (!reasons.length) return;
      reasons.sort((a, b) => RANK[a] - RANK[b]);
      out.push({ id, reason: reasons[0], stops: own });
    }),
  );
  return out;
}

/** "p:i" in form order: negative when a comes first. */
export function compareRows(a: string, b: string): number {
  const [ap, ai] = a.split(":").map(Number);
  const [bp, bi] = b.split(":").map(Number);
  return ap - bp || ai - bi;
}

/**
 * Where the previous row (dir -1) or the next (dir 1) is, in ids (the to-do rows, in order), from row at. With no row
 * at, it is from the start of part, or from the top when fromTop. Null at either end.
 */
export function nextRow(
  ids: readonly string[],
  dir: number,
  at: string | null | undefined,
  part: number,
  fromTop = false,
): string | null {
  if (!ids.length) return null;
  if (at) {
    const here = at;
    return dir > 0
      ? (ids.find((id) => compareRows(id, here) > 0) ?? null)
      : ([...ids].reverse().find((id) => compareRows(id, here) < 0) ?? null);
  }
  if (fromTop) return dir > 0 ? ids[0] : null;
  const partOf = (id: string) => Number(id.split(":")[0]);
  return dir > 0
    ? (ids.find((id) => partOf(id) >= part) ?? null)
    : ([...ids].reverse().find((id) => partOf(id) < part) ?? null);
}
