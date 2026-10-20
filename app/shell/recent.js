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
