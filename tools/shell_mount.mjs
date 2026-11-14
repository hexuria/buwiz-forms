// Mounts the real app/main.js with app/shell/chrome.js, ui.js, recent.js and
// theme.js on a mocked kit and host, for the chrome tests (title bar, Recent,
// shortcuts). The screens behind the chrome are stubs. The demo taxpayer is
// Juana Santos, TIN 000-123-453-00000.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { kitMethods, strictElement } from "./kit_surface.mjs";

const root = new URL("../", import.meta.url);
const KIT = kitMethods(root);

// A mock element records every call the screen made on it.
const meta = new WeakMap();
function el() {
  const e = strictElement(KIT, (k, args, self) => {
    const m = meta.get(self);
    if (k === "child") m.kids.push(args[0]);
    else if (k === "children") m.kids.push(...args[0]);
    else m.calls.push([k, ...args]);
    if (k === "id") m.id = String(args[0]);
    if (k === "when" && args[0]) args[1](self);
    if (k === "when_some" && args[0] != null) args[1](self, args[0]);
  });
  meta.set(e, { id: null, calls: [], kids: [] });
  return e;
}

/** Every recorded node under `e`, depth first. */
export function nodes(e, out = []) {
  const m = meta.get(e);
  if (!m) return out;
  out.push(m);
  for (const k of m.kids) nodes(k, out);
  return out;
}
export const byId = (tree, id) => nodes(tree).filter((m) => m.id === id);
export function one(tree, id) {
  const all = byId(tree, id);
  assert.equal(all.length, 1, `exactly one #${id}`);
  return all[0];
}
/** The ids under a node, in order. */
export function ids(m, out = []) {
  if (m.id) out.push(m.id);
  for (const k of m.kids) if (meta.get(k)) ids(meta.get(k), out);
  return out;
}
/** The last value a node was given for `name` (its tooltip, say). */
export function arg(m, name) {
  const call = m.calls.filter((c) => c[0] === name).at(-1);
  return call ? call[1] : undefined;
}
export function click(tree, id, cx) {
  const call = one(tree, id).calls.find((c) => c[0] === "on_click");
  assert.ok(call, `#${id} has a click handler`);
  call[1]({}, cx);
}
/** Run the app root's handler for a script action, as GPUI would; false when none claims it. */
export function dispatch(tree, action, cx) {
  const handler = one(tree, "app-root").calls.find((c) => c[0] === "on_action" && c[1] === action);
  if (!handler) return false;
  handler[2]({ action }, cx);
  return true;
}

// The host window.
let viewport = { width: 1000, height: 800 };
export const storage = new Map();
const windowStub = {
  viewport_size: () => viewport,
  localStorage: { getItem: (k) => storage.get(k) ?? null, setItem: (k, v) => storage.set(k, v) },
};

const context = vm.createContext({ console, Map, Set, JSON, Date, Math, Number, String, Proxy, WeakMap, window: windowStub });
const mock = (name, exports) =>
  new vm.SyntheticModule(Object.keys(exports), function () {
    for (const [k, v] of Object.entries(exports)) this.setExport(k, v);
  }, { context, identifier: name });
const source = (path) => new vm.SourceTextModule(fs.readFileSync(new URL(path, root), "utf8"), { context, identifier: path });
const stub = () => el();
const stubClass = class {
  constructor() {
    return new Proxy(this, { get: (o, k) => (k in o ? o[k] : () => null) });
  }
};
const InputState = {
  new: () => {
    let text = "";
    return { value: () => text, set_value: (v) => (text = v), on() {} };
  },
};
class View {}
// The Calendar and Filings filter bars as main.js uses them: Esc closes an
// open combo first, and Filings takes a Home tile's preset once.
class CalendarFilters {
  constructor() {
    this.open = false;
  }
  dismiss(cx) {
    if (!this.open) return false;
    this.open = false;
    cx.notify();
    return true;
  }
}
class FilingsFilters extends CalendarFilters {
  applyPreset(app) {
    if (!app.filingsPreset) return;
    this.preset = app.filingsPreset;
    app.filingsPreset = null;
  }
}

