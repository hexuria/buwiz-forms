// Recent returns (the sidebar's "Recent"): what an entry is made of.

/**
 * A Recent entry for a return the app opened. `key` is the entry's own id
 * (always text, used for the row's element id and to keep one entry per
 * return); a stored return's exact key travels as `returnKey`, so it can
 * never take the entry's id.
 */
export function recentEntry(opts) {
  // `newKey` started a new return once; reopening it from Recent opens what
  // is stored for its period.
  // `newKeyFrom` is the created_at of the sample that offered `newKey`.
  const { newKey: _started, newKeyFrom: _sampleCreatedAt, ...entry } = opts;
  const { tin, code, year, period } = entry;
  const exact = entry.returnKey != null ? `|${entry.returnKey}` : "";
  return { ...entry, key: `${tin}|${code}|${year}|${period}${exact}` };
}

/** What opening a Recent entry asks for: the same return, exactly. */
export function recentOpenOptions(r) {
  const { tin, code, year, period } = r;
  return { tin, code, year, period, ...(r.returnKey != null ? { returnKey: r.returnKey } : {}) };
}

/** How many returns Recent keeps. */
const RECENT_MAX = 4;

/** The id of the Recent entry for a return, from its identity alone. */
export function recentKey(r) {
  return recentEntry(recentOpenOptions(r)).key;
}

/**
 * Two entries for one return: the same taxpayer, form and period, and exact
 * keys that do not disagree. An entry without an exact key opens what is
 * stored for its period, which is the return an exact key names: listing
 * both showed one return twice (opened once from the form list, once from
 * a Filings card). Two different exact keys are two returns.
 */
export function sameReturn(a, b) {
  if (a.tin !== b.tin || a.code !== b.code || a.year !== b.year || a.period !== b.period) return false;
  return a.returnKey == null || b.returnKey == null || a.returnKey === b.returnKey;
}

/**
 * Recent as stored. An entry an earlier build wrote without a text id (a
 * number, or none) is rebuilt; a return an earlier build listed twice keeps
 * its first place (and its exact key); anything that is not a list is no list.
 */
export function loadRecent(stored) {
  if (!Array.isArray(stored)) return [];
  const out = [];
  for (const raw of stored) {
    if (!raw || typeof raw !== "object") continue;
    const r = typeof raw.key === "string" ? raw : recentEntry(raw);
    const at = out.findIndex((x) => sameReturn(x, r));
    if (at < 0) out.push(r);
    else if (out[at].returnKey == null && r.returnKey != null) out[at] = r;
  }
  return out;
}

/**
 * Opening a return: one Recent already lists keeps its place. A return not
 * listed yet joins at the top, and the oldest entry past four goes.
 */
export function addRecent(list, opts) {
  const entry = recentEntry(opts);
  const at = list.findIndex((r) => sameReturn(r, entry));
  if (at < 0) return [entry, ...list].slice(0, RECENT_MAX);
  // Listed without its exact key, opened now with it: the entry takes the
  // key where it is.
  if (list[at].returnKey == null && entry.returnKey != null) return list.map((r, i) => (i === at ? entry : r));
  return list;
}

/** A return was changed and saved: it moves to the top (or joins there). */
export function bumpRecent(list, opts) {
  const saved = recentEntry(opts);
  const listed = list.find((r) => sameReturn(r, saved));
  // The more exact of the two stays.
  const entry = listed && (listed.returnKey != null || saved.returnKey == null) ? listed : saved;
  return [entry, ...list.filter((r) => r !== listed && r.key !== entry.key)].slice(0, RECENT_MAX);
}
