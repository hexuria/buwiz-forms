// The cached reads over nsh/bir (old app/shell/store.js, the Store), made of NSH signals.
//
// A render must not hit the database: every read here is memoised by key until a write drops it. Each key owns one
// signal (its revision): a read subscribes to it, forget(key) moves it, so a screen re-runs only the reads whose key
// changed. A read that throws answers its fallback, logs once, and is asked again after two seconds (a timer moves the
// key's revision, so what shows the fallback re-reads by itself); each further failure of the same key waits twice as
// long (2 s, 4 s, 8 s ... a minute), and a success, forget(key) or bump() starts the wait over. The session's scope narrows what taxpayers() and the
// returns and deadlines name: setScope(root) is called by the app state and nowhere else.
//
//   const list = store.taxpayers();                       // in JSX or a closure: re-runs when taxpayers change
//   store.write("set_setting", "k", "v");                 // a write: runs, then drops every cached read
//   store.writeFor(/^setting:/, "set_setting", "k", "v"); // a write that drops only the keys it touches
import { batch, signal, type Signal } from "nsh";
import { bir } from "../bir";
import { errorText } from "../lib/errors";

/** A read that failed answers its fallback for this long, then is asked again. */
export const FAILED_READ_RETRY_MS = 2000;
/** The longest a failing read waits between two asks. */
export const FAILED_READ_RETRY_MAX_MS = 60_000;

/** How long a key that has now failed `failures` times in a row answers its fallback: 2 s, 4 s, 8 s ... capped at a minute. */
export function failedReadDelay(failures: number): number {
  return Math.min(FAILED_READ_RETRY_MAX_MS, FAILED_READ_RETRY_MS * 2 ** Math.max(0, failures - 1));
}

/** The cache key of savedReturns(): not a "returns:" key, so the 30 s poll keeps it (Home refreshes it in the background after it is drawn). */
export const SAVED_RETURNS_KEY = "saved_returns";

/**
 * The host functions that queue a return or take it out of the queue. The submission worker answers within a second or two (dry-run:
 * "Submitted"), and nothing tells the app, so after one of these the lists are read again at once and then at each of QUEUE_REREAD_MS.
 */
export const QUEUE_FUNCTIONS: ReadonlySet<string> = new Set(["queue_form_draft", "cancel_queued", "screen_form_queue", "screen_form_cancel_queue"]);
/** How long after a queue or a cancel-queue the lists are read again (and they are read at once as well). */
export const QUEUE_REREAD_MS = [1000, 3000, 10_000] as const;
/** What the worker changes behind the app's back: the returns lists, their deadlines and yearly form rows, the jobs, the drafts count, the as-needed forms. */
export const WORKER_KEYS = /^(returns:|deadlines|forms:|jobs$|saved_returns$|as_needed:)/;
/**
 * What the plain 30 s events poll drops: WORKER_KEYS less the drafts count and the as-needed forms. The drafts count (SAVED_RETURNS_KEY) is
 * kept past this poll on purpose, so a visit to Home finds it held and draws at once (Home reads it again in the background after it is
 * up). The Queued watch and the queue actions use the wider WORKER_KEYS: a return that leaves the queue changes both.
 */
export const POLL_KEYS = /^(returns:|deadlines|forms:|jobs$)/;

type Bir = typeof bir;
type BirName = keyof Bir;
type BirArgs<K extends BirName> = Bir[K] extends (...args: infer A) => unknown ? A : never;
type BirResult<K extends BirName> = Bir[K] extends (...args: never[]) => infer R ? R : never;

interface Entry {
  /** Moves when the key is forgotten: what read the key re-runs. */
  rev: Signal<number>;
  has: boolean;
  value: unknown;
  /** When the read failed (Date.now()), so it is asked again. */
  failedAt: number | undefined;
  /** How many asks in a row have failed since the key was last forgotten or read well: the backoff's step. */
  failures: number;
  /** The last error text, so one failure is logged once. */
  error: string | undefined;
  retry: ReturnType<typeof setTimeout> | undefined;
}

