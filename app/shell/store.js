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

export class Store {
  constructor() {
    this.version = 0;
    this.cache = new Map();
    /** Last error per cache key, for screens that show one. */
    this.errors = new Map();
    /**
     * Which taxpayers this session may see. `scopeRoot` (a TIN root) narrows
     * the taxpayer list to one taxpayer for a taxpayer sign-in; `canSee(root)`
     * also drops locked taxpayers' deadlines and returns, which name them.
     */
    /** @type {string | null} */
    this.scopeRoot = null;
    /** @type {(root: string) => boolean} */
    this.canSee = (_root) => true;
  }

  /** Drop every cached read. Call after any write. */
  bump() {
    this.version += 1;
    this.cache.clear();
    this.errors.clear();
  }

  call(name, ...args) {
    const fn = bir[name];
    if (typeof fn !== "function") throw new Error(`This build has no bir.${name}`);
    return fn(...args);
  }

  /** Memoised read: `fallback` when the call throws; the error is kept. */
  read(key, name, args, fallback) {
    if (this.cache.has(key)) return this.cache.get(key);
    let value = fallback;
    try {
      value = this.call(name, ...args);
      this.errors.delete(key);
    } catch (e) {
      this.errors.set(key, String(e));
    }
    this.cache.set(key, value);
    return value;
  }

  /** Drop one cached read so the next render reads it again. */
  forget(key) {
    this.cache.delete(key);
  }

  /** A write: runs the call, bumps the version, rethrows errors as Error. */
  write(name, ...args) {
    try {
      return this.call(name, ...args);
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
  deadlines(from, to) {
    return this.read(`deadlines:${from}:${to}`, "deadlines_between", [from, to], []).filter((d) => this.canSee(d.tin_root));
  }
  returns(which) {
    return this.read(`returns:${which}`, "list_returns", [which], []).filter((r) => this.canSee(r.tin_root));
  }
  authState() {
    return this.read("auth_state", "auth_state", [], {
      enable_profile_pins: false,
      has_admin_totp: false,
      has_admin_pin: false,
      hide_tax_profiles: false,
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
