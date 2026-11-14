// Text for a TIN and a form code, from store.js (formatTin, displayCode). Pure: no host call, no nsh import.

/**
 * "009-123-456" from a 9-digit root; "009-123-456-00000" with a branch code. Anything but digits in the root is dropped
 * ("009-123-456" and "009 123 456" read as the same root), a root of 9 digits or more shows its first nine (a full
 * 14-digit TIN loses its branch: pass the branch apart), a shorter root stays as its digits, and a missing branch (null,
 * undefined, "") leaves no trailing dash.
 */
export function formatTin(root?: string | number | null, branch?: string | number | null): string {
  const r = String(root || "").replace(/\D/g, "");
  const base = r.length >= 9 ? `${r.slice(0, 3)}-${r.slice(3, 6)}-${r.slice(6, 9)}` : r;
  return branch ? `${base}-${branch}` : base;
}

/** The code people see for a form code ("2551Q", "1601C"): the code as it is, "" for none. */
export function displayCode(code?: string | number | null): string {
  return String(code || "");
}