const real = {
  "./shell/store.js": source("app/shell/store.js"),
  "./shell/theme.js": source("app/shell/theme.js"),
  "./shell/ui.js": source("app/shell/ui.js"),
  "./shell/chrome.js": source("app/shell/chrome.js"),
  "./shell/facts.js": source("app/shell/facts.js"),
  "./shell/chrome_gate.js": source("app/shell/chrome_gate.js"),
  "./shell/recent.js": source("app/shell/recent.js"),
};
const periods = source("app/shell/periods.js");
const linked = {
  "gpui-kit": mock("gpui-kit", { View, div: el, svg: el }),
  "gpui-base": mock("gpui-base", { Input: { new: stub }, InputState, Popup: { new: stub }, OtpInput: { new: stub }, OtpState: InputState, set_theme() {} }),
  process: mock("process", { default: { env: {} } }),
  bir: mock("bir", {}),
  "./shell/home.js": mock("home", { renderHome: stub }),
  "./shell/library.js": mock("library", { renderLibrary: stub }),
  "./shell/calendar_page.js": mock("calendar", { renderCalendar: stub, CalendarFilters }),
  "./shell/filings.js": mock("filings", { renderFilings: stub, FilingsFilters }),
  "./shell/settings.js": mock("settings", { renderSettings: stub }),
  "./shell/taxpayer.js": mock("taxpayer", { renderTaxpayerPage: stub, renderNewTaxpayer: stub, TaxpayerActions: stubClass }),
  "./shell/form_page.js": mock("form_page", {
    renderFormPage: stub,
    renderFormSplit: () => null,
    formColumnBox: () => null,
    formScrollArea: (_app, id, plain) => plain(stub().id(id)).overflow_y_scroll().overflow_x_hidden(),
    renderExistingOffer: stub,
    renderRemovalConfirm: stub,
    renderQueueConfirm: stub,
    renderSamplePeriod: stub,
    FormPageActions: stubClass,
  }),
  "./shell/auth.js": mock("auth", { renderSignIn: stub, renderSetup: stub }),
  "./shell/gate.js": mock("gate", { renderAccountGate: stub }),
  "./shell/update.js": mock("update", { installReadyUpdate() {}, pollUpdateStatus: () => false, renderUpdatePrompt: (_cx, _app, page) => page, setAutoUpdateDownload() {} }),
  "./shell/security.js": mock("security", { SecurityActions: stubClass }),
  "./shell/account.js": mock("account", { AccountActions: stubClass }),
  "./shell/setup_change.js": mock("setup_change", { SetupChange: stubClass }),
  "./shell/cron_tasks.js": mock("cron_tasks", { renderCronTasksPage: stub }),
};
// chrome.js and its neighbours import each other as "./x.js".
for (const [k, v] of Object.entries(real)) {
  linked[k] = v;
  linked[k.replace("./shell/", "./")] = v;
}
linked["./periods.js"] = periods;
linked["./todo.js"] = source("app/shell/todo.js");
const main = source("app/main.js");
await main.link((s) => {
  if (!linked[s]) throw new Error(`unlinked import ${s}`);
  return linked[s];
});
await main.evaluate();

export const ui = real["./shell/ui.js"].namespace;
export const recent = real["./shell/recent.js"].namespace;

// BirRoot builds BirApp through cx.new; take the class from there.
let BirApp = null;
new main.namespace.default().init({}, { new: (Class) => (BirApp = Class) });
assert.ok(BirApp, "BirApp class");

export const cx = {
  notify() {},
  theme: () => ({ is_dark: false }),
  timer: { after() {}, every() {} },
};

export const TIN = "00012345300000";
const taxpayer = {
  tin_root: "000123453",
  name: "Juana Santos",
  tin_display: "000-123-453-00000",
  branches: [{ code: "00000", name: "Head office", tin: TIN, is_head_office: true, facts: { registered_address: "", rdo_code: "" } }],
};
const store = {
  taxpayers: () => [taxpayer],
  authState: () => ({ mode: "solo" }),
  returns: () => [],
  taxYears: () => [2026],
  yearlyForms: () => ["1601C", "2551Q", "1701Q", "0619E", "1601EQ"],
  templates: () => [],
  taxTypes: () => [],
  annualConflicts: () => [],
  yearFacts: () => null,
  electionReadiness: () => null,
  screenIndex: () => [{ c: "1601C", freq: "Monthly" }, { c: "2551Q", freq: "Quarterly" }],
  logScreen() {},
  // Cached reads main.js drops (Home's Drafts count on each visit).
  forgotten: [],
  forget(key) {
    this.forgotten.push(key);
  },
  call() {},
  write() {},
};

/** A signed-in app on the main window. `os`: "macos", "windows" or "linux". */
export function makeApp({ width = 1000, os = "windows", view = "home", recentList = [] } = {}) {
  viewport = { width, height: 800 };
  const app = Object.create(BirApp.prototype);
  Object.assign(app, {
    store,
    info: { os, database_path: "/demo/buwiz.db" },
    booted: true,
    cloudLocked: false,
    appLocked: false,
    setup: { step: "choose" },
    setupPending: null,
    view,
    history: [],
    forward: [],
    sidebarOpen: width >= 860,
    panel: null,
    lastPanel: "profile",
    isMac: os === "macos",
    isLinux: os === "linux",
    platformKnown: true,
    recent: recentList,
    prefs: { showRecent: true },
    accountMenu: false,
    settingsMenu: null,
    overlay: null,
    toastState: null,
    themePref: "light",
    readIds: new Set(),
    session: { kind: "open" },
    selRoot: taxpayer.tin_root,
    selCode: "00000",
    selYear: 2026,
    wasNarrow: width < 860,
    focusRoot: { focus() {} },
    palette: { input: InputState.new(), index: 0 },
    taxpayerUi: { panelEdit: null, reset() {}, leaveView() {}, leaveYear() {} },
    // The form page is a stub: opening a return keeps its options as the state.
    formUi: {
      state: null,
      open(opts) {
        this.state = { ...opts };
      },
      leavingPage() {},
      askingRemoval: () => false,
    },
    security: { leave() {} },
    setupChange: { leave() {} },
    calendarUi: new CalendarFilters(),
    filingsUi: new FilingsFilters(),
  });
  return app;
}
