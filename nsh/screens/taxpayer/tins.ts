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
/** The refusal sentence of the last Activate, keyed by the 14-digit TIN, until a retry, a Dismiss or a new owner clears it. */
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

/** The key the account's rows carry: the 9-digit root plus the 5-digit normalized branch, however old the stored tin
 * (a legacy row's tin is the 9-digit root plus a 3-digit branch). Anything but digits is stripped first. */
export function tinKeyParts(tin: unknown): [string, string] {
  const digits = String(tin).replace(/\D/g, "");
  return [digits.slice(0, 9), digits.slice(9).padStart(5, "0")];
}

/** The row for one TIN root (9 digits) and branch (5 digits), or null. */
export function tinRow(tinRoot: string, branch: string): TinStatusRow | null {
  const rows = list()?.rows;
  if (!rows) return null;
  return rows.find((row) => row.tin_root === tinRoot && row.branch === branch) ?? null;
}

/** The signed-in owner the kept list belongs to; undefined until the first read checks. */
let owner: string | null | undefined = undefined;

/** Who the account list belongs to now (a local read, never the network): the user id, the email, null signed out. */
async function currentOwner(): Promise<string | null> {
  try {
    const status = await bir.cloud_account_status();
    return status.signed_in ? status.user_id || status.email || "signed-in" : null;
  } catch {
    return null;
  }
}

/**
 * Reads the account's TINs. Screens pass 60; Queue and the moments after a change pass 0. The answer is THIS call's:
 * a thrown read answers null (never the kept list, which would pass as fresh downstream) and the kept list is marked
 * stale for the chips. A new signed-in owner clears the kept list and the refusals first: nothing of one account
 * answers for another.
 */
export async function refreshTins(maxAge: number): Promise<TinStatusList | null> {
  if (inFlight && maxAge > 0) {
    await inFlight;
    return list();
  }
  const run = (async (): Promise<TinStatusList | null> => {
    const me = await currentOwner();
    if (owner !== undefined && me !== owner) {
      list.set(null);
      refusals.set({});
    }
    owner = me;
    try {
      const answer = await bir.account_tins(maxAge);
      list.set(answer);
      error.set(null);
      return answer;
    } catch (e) {
      error.set(errorText(e));
      const kept = list();
      if (kept && !kept.stale) list.set({ ...kept, stale: true, rows: kept.rows.map((row) => ({ ...row, stale: true })) });
      return null;
    }
  })();
  inFlight = run.then(() => undefined);
  try {
    return await run;
  } finally {
    if (inFlight !== null) inFlight = null;
  }
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
  await refreshTins(0);
  activating.set(activating().filter((tin) => tin !== tinFull));
  if (thrown) refusals.set({ ...refusals(), [tinFull]: thrown });
  return row;
}
