// Cached reads over the `bir` host module.
//
// Render must not hit the database (layout lesson from the ebirforms build:
// synchronous reads on every render cost more than drawing). Every read here
// is memoised until the next write bumps the data version; every write goes
// through `write`, which bumps it.
//
// The module is imported as a namespace so a host function this build lacks
// reads as a clear error on use rather than failing the whole app at link.

import * as bir from "bir";

/**
 * A read that failed is answered from its fallback for this long, then tried
 * again: one transient failure (a database another process holds locked)
 * must not show empty data until the next write.
 */
const FAILED_READ_RETRY_MS = 2000;

/** The text of a host error: "`bir.fn`: text" arrives; the person needs only the text. */
export function errorText(e) {
  return String(e && e.message ? e.message : e)
    .replace(/^Error:\s*/, "")
    .replace(/^`[^`]+`:\s*/, "");
}

export class Store {
  constructor() {
    this.version = 0;
    this.cache = new Map();
    /** Last error per cache key, for screens that show one. */
    this.errors = new Map();
    /** When each failed read failed (`Date.now()`), so it is tried again. */
    this.failedAt = new Map();
    /**
     * Which taxpayers this session may see. `scopeRoot` (a TIN root) narrows
     * the taxpayer list to one taxpayer for a taxpayer sign-in; `canSee(root)`
     * also drops locked taxpayers' deadlines and returns, which name them.
     */
    /** @type {string | null} */
    this.scopeRoot = null;
    /** @type {(root: string) => boolean} */
    this.canSee = (_root) => true;
    /** @type {{ value: string, label: string }[] | null} */
    this.rdoList = null;
    /** @type {{ value: string, label: string }[] | null} */
    this.zipList = null;
  }

  /**
   * A screen failure for the daily log and the Logs tab. `console.*` also
   * lands there (`gpui_shell::script`); this is the path that does not go
   * through `console`. Logging must not hide the original failure.
   */
  logScreen(level, message) {
    const text = String(message);
    try {
      if (typeof bir.record_screen_log === "function") {
        bir.record_screen_log(level, text);
        return;
      }
    } catch {
      // Fall through to console, which the shell subscriber also records.
    }
    if (level === "error") console.error(text);
    else if (level === "warn" || level === "warning") console.warn(text);
    else console.log(text);
  }

  /** Drop every cached read. Call after any write. */
  bump() {
    this.version += 1;
    this.cache.clear();
    this.errors.clear();
    this.failedAt.clear();
  }

  call(name, ...args) {
    const fn = bir[name];
    if (typeof fn !== "function") throw new Error(`This build has no bir.${name}`);
    return fn(...args);
  }

  /**
   * Memoised read: `fallback` when the call throws; the error is kept, and
   * the call is tried again on the first read after FAILED_READ_RETRY_MS.
   */
  read(key, name, args, fallback) {
    if (this.cache.has(key)) {
      const failed = this.failedAt.get(key);
      if (failed === undefined || Date.now() - failed < FAILED_READ_RETRY_MS) return this.cache.get(key);
    }
    let value = fallback;
    try {
      value = this.call(name, ...args);
      this.errors.delete(key);
      this.failedAt.delete(key);
    } catch (e) {
      const text = String(e);
      if (this.errors.get(key) !== text) this.logScreen("error", `bir.${name}: ${errorText(e)}`);
      this.errors.set(key, text);
      this.failedAt.set(key, Date.now());
    }
    this.cache.set(key, value);
    return value;
  }

  /** True when the last read of `key` failed and holds its fallback. */
  failed(key) {
    return this.failedAt.has(key);
  }

  /** Drop one cached read so the next render reads it again. */
  forget(key) {
    this.cache.delete(key);
    this.failedAt.delete(key);
  }

  /** Drop every cached read whose key `test` accepts. */
  forgetWhere(test) {
    for (const key of [...this.cache.keys()]) if (test(key)) this.forget(key);
  }

  /** A write: runs the call, bumps the version, rethrows errors as Error. */
  write(name, ...args) {
    try {
      return this.call(name, ...args);
    } catch (e) {
      this.logScreen("error", `bir.${name}: ${errorText(e)}`);
      throw e;
    } finally {
      this.bump();
    }
  }

  // ------------------------------------------------------------ reads

  systemInfo() {
    return this.read("system_info", "system_info", [], { os: "macos" });
  }
  catalog() {
    return this.read("forms_catalog", "forms_catalog", [], []);
  }
  screenIndex() {
    return this.read("screen_forms_index", "screen_forms_index", [], []);
  }
  /** RDO and ZIP choices as `{ value, label }`. Static, so kept past `bump()`. */
  rdoOptions() {
    if (!this.rdoList) {
      try {
        this.rdoList = this.call("list_rdos").map((r) => ({ value: r.code, label: `${r.code} - ${r.description}` }));
      } catch (_e) {
        return [];
      }
    }
    return this.rdoList;
  }
  zipOptions() {
    if (!this.zipList) {
      try {
        this.zipList = this.call("list_zip_codes").map((z) => ({ value: z.split(" - ")[0].trim(), label: z }));
      } catch (_e) {
        return [];
      }
    }
    return this.zipList;
  }
  taxpayers() {
    const all = this.read("taxpayers", "list_taxpayers", [], []);
    return this.scopeRoot ? all.filter((t) => t.tin_root === this.scopeRoot) : all;
  }
  yearBounds() {
    const now = new Date().getFullYear();
    return this.read("tax_year_bounds", "tax_year_bounds", [], { min: 2010, max: now + 1 });
  }
  taxYears(tin) {
    return tin ? this.read(`years:${tin}`, "list_tax_years", [tin], []) : [];
  }
  yearlyForms(tin, year) {
    return tin && year ? this.read(`forms:${tin}:${year}`, "yearly_forms", [tin, year], []) : [];
  }
  templates(tin) {
    return tin ? this.read(`templates:${tin}`, "list_form_templates", [tin], []) : [];
  }
  starterPacks(tin) {
    return tin ? this.read(`packs:${tin}`, "starter_packs", [tin], []) : [];
  }
  /** Standing send OKs for one branch: `{ tin, form, granted_at }`. */
  sendGrants(tin) {
    return tin ? this.read(`grants:${tin}`, "list_send_grants", [tin], []) : [];
  }
  /** Confirmation-inbox state for one branch profile (P2-5). */
  inboxState(tin) {
    return tin
      ? this.read(`inbox:${tin}`, "inbox_state", [tin], {
          tin,
          inbox_email: "",
          auth_method: "AppPassword",
          tracking_enabled: false,
          imap_host: "imap.gmail.com",
          has_app_password: false,
          oauth_connected: false,
          oauth_inbox_email: null,
          oauth_updated_at: null,
          still_pending: false,
          matching_tins: [],
          last_check_at: null,
          last_check_status: null,
          last_check_log: null,
        })
      : null;
  }
  deadlines(from, to) {
    return this.read(`deadlines:${from}:${to}`, "deadlines_between", [from, to], []).filter((d) => this.canSee(d.tin_root));
  }
  /**
   * One branch's deadlines for one form and tax year, in date order. The
   * deadlines between `from` and `to` are grouped once per data version, so
   * a list that asks this per row pays a map lookup per row rather than a
   * scan of every taxpayer's deadlines (the Forms tab asks for each form).
   */
  formDeadlines(tin, code, year, from, to) {
    const key = `deadlines_by_form:${from}:${to}`;
    const source = `deadlines:${from}:${to}`;
    let index = this.cache.get(key);
    if (!index) {
      index = new Map();
      for (const d of this.read(source, "deadlines_between", [from, to], [])) {
        const k = `${d.tin}|${d.form_code}|${d.taxable_year}`;
        const rows = index.get(k);
        if (rows) rows.push(d);
        else index.set(k, [d]);
      }
      // An index of a failed read stays uncached, so the retry rebuilds it.
      if (!this.failed(source)) this.cache.set(key, index);
    }
    return (index.get(`${tin}|${code}|${year}`) || []).filter((d) => this.canSee(d.tin_root));
  }
  /** The as-needed forms each branch files in `year`, with their returns that hold no trigger date yet. */
  asNeeded(year) {
    return this.read(`as_needed:${year}`, "as_needed_forms", [year], []).filter((f) => this.canSee(f.tin_root));
  }
  returns(which) {
    return this.read(`returns:${which}`, "list_returns", [which], []).filter((r) => this.canSee(r.tin_root));
  }
  authState() {
    return this.read("auth_state", "auth_state", [], {
      mode: null,
      admin_root: null,
      has_admin_pin: false,
      has_admin_totp: false,
      has_recovery_key: false,
      mac_login_available: false,
    });
  }
  backgroundJobs() {
    return this.read("jobs", "list_background_jobs", [], []);
  }
  setting(key) {
    return this.read(`setting:${key}`, "get_setting", [key], null);
  }
}

/** ISO date (YYYY-MM-DD) in local time. */
export function isoDate(date) {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

export function addDays(date, days) {
  const next = new Date(date.getTime());
  next.setDate(next.getDate() + days);
  return next;
}

/** Whole days from today (local midnight) to an ISO date. */
export function daysUntil(iso) {
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const [y, m, d] = iso.split("-").map(Number);
  return Math.round((new Date(y, m - 1, d).getTime() - today.getTime()) / 86400000);
}

export const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
export const MONTHS_LONG = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

/** "Oct 9, 2026" from an ISO date or a SQLite timestamp. */
export function shortDate(text) {
  if (!text) return "";
  // SQLite timestamps ("2026-10-14 22:10:03") are UTC; show the local day.
  const ts = /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2})/.exec(String(text));
  if (ts) {
    const local = new Date(Date.UTC(+ts[1], +ts[2] - 1, +ts[3], +ts[4], +ts[5]));
    return `${MONTHS[local.getMonth()]} ${local.getDate()}, ${local.getFullYear()}`;
  }
  const [datePart] = String(text).split(/[ T]/);
  const [y, m, d] = datePart.split("-").map(Number);
  if (!y || !m || !d) return String(text);
  return `${MONTHS[m - 1]} ${d}, ${y}`;
}

/** "009-123-456" from a 9-digit root; "009-123-456-00000" with a branch code. */
export function formatTin(root, branch) {
  const r = String(root || "").replace(/\D/g, "");
  const base = r.length >= 9 ? `${r.slice(0, 3)}-${r.slice(3, 6)}-${r.slice(6, 9)}` : r;
  return branch ? `${base}-${branch}` : base;
}

/** The code people see: 1601C → 1601-C where BIR writes it that way. */
export function displayCode(code) {
  return String(code || "");
}
