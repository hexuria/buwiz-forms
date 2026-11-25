// Text for a TIN, a form code and the ZIP choices. Pure: no host call, no nsh import.

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

/**
 * One choice per ZIP code from the reference list's "1100 - Central" entries. Several places share a code (1100 is
 * Central, Piñahan, Project 6 and Quezon City CPO), and only the code is stored, so a choice per place would tick one
 * place and show another. The label names every place with that code, in the list's order; an entry with no code is
 * left out.
 */
export function zipChoices(entries: readonly string[]): { value: string; label: string }[] {
  const places = new Map<string, string[]>();
  for (const entry of entries) {
    const cut = entry.indexOf(" - ");
    const code = (cut < 0 ? entry : entry.slice(0, cut)).trim();
    if (!code) continue;
    const place = cut < 0 ? "" : entry.slice(cut + 3).trim();
    const known = places.get(code) ?? [];
    if (place && !known.includes(place)) known.push(place);
    places.set(code, known);
  }
  return [...places].map(([value, names]) => ({ value, label: names.length ? `${value} - ${names.join(", ")}` : value }));
}
