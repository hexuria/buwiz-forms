/**
 * The form page's to-do list (decision 12): the rows the pill's ‹ and ›
 * visit, in form order, each once with the strongest reason it needs the
 * user. Pure: the page hands in what it knows and gets rows back.
 *
 * - `message`: the row holds a validation message.
 * - `required`: a required box (field flag `r`) is empty, not computed and
 *   not shut (`blocked(k)`: locked, turned off by another answer, or a
 *   read-only return). A radio group counts as one item.
 * - `stop`: one of the row's boxes is a stop (`effectiveStops`), not all
 *   of its stop boxes hold a value, and the user has not stepped past it on
 *   this visit (`passed`).
 */

/** Strongest first. */
const RANK = { message: 0, required: 1, stop: 2 };

/**
 * The boxes the pill stops on: the automatic stops less the ones switched
 * off, plus the ones switched on (I-2). Absent fields count as empty.
 */
export function effectiveStops(autoStops, stopEdits) {
  const edits = stopEdits || {};
  const out = new Set();
  for (const k of autoStops || []) if (edits[k] !== false) out.add(k);
  for (const [k, on] of Object.entries(edits)) if (on === true) out.add(k);
  return out;
}

/** Where "Stop here" should send `on` for box `k`: back to automatic (null) when that already says so. */
export function stopCall(k, on, autoStops) {
  const auto = (autoStops || []).includes(k);
  return on === auto ? null : on;
}

/** The keys an item's boxes write (a schedule's every cell, a pooled line's pool). */
export function itemKeys(it) {
  if (it.rows) return it.keys.flat();
  return (it.f || []).flatMap((f) => (f.pool ? f.pool.keys : [f.k]));
}

function ticked(v) {
  return v === "true" || v === "1" || v === "Y";
}

function holds(v) {
  return v != null && String(v).trim() !== "";
}

/** Whether box `k` of item `it` holds an answer: a tick group by any tick, any other box by any text. */
function answered(it, k, values) {
  const f = (it.f || []).find((x) => x.k === k);
  if (f && (f.t === "radio" || f.t === "chk")) {
    const group = f.t === "radio" ? (it.f || []).filter((x) => x.t === "radio" && (!f.grp || x.grp === f.grp)) : [f];
    return group.some((x) => ticked(values[x.k]));
  }
  return holds(values[k]);
}

/** Whether a required box of a plain item is still empty. */
function requiredBlank(it, values, blocked) {
  const fs = (it.f || []).filter((f) => f.r && !f.c && !blocked(f.k));
  if (!fs.length) return false;
  const radios = fs.filter((f) => f.t === "radio" || f.t === "chk");
  // A tick group is one item: answered by any tick in it, shut ones too.
  if (radios.length) {
    const all = (it.f || []).filter((f) => f.t === "radio" || f.t === "chk");
    if (!all.some((f) => ticked(values[f.k]))) return true;
  }
  return fs.some((f) => f.t !== "radio" && f.t !== "chk" && !holds(values[f.k]));
}

/**
 * The rows needing the user, in form order (part, then item):
 * `[{ id: "p:i", reason, stops: [keys] }]`.
 *
 * `opts`: `parts` (the schema's), `messages` (row id → text), `values`,
 * `stops` (Set of box keys), `passed` (Set of row ids), `blocked(k)`.
 */
export function todoRows({ parts, messages = {}, values = {}, stops = new Set(), passed = new Set(), blocked = () => false }) {
  const out = [];
  (parts || []).forEach((p, pi) =>
    p.items.forEach((it, ii) => {
      const id = `${pi}:${ii}`;
      const reasons = [];
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

/** "p:i" in form order: negative when `a` comes first. */
export function compareRows(a, b) {
  const [ap, ai] = a.split(":").map(Number);
  const [bp, bi] = b.split(":").map(Number);
  return ap - bp || ai - bi;
}

/**
 * Where ‹ (`dir` -1) or › (1) goes in `ids` (the to-do rows, in order) from
 * row `at` (null: from the start of part `part`, or from the top when
 * `fromTop`). Null at either end.
 */
export function nextRow(ids, dir, at, part, fromTop = false) {
  if (!ids.length) return null;
  if (at) {
    return dir > 0 ? ids.find((id) => compareRows(id, at) > 0) ?? null : [...ids].reverse().find((id) => compareRows(id, at) < 0) ?? null;
  }
  if (fromTop) return dir > 0 ? ids[0] : null;
  const partOf = (id) => Number(id.split(":")[0]);
  return dir > 0 ? ids.find((id) => partOf(id) >= part) ?? null : [...ids].reverse().find((id) => partOf(id) < part) ?? null;
}