const entries = new Map<string, Entry>();
const scope = signal<string | null>(null);
let rdoList: { value: string; label: string }[] | null = null;
let zipList: { value: string; label: string }[] | null = null;
type FormIndex = ReturnType<Bir["screen_forms_index"]>;
/** The form library's index: it never changes while the app runs and takes about a second to build, so it is kept past bump(). */
let formIndex: FormIndex | null = null;
/** What screenIndex() answers while there is no index (a build that failed, waiting out its backoff): one array, so a reader that gets it again is not told of a change. */
const NO_INDEX: FormIndex = [];
/**
 * The index's revision: what shows the index reads it, so it asks again when a failed build is due again and once the index fills after
 * a failure (a signal is never written inside a read, so both moves come from a timer).
 */
const formIndexRev = signal(0);
/** A build that failed: waiting out its backoff (until the retry timer), how many in a row (the backoff's step), the last error text (logged once), the retry timer. */
const formIndexFail: { waiting: boolean; count: number; error: string | undefined; retry: ReturnType<typeof setTimeout> | undefined } = {
  waiting: false,
  count: 0,
  error: undefined,
  retry: undefined,
};
/** The pending re-reads after a queue action; a newer action starts the series again. */
let queueRereads: ReturnType<typeof setTimeout>[] = [];
/** When the last queue action happened (Date.now()), 0 for none: the Queued watch starts its fast phase again from it. */
let lastQueueActionAt = 0;

function entryOf(key: string): Entry {
  let e = entries.get(key);
  if (!e) {
    e = { rev: signal(0), has: false, value: undefined, failedAt: undefined, failures: 0, error: undefined, retry: undefined };
    entries.set(key, e);
  }
  return e;
}

/** A screen failure for the daily log and the Logs tab (what console.* also reaches). Logging never hides the failure. */
function logScreen(level: "error" | "warn" | "info", message: string): void {
  try {
    bir.record_screen_log(level, String(message));
  } catch {
    if (level === "error") console.error(message);
    else if (level === "warn") console.warn(message);
    else console.log(message);
  }
}

/** Makes what read the entry ask again; the failure count stays, so the wait keeps growing. */
function askAgain(e: Entry): void {
  e.has = false;
  e.failedAt = undefined;
  if (e.retry !== undefined) clearTimeout(e.retry);
  e.retry = undefined;
  e.rev.update((n) => n + 1);
}

/** An explicit drop: what read the key asks again, and its backoff starts over. */
function forget(key: string): void {
  const e = entries.get(key);
  if (!e) return;
  e.failures = 0;
  askAgain(e);
}

/** Memoised read of key: fn once, then the cached answer until forgotten; the fallback while fn throws. */
function read<T>(key: string, fn: () => T, fallback: T, label = key): T {
  const e = entryOf(key);
  e.rev();
  if (e.has && (e.failedAt === undefined || Date.now() - e.failedAt < failedReadDelay(e.failures))) return e.value as T;
  let value = fallback;
  try {
    value = fn();
    e.error = undefined;
    e.failedAt = undefined;
    e.failures = 0;
  } catch (error) {
    const text = String(error);
    if (e.error !== text) logScreen("error", `${label}: ${errorText(error)}`);
    e.error = text;
    e.failedAt = Date.now();
    e.failures += 1;
    if (e.retry !== undefined) clearTimeout(e.retry);
    e.retry = setTimeout(() => askAgain(e), failedReadDelay(e.failures));
  }
  e.has = true;
  e.value = value;
  return value;
}

function call<K extends BirName>(name: K, ...args: BirArgs<K>): BirResult<K> {
  const fn = bir[name] as unknown;
  if (typeof fn !== "function") throw new Error(`This build has no bir.${name}`);
  return (fn as (...a: unknown[]) => unknown)(...args) as BirResult<K>;
}

function keysMatching(test: RegExp | string | ((key: string) => boolean)): string[] {
  const match = typeof test === "function" ? test : typeof test === "string" ? (k: string) => k === test : (k: string) => test.test(k);
  return [...entries.keys()].filter(match);
}

function toIndex<T extends { tin: string; form_code: string; taxable_year: number }>(rows: T[]): Map<string, T[]> {
  const index = new Map<string, T[]>();
  for (const d of rows) {
    const k = `${d.tin}|${d.form_code}|${d.taxable_year}`;
    const list = index.get(k);
    if (list) list.push(d);
    else index.set(k, [d]);
  }
  return index;
}

/** Reads the worker-moved lists again (what shows them re-reads). */
function rereadWorkerLists(): void {
  store.forgetWhere(WORKER_KEYS);
}

