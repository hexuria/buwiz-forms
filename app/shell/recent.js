// Recent returns (the sidebar's "Recent"): what an entry is made of.

/**
 * A Recent entry for a return the app opened. `key` is the entry's own id
 * (always text, used for the row's element id and to keep one entry per
 * return); a stored return's exact key travels as `returnKey`, so it can
 * never take the entry's id.
 */
export function recentEntry(opts) {
  const { tin, code, year, period } = opts;
  const exact = opts.returnKey != null ? `|${opts.returnKey}` : "";
  return { ...opts, key: `${tin}|${code}|${year}|${period}${exact}` };
}

/** What opening a Recent entry asks for: the same return, exactly. */
export function recentOpenOptions(r) {
  const { tin, code, year, period } = r;
  return { tin, code, year, period, ...(r.returnKey != null ? { returnKey: r.returnKey } : {}) };
}
