// The account's TIN statuses on screen (D2: relay-filing, tracking hexuria/buwiz-forms#299): one owner of the list the
// host's account_tins answers, the refresh points the product sets, and the Activate flow. The chips themselves are in
// TinChip.tsx. Never on a timer: the list is asked at start, when the app becomes active, at Queue (max_age 0), after
// Activate or a return from the account site; screens otherwise read with max_age 60.
// TODO(account_tins_cache): keep the last list in the room file so an offline start shows it; that needs the
// account-room work in flight elsewhere, so this PR reads memory only.
import { signal } from "nsh";
import type { TinStatusList, TinStatusRow } from "../../bir";
import { bir } from "../../bir";
import { errorText } from "../../lib/errors";

const list = signal<TinStatusList | null>(null);
const error = signal<string | null>(null);
/** The 14-digit TINs whose Activate request is running. */
const activating = signal<string[]>([]);
/** The refusal sentence of the last Activate, keyed by the 14-digit TIN, until a refresh or a retry clears it. */
const refusals = signal<Record<string, string>>({});
let inFlight: Promise<void> | null = null;

/** The list as last read, or null when there is none yet (never read, signed out, unreadable). */
export function tinsList(): TinStatusList | null {
  return list();
}

/** Why there is no list (signed out, offline with no list, the server's refusal), or null. */
export function tinsError(): string | null {
  return error();
}

/** The row for one TIN root (9 digits) and branch (5 digits), or null. */
export function tinRow(tinRoot: string, branch: string): TinStatusRow | null {
  const rows = list()?.rows;
  if (!rows) return null;
  return rows.find((row) => row.tin_root === tinRoot && row.branch === branch) ?? null;
}

/** Reads the account's TINs. Screens pass 60; Queue and the moments after a change pass 0. Errors are kept, not thrown. */
export async function refreshTins(maxAge: number): Promise<TinStatusList | null> {
  if (inFlight && maxAge > 0) {
    await inFlight;
    return list();
  }
  const run = (async () => {
    try {
      const answer = await bir.account_tins(maxAge);
      list.set(answer);
      error.set(null);
    } catch (e) {
      error.set(errorText(e));
    }
  })();
  inFlight = run;
  try {
    await run;
  } finally {
    if (inFlight === run) inFlight = null;
  }
  return list();
}

/** True while this TIN's Activate request runs. */
export function tinActivating(tinFull: string): boolean {
  return activating().includes(tinFull);
}

/** The refusal sentence the last Activate of this TIN ended in, or null. */
export function tinRefusal(tinFull: string): string | null {
  return refusals()[tinFull] ?? null;
}

/** Clears the refusal line (a retry, or the row showing). */
export function clearTinRefusal(tinFull: string): void {
  if (!(tinFull in refusals())) return;
  const next = { ...refusals() };
  delete next[tinFull];
  refusals.set(next);
}

/**
 * "Activate this TIN to send". The host words every refusal; this shows exactly its sentence. Whether it throws or
 * answers, the list is asked again at the server (max_age 0): an activation or a refusal saying the account already
 * holds the TIN changes what the list says.
 */
export async function activateTin(tinFull: string): Promise<TinStatusRow | null> {
  if (activating().includes(tinFull)) return null;
  activating.set([...activating(), tinFull]);
  clearTinRefusal(tinFull);
  let row: TinStatusRow | null = null;
  let thrown: string | null = null;
  try {
    row = await bir.activate_tin(tinFull);
  } catch (e) {
    thrown = errorText(e);
  }
  activating.set(activating().filter((tin) => tin !== tinFull));
  await refreshTins(0);
  if (thrown) refusals.set({ ...refusals(), [tinFull]: thrown });
  return row;
}

/** A return from the account site (a Fix or Verify link): the list is asked again at the server. */
export async function returnedFromAccountSite(): Promise<void> {
  await refreshTins(0);
}