/** Cancels the pending re-reads of a queue action. */
function cancelRereads(): void {
  for (const timer of queueRereads) clearTimeout(timer);
  queueRereads = [];
}

/** After a queue action: read now, and again after each of QUEUE_REREAD_MS. */
function rereadSoon(): void {
  cancelRereads();
  lastQueueActionAt = Date.now();
  rereadWorkerLists();
  queueRereads = QUEUE_REREAD_MS.map((ms) => setTimeout(rereadWorkerLists, ms));
}

/** The index could not be built (or came back empty): [] until the retry is due, which waits twice as long each time in a row (2 s, 4 s ... a minute). */
function formIndexFailed(error: unknown): void {
  const text = String(error);
  if (formIndexFail.error !== text) logScreen("error", `screen_forms_index: ${errorText(error)}`);
  formIndexFail.error = text;
  formIndexFail.waiting = true;
  formIndexFail.count += 1;
  if (formIndexFail.retry !== undefined) clearTimeout(formIndexFail.retry);
  formIndexFail.retry = setTimeout(() => {
    formIndexFail.retry = undefined;
    formIndexFail.waiting = false;
    formIndexRev.update((n) => n + 1);
  }, failedReadDelay(formIndexFail.count));
}

/** The app's reads and writes of the Rust brain. One instance; screens use it through shell/index.ts. */
export const store = {
  // ------------------------------------------------------------ scope (the session's visibility)

  /** The TIN root a taxpayer sign-in narrows everything to, or null (reactive). */
  scopeRoot: (): string | null => scope(),
  /** Narrows taxpayers() to one taxpayer for a taxpayer sign-in; null shows all. Only the app state calls it. */
  setScope(root: string | null): void {
    scope.set(root);
  },
  /** Whether the session may see a taxpayer's deadlines and returns (reactive: they name the taxpayer). */
  canSee(root: string): boolean {
    const s = scope();
    return !s || root === s;
  },

  // ------------------------------------------------------------ cache control

  /** Drops one cached read: what read it re-runs. */
  forget,
  /** Drops every cached read whose key matches. */
  forgetWhere(test: RegExp | string | ((key: string) => boolean)): void {
    batch(() => {
      for (const key of keysMatching(test)) forget(key);
    });
  },
  /**
   * Drops every cached read. Call after a write that could touch anything. Name the host function that was written (a page's
   * afterWrite hook has it): a queue or a cancel-queue also starts the re-reads at once and after 1 s, 3 s and 10 s.
   */
  bump(written?: string): void {
    batch(() => {
      for (const key of [...entries.keys()]) forget(key);
    });
    if (written !== undefined && QUEUE_FUNCTIONS.has(written)) rereadSoon();
  },
  /** A return was queued or taken out of the queue (by a path that does not go through write()): read the lists now and again after 1 s, 3 s and 10 s. */
  afterQueueChange(): void {
    rereadSoon();
  },
  /** Cancels the re-reads a queue action left pending: the session they were for has ended (a lock, a sign-out) or the frame is gone. */
  cancelRereads,
  /** When the last queue action happened (Date.now()), or 0. */
  lastQueueAction: (): number => lastQueueActionAt,
  /** True when key holds an answer already (a read that is not dropped yet). */
  has(key: string): boolean {
    return entries.get(key)?.has === true;
  },
  /** True when the last read of key failed and holds its fallback. */
  failed(key: string): boolean {
    return entries.get(key)?.failedAt !== undefined;
  },
  /** A screen failure for the log. */
  logScreen,

  // ------------------------------------------------------------ calls and writes

  /** A host call that is not cached (a verify, a one-off read). Throws what the host throws. */
  call,
  /** A memoised read of your own: `fn` once per key until it is forgotten; the fallback while `fn` throws (asked again after two seconds). */
  read,
  /** A write: runs the host function, then drops every cached read (also when it throws); a failure is logged and rethrown. */
  write<K extends BirName>(name: K, ...args: BirArgs<K>): BirResult<K> {
    try {
      return call(name, ...args);
    } catch (error) {
      logScreen("error", `bir.${name}: ${errorText(error)}`);
      throw error;
    } finally {
      store.bump(name);
    }
  },
  /** A write that drops only the cached reads whose key matches `touches`. */
  writeFor<K extends BirName>(touches: RegExp | string | ((key: string) => boolean), name: K, ...args: BirArgs<K>): BirResult<K> {
    try {
      return call(name, ...args);
    } catch (error) {
      logScreen("error", `bir.${name}: ${errorText(error)}`);
      throw error;
    } finally {
      store.forgetWhere(touches);
      if (QUEUE_FUNCTIONS.has(name)) rereadSoon();
    }
  },
  /** write() for a host function that answers a promise: drops the cache once it settles. */
  async writeAsync<K extends BirName>(name: K, ...args: BirArgs<K>): Promise<Awaited<BirResult<K>>> {
    try {
      return (await call(name, ...args)) as Awaited<BirResult<K>>;
    } catch (error) {
      logScreen("error", `bir.${name}: ${errorText(error)}`);
      throw error;
    } finally {
      store.bump(name);
    }
  },

  // ------------------------------------------------------------ reads

  systemInfo() {
    return read("system_info", () => bir.system_info(), { os: "macos" } as ReturnType<Bir["system_info"]>);
  },
  catalog() {
    return read("forms_catalog", () => bir.forms_catalog(), []);
  },
  /**
   * The form library's index (code, title, frequency ...). Built once (about a second in a debug build, a sync host call) and kept
   * for the run, past bump(): the library never changes while the app runs. The shell warms it after the first screen is drawn.
   * Reactive: what shows it asks again when it fills after a failure. A failed or empty build answers [] and is logged once; the
   * host is not asked again until the retry is due (2 s, then 4 s, 8 s ... a minute, as a failed read), when what shows it re-runs.
   */
  screenIndex(): FormIndex {
    formIndexRev();
    if (formIndex) return formIndex;
    if (formIndexFail.waiting) return NO_INDEX;
    try {
      const rows = bir.screen_forms_index();
      if (!rows.length) throw new Error("The form library came back empty.");
      formIndex = rows;
      if (formIndexFail.count > 0) {
        // What showed [] shows the forms now.
        if (formIndexFail.retry !== undefined) clearTimeout(formIndexFail.retry);
        formIndexFail.retry = undefined;
        setTimeout(() => formIndexRev.update((n) => n + 1), 0);
      }
      formIndexFail.waiting = false;
      formIndexFail.count = 0;
      formIndexFail.error = undefined;
      return rows;
    } catch (error) {
      formIndexFailed(error);
      return NO_INDEX;
    }
  },
  /** RDO choices as { value, label }. Static, so kept past bump(). */
  rdoOptions(): { value: string; label: string }[] {
    if (!rdoList) {
      try {
        rdoList = bir.list_rdos().map((r) => ({ value: r.code, label: `${r.code} - ${r.description}` }));
      } catch {
        return [];
      }
    }
    return rdoList;
  },
  /** ZIP choices as { value, label }. Static, so kept past bump(). */
  zipOptions(): { value: string; label: string }[] {
    if (!zipList) {
      try {
        zipList = bir.list_zip_codes().map((z) => ({ value: z.split(" - ")[0].trim(), label: z }));
      } catch {
        return [];
      }
    }
    return zipList;
  },
  /** The taxpayers the session may see: one after a taxpayer sign-in, else all. */
  taxpayers() {
    const all = read("taxpayers", () => bir.list_taxpayers(), []);
    const s = scope();
    return s ? all.filter((t) => t.tin_root === s) : all;
  },
  yearBounds() {
    return read("tax_year_bounds", () => bir.tax_year_bounds(), { min: 2010, max: new Date().getFullYear() + 1 });
  },
  taxYears(tin: string | null | undefined): number[] {
    return tin ? read(`years:${tin}`, () => bir.list_tax_years(tin), []) : [];
  },
  yearlyForms(tin: string | null | undefined, year: number | null | undefined): string[] {
    return tin && year ? read(`forms:${tin}:${year}`, () => bir.yearly_forms(tin, year), []) : [];
  },
  templates(tin: string | null | undefined) {
    return tin ? read(`templates:${tin}`, () => bir.list_form_templates(tin), []) : [];
  },
  /** The setups offered to a taxpayer type: built-in first, then the user's. */
  setups(taxpayerType: Parameters<Bir["list_setups"]>[0] | null | undefined) {
    return taxpayerType ? read(`setups:${taxpayerType}`, () => bir.list_setups(taxpayerType), []) : [];
  },
  /** One branch's facts for a tax year, who holds each and which forms read them; null when the read failed. */
  yearFacts(tin: string | null | undefined, year: number | null | undefined) {
    return tin && year ? read<ReturnType<Bir["year_facts"]> | null>(`year_facts:${tin}:${year}`, () => bir.year_facts(tin, year), null) : null;
  },
  /** Annual income tax returns among codes that stand for one another, as groups. */
  annualConflicts(codes: Iterable<string>): string[][] {
    const list = [...codes].sort();
    return list.length > 1 ? read(`annual_conflicts:${list.join(",")}`, () => bir.annual_return_conflicts(list), []) : [];
  },
  /** The tax types the year's forms list gives, as chips. */
  taxTypes(tin: string | null | undefined, year: number | null | undefined) {
    return tin && year ? read(`tax_types:${tin}:${year}`, () => bir.tax_types_for_year(tin, year), []) : [];
  },
  /** Standing send OKs for one branch. */
  sendGrants(tin: string | null | undefined) {
    return tin ? read(`grants:${tin}`, () => bir.list_send_grants(tin), []) : [];
  },
  /** Confirmation-inbox state for one branch profile; null when the read failed. */
  inboxState(tin: string | null | undefined) {
    return tin ? read<ReturnType<Bir["inbox_state"]> | null>(`inbox:${tin}`, () => bir.inbox_state(tin), null) : null;
  },
  /** Whether the tax profile can record the year's income-tax election, and if not what it lacks; null when the read failed. */
  electionReadiness(tin: string | null | undefined, year: number | null | undefined) {
    return tin && year
      ? read<ReturnType<Bir["income_tax_election_readiness"]> | null>(`election_ready:${tin}:${year}`, () => bir.income_tax_election_readiness(tin, year), null)
      : null;
  },
  deadlines(from: string, to: string) {
    return read(`deadlines:${from}:${to}`, () => bir.deadlines_between(from, to), []).filter((d) => store.canSee(d.tin_root));
  },
  /**
   * One branch's deadlines for one form and tax year, in date order. The deadlines between from and to are grouped once
   * per data version, so a list that asks this per row pays a map lookup per row (the Forms tab asks for each form).
   */
  formDeadlines(tin: string, code: string, year: number, from: string, to: string) {
    const source = `deadlines:${from}:${to}`;
    const index = read(`deadlines_by_form:${from}:${to}`, () => toIndex(read(source, () => bir.deadlines_between(from, to), [])), new Map<string, ReturnType<Bir["deadlines_between"]>>());
    return (index.get(`${tin}|${code}|${year}`) ?? []).filter((d) => store.canSee(d.tin_root));
  },
  /** The as-needed forms each branch files in year, with their returns that hold no trigger date yet. */
  asNeeded(year: number) {
    return read(`as_needed:${year}`, () => bir.as_needed_forms(year), []).filter((f) => store.canSee(f.tin_root));
  },
  returns(which: "filed" | "saved" | "all") {
    return read(`returns:${which}`, () => bir.list_returns(which), []).filter((r) => store.canSee(r.tin_root));
  },
  /** The saved (Draft) returns, for Home's Drafts count: outside the 30 s poll's returns sweep; Home forgets it on each visit. */
  savedReturns() {
    return read(SAVED_RETURNS_KEY, () => bir.list_returns("saved"), []).filter((r) => store.canSee(r.tin_root));
  },
  authState() {
    return read("auth_state", () => bir.auth_state(), {
      mode: null,
      admin_root: null,
      has_admin_pin: false,
      has_admin_totp: false,
      has_recovery_key: false,
      mac_login_available: false,
    } as ReturnType<Bir["auth_state"]>);
  },
  /** PIN and authenticator of a taxpayer (shared by all its branches). */
  taxpayerCreds(root: string) {
    return read(`tpauth:${root}`, () => bir.taxpayer_auth(root), { has_pin: false, has_totp: false });
  },
  backgroundJobs() {
    return read("jobs", () => bir.list_background_jobs(), []);
  },
  setting(key: string): string | null {
    return read(`setting:${key}`, () => bir.get_setting(key), null);
  },
};

/** The type of the store, for screens that take it as a parameter. */
export type Store = typeof store;
