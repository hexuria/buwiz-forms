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
 * Recent as stored. An entry an earlier build wrote without a text id (a
 * number, or none) is rebuilt; anything that is not a list is no list.
 */
export function loadRecent(stored) {
  if (!Array.isArray(stored)) return [];
  return stored.filter((r) => r && typeof r === "object").map((r) => (typeof r.key === "string" ? r : recentEntry(r)));
}

/**
 * Opening a return: one Recent already lists keeps its place. A return not
 * listed yet joins at the top, and the oldest entry past four goes.
 */
export function addRecent(list, opts) {
  const entry = recentEntry(opts);
  if (list.some((r) => r.key === entry.key)) return list;
  return [entry, ...list].slice(0, RECENT_MAX);
}

/** A return was changed and saved: it moves to the top (or joins there). */
export function bumpRecent(list, opts) {
  const key = recentKey(opts);
  const entry = list.find((r) => r.key === key) || recentEntry(opts);
  return [entry, ...list.filter((r) => r.key !== key)].slice(0, RECENT_MAX);
}
