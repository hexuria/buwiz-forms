// Money text from form_page.js: formatMoney (what an amount box shows) and wholePesosOf (the whole-peso amount
// a "Use it" hint offers). Pure. No Intl (QuickJS has none), so the digits are grouped by hand.

/** An amount in whole pesos, and whether the centavos were dropped to reach it. */
export interface WholePesos {
  amount: number;
  rounded: boolean;
}

/**
 * "1,234.50" from a number-ish string; empty stays empty. whole: a box over a whole-number field shows whole pesos
 * ("1,234"); a typed fraction stays as typed so the box's own message explains it.
 */
export function formatMoney(text: string | number, whole = false): string {
  const raw = String(text);
  const n = Number(raw.replace(/[₱,\s]/g, ""));
  if (raw.trim() === "" || Number.isNaN(n)) return raw;
  if (whole && !Number.isInteger(n)) return raw;
  // QuickJS has no Intl, so group the digits by hand.
  const [int, frac] = Math.abs(n).toFixed(whole ? 0 : 2).split(".");
  const grouped = `${n < 0 ? "-" : ""}${int.replace(/\B(?=(\d{3})+(?!\d))/g, ",")}`;
  return whole ? grouped : `${grouped}.${frac}`;
}

/**
 * What an amount box shows, in whole pesos rounded half up (Math.round, as the official helpers round), and whether
 * that dropped centavos; null for a blank, zero or unreadable box.
 */
export function wholePesosOf(text: string | number | null | undefined): WholePesos | null {
  const raw = String(text ?? "");
  const n = Number(raw.replace(/[₱,\s]/g, ""));
  if (raw.trim() === "" || !Number.isFinite(n)) return null;
  const amount = Math.round(n);
  if (amount === 0) return null;
  return { amount, rounded: amount !== n };
}
