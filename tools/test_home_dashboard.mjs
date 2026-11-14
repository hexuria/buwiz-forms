// Home's dashboard (round-two plan, claim 5), driven through the real
// app/main.js navigation and poll, app/shell/home.js, dashboard.js, store.js
// and ui.js with a mocked kit and a fake `bir` host holding demo data.
// "Today" is pinned to Friday, November 13, 2026, so no count depends on
// the clock.
//
// Checks: each tile's number and the host call it comes from (zero and
// overdue cases included), the 30-day strip's days and counts, the
// per-taxpayer progress and next deadline, navigation from the tiles, the
// strip and the rows, the three Filings tiles' presets reaching the real
// Filings filters through main.js go(), the empty state of a new install, and that the 30 s
// poll does not ask list_returns("saved") again while a visit to Home does.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { kitMethods, strictElement } from "./kit_surface.mjs";

const root = new URL("../", import.meta.url);
const KIT = kitMethods(root);

// ---------------------------------------------------------------- mock kit

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
function nodes(e, out = []) {
  const m = meta.get(e);
  if (!m) return out;
  out.push(m);
  for (const k of m.kids) nodes(k, out);
  return out;
}
const sub = (m) => {
  const out = [m];
  for (const k of m.kids) nodes(k, out);
  return out;
};
const byId = (tree, id) => nodes(tree).filter((m) => m.id === id);
const one = (tree, id) => {
  const all = byId(tree, id);
  assert.equal(all.length, 1, `exactly one #${id}`);
  return all[0];
};
const has = (m, name, ...args) => m.calls.some((c) => c[0] === name && args.every((a, i) => c[i + 1] === a));
const arg = (m, name) => (m.calls.find((c) => c[0] === name) || [])[1];
/** Every string the node shows, in order. */
function texts(m, out = []) {
  for (const k of m.kids) {
    if (typeof k === "string") out.push(k);
    else if (meta.get(k)) texts(meta.get(k), out);
  }
  return out;
}
const kidsOf = (m) => m.kids.map((k) => meta.get(k)).filter(Boolean);

// ---------------------------------------------------------------- pinned clock

const NOW = new Date(2026, 10, 13, 10, 0, 0).getTime();
class PinnedDate extends Date {
  constructor(...a) {
    if (a.length) super(...a);
    else super(NOW);
  }
  static now() {
    return NOW;
  }
}

// ---------------------------------------------------------------- fake host

const JUANA = { root: "000123453", tin: "00012345300000", name: "Juana Santos" };
const SARI = { root: "000987654", tin: "00098765400000", name: "Santos Sari-Sari Store" };
const LAKBAY = { root: "000222333", tin: "00022233300000", name: "Lakbay Travel Agency" };
const taxpayer = (p, archived = false) => ({
  tin_root: p.root,
  name: p.name,
  tin_display: `${p.root.slice(0, 3)}-${p.root.slice(3, 6)}-${p.root.slice(6)}`,
  archived,
  branches: [{ code: "00000", name: "Head office", tin: p.tin, tin_root: p.root, is_head_office: true, facts: {} }],
});
const who = (p) => ({ tin: p.tin, tin_root: p.root, branch_code: "00000", branch_name: "Head office", taxpayer_name: p.name, branch_count: 1 });
let nextId = 1;
const dl = (p, code, year, period, label, due, status) => ({
  ...who(p),
  form_code: code,
  form_name: code,
  frequency: "Monthly",
  taxable_year: year,
  period,
  period_label: label,
  due_date: due,
  status,
  draft_id: status === "Not started" ? null : nextId,
  return_key: status === "Not started" ? null : nextId++,
});
const ret = (p, code, label, status, updated) => ({ id: nextId++, ...who(p), form_code: code, taxable_year: 2026, period: 1, period_label: label, status, updated_at: updated, return_key: null });

const DEMO = {
  taxpayers: [taxpayer(JUANA), taxpayer(SARI), taxpayer(LAKBAY, true)],
  deadlines: [
    // More than 366 days back: not on the Overdue tile.
    dl(JUANA, "1601C", 2025, 10, "October 2025", "2025-11-10", "Not started"),
    // Overdue: open and past due within the last 366 days.
    dl(JUANA, "1601C", 2025, 11, "November 2025", "2025-12-10", "Not started"),
    dl(JUANA, "1701", 2025, 0, "TY 2025", "2026-04-15", "Confirmed"),
    dl(JUANA, "1601C", 2026, 9, "September 2026", "2026-10-12", "Submitted"),
    dl(JUANA, "2551Q", 2026, 3, "Q3 2026", "2026-10-26", "Draft"),
    dl(JUANA, "1601C", 2026, 10, "October 2026", "2026-11-10", "Not started"),
    dl(SARI, "1601C", 2026, 9, "September 2026", "2026-10-12", "Confirmed"),
    dl(SARI, "1601C", 2026, 10, "October 2026", "2026-11-10", "Confirmed"),
    // The next 7 days.
    dl(SARI, "0605", 2026, 0, "TY 2026", "2026-11-13", "Draft"),
    dl(JUANA, "1701Q", 2026, 3, "Q3 2026", "2026-11-16", "Not started"),
    dl(SARI, "1600", 2026, 10, "October 2026", "2026-11-17", "Submitted"),
    dl(LAKBAY, "1601C", 2026, 10, "October 2026", "2026-11-18", "Not started"),
    // Later in the strip; Queued is not open.
    dl(JUANA, "0619E", 2026, 11, "November 2026", "2026-12-10", "Not started"),
    dl(JUANA, "1601C", 2026, 11, "November 2026", "2026-12-10", "Not started"),
    dl(SARI, "1601C", 2026, 11, "November 2026", "2026-12-10", "Queued"),
    // Past the strip, within 60 days.
    dl(SARI, "1601C", 2026, 12, "December 2026", "2027-01-11", "Not started"),
  ],
  saved: [ret(JUANA, "2551Q", "Q3 2026", "Draft", "2026-11-01 02:00:00"), ret(SARI, "0605", "TY 2026", "Draft", "2026-11-12 02:00:00")],
  filed: [
    ret(SARI, "1601C", "November 2026", "Queued", "2026-11-12 03:00:00"),
    ret(SARI, "1600", "October 2026", "Submitted", "2026-11-12 02:00:00"),
    ret(SARI, "1601C", "October 2026", "Confirmed", "2026-11-11 02:00:00"),
    ret(JUANA, "1601C", "September 2026", "Submitted", "2026-10-13 02:00:00"),
    ret(JUANA, "1701", "TY 2025", "Paid", "2026-04-20 02:00:00"),
  ],
};
const EMPTY = { taxpayers: [taxpayer(JUANA)], deadlines: [], saved: [], filed: [] };
let host = DEMO;
const calls = [];
const screenLogs = [];
const birExports = {
  list_taxpayers: () => (calls.push(["list_taxpayers"]), host.taxpayers),
  deadlines_between: (from, to) => (calls.push(["deadlines_between", from, to]), host.deadlines.filter((d) => d.due_date >= from && d.due_date <= to)),
  list_returns: (which) => (calls.push(["list_returns", which]), which === "saved" ? host.saved : which === "filed" ? host.filed : [...host.saved, ...host.filed]),
  as_needed_forms: () => [],
  list_form_templates: () => [],
  record_screen_log: (level, text) => screenLogs.push([level, text]),
};
const count = (name, ...args) => calls.filter((c) => c[0] === name && args.every((a, i) => c[i + 1] === a)).length;

// ---------------------------------------------------------------- the shell

let viewport = { width: 1200, height: 800 };
const storage = new Map();
const windowStub = {
  viewport_size: () => viewport,
  localStorage: { getItem: (k) => storage.get(k) ?? null, setItem: (k, v) => storage.set(k, v) },
};
const context = vm.createContext({ console, Map, Set, JSON, Date: PinnedDate, Math, Number, String, Proxy, WeakMap, window: windowStub });
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
const InputState = { new: () => ({ value: () => "", set_value() {}, on() {} }) };
class View {}
// A popover holds its trigger, as the host's does.
const Popup = {
  new: (id, trigger) => {
    const e = el();
    e.id(id);
    meta.get(e).kids.push(trigger);
    return e;
  },
};

const real = {
  "./shell/store.js": source("app/shell/store.js"),
  "./shell/theme.js": source("app/shell/theme.js"),
  "./shell/ui.js": source("app/shell/ui.js"),
  "./shell/chrome.js": source("app/shell/chrome.js"),
  "./shell/chrome_gate.js": source("app/shell/chrome_gate.js"),
  "./shell/recent.js": source("app/shell/recent.js"),
  "./shell/home.js": source("app/shell/home.js"),
  "./shell/dashboard.js": source("app/shell/dashboard.js"),
  // Filings takes the tiles' presets; Calendar is made beside it at boot.
  "./shell/calendar_page.js": source("app/shell/calendar_page.js"),
  "./shell/filings.js": source("app/shell/filings.js"),
};
const linked = {
  "gpui-kit": mock("gpui-kit", { View, div: el, svg: el }),
  "gpui-base": mock("gpui-base", { Input: { new: stub }, InputState, Popup, OtpInput: { new: stub }, OtpState: InputState, set_theme() {} }),
  process: mock("process", { default: { env: {} } }),
  bir: mock("bir", birExports),
  "./shell/library.js": mock("library", { renderLibrary: stub }),
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
for (const [k, v] of Object.entries(real)) {
  linked[k] = v;
  linked[k.replace("./shell/", "./")] = v;
}
linked["./periods.js"] = source("app/shell/periods.js");
linked["./todo.js"] = source("app/shell/todo.js");
const main = source("app/main.js");
await main.link((s) => {
  if (!linked[s]) throw new Error(`unlinked import ${s}`);
  return linked[s];
});
await main.evaluate();
const { Store } = real["./shell/store.js"].namespace;
const { renderHome } = real["./shell/home.js"].namespace;
const { CalendarFilters } = real["./shell/calendar_page.js"].namespace;
const { FilingsFilters, renderFilings } = real["./shell/filings.js"].namespace;

let BirApp = null;
new main.namespace.default().init({}, { new: (Class) => (BirApp = Class) });
assert.ok(BirApp, "BirApp class");

let notified = 0;
const cx = {
  notify() {
    notified += 1;
  },
  theme: () => ({ is_dark: false }),
  timer: { after() {}, every() {} },
  stop_propagation() {},
  // The filter bars' combo boxes are retained views.
  new(Class, props) {
    const view = new Class();
    view.init(props, cx);
    return { __view: view, set_props: (p) => view.update?.(p), release: () => true };
  },
};

const opened = [];
function makeApp(data) {
  host = data;
  calls.length = 0;
  const app = Object.create(BirApp.prototype);
  Object.assign(app, {
    store: new Store(),
    view: "home",
    history: [],
    forward: [],
    sidebarOpen: true,
    panel: null,
    wasNarrow: false,
    accountMenu: false,
    toastState: null,
    readIds: new Set(),
    knownEvents: new Set(),
    selRoot: JUANA.root,
    selCode: "00000",
    selYear: 2026,
    taxpayerUi: { leaveView() {} },
    formUi: { leavingPage() {} },
    security: { leave() {} },
    setupChange: { leave() {} },
    invalidate() {},
    openForm(opts) {
      opened.push({ opts: JSON.parse(JSON.stringify(opts)), root: this.selRoot, code: this.selCode });
    },
  });
  // As boot() makes them.
  app.calendarUi = new CalendarFilters(app, cx);
  app.filingsUi = new FilingsFilters(app, cx);
  return app;
}
const click = (tree, id) => {
  const call = one(tree, id).calls.find((c) => c[0] === "on_click");
  assert.ok(call, `#${id} has a click handler`);
  call[1]({}, cx);
};
/** A tile's label, number and sub-line. */
const tile = (tree, key) => texts(one(tree, `home-tile-${key}`));
const dueRows = (tree) => kidsOf(one(tree, "home-due")).map((m) => m.id);
const dueKey = (d) => `due-${d.tin}-${d.form_code}-${d.taxable_year}-${d.period}`;
const D = (code, due, p = null) => DEMO.deadlines.find((d) => d.form_code === code && d.due_date === due && (!p || d.tin_root === p.root));

// ---------------------------------------------------------------- tiles
{
  const app = makeApp(DEMO);
  const home = renderHome(cx, app, 1000);
  assert.deepEqual(tile(home, "overdue"), ["Overdue", "3", "past the due date"]);
  assert.deepEqual(tile(home, "week"), ["Due in 7 days", "3", "by Nov 20"]);
  assert.deepEqual(tile(home, "drafts"), ["Drafts", "2", "started, not queued"]);
  assert.deepEqual(tile(home, "waiting"), ["Waiting to send", "1", "queued to send"]);
  assert.deepEqual(tile(home, "filed"), ["Filed this month", "2", "submitted in Nov"]);
  // A tile's label and caption wrap instead of being cut ("submitted in
  // Nove…"): five tiles across get 121 to 156 px, less 34 px of padding.
  for (const key of ["overdue", "week", "drafts", "waiting", "filed"]) {
    const words = sub(one(home, `home-tile-${key}`)).filter((m) => m.kids.some((k) => typeof k === "string") && m.calls.some((c) => c[0] === "text_size" || c[0] === "font_weight" || c[0] === "text_color"));
    assert.equal(words.length, 3, `${key}: label, number and caption`);
    for (const m of words) {
      assert.ok(!has(m, "truncate"), `${key}: "${m.kids.find((k) => typeof k === "string")}" is not cut`);
      assert.ok(has(m, "whitespace_normal") || /^\d+$/.test(m.kids.find((k) => typeof k === "string")), `${key}: "${m.kids.find((k) => typeof k === "string")}" wraps`);
    }
  }

  // The named host calls, and only those, feed the numbers.
  assert.equal(count("deadlines_between", "2025-11-12", "2026-11-20"), 1, "Overdue and Due in 7 days: today − 366 to today + 7");
  assert.equal(count("deadlines_between", "2026-11-13", "2026-12-12"), 1, "the strip: today to today + 29");
  assert.equal(count("deadlines_between", "2026-01-01", "2026-11-13"), 1, "progress: Jan 1 to today");
  assert.equal(count("deadlines_between", "2026-11-13", "2027-01-12"), 1, "next deadline: today to today + 60");
  assert.equal(count("deadlines_between"), 4);
  assert.equal(count("list_returns", "saved"), 1, "Drafts: list_returns(saved)");
  assert.equal(count("list_returns", "filed"), 1, "Waiting and Filed this month: list_returns(filed)");
  assert.equal(count("list_taxpayers"), 1);
  assert.deepEqual(screenLogs, [], "no host read failed");

  // The greeting line stays, with the pinned date.
  const header = texts(nodes(home)[0]);
  assert.ok(header.includes("Good morning"), `greeting: ${header}`);
  assert.ok(header.includes("Friday, November 13, 2026."), `date: ${header}`);

  // Five across on a wide pane, wrapped on a narrow one.
  assert.ok(has(one(home, "home-tile-overdue"), "w", Math.floor((1000 - 4 * 8) / 5)));
  const narrow = renderHome(cx, makeApp(DEMO), 400);
  assert.ok(has(one(narrow, "home-tile-overdue"), "w", Math.floor((400 - 8) / 2)));
}

// ---------------------------------------------------------------- strip
{
  const app = makeApp(DEMO);
  const home = renderHome(cx, app, 1000);
  const strip = one(home, "home-strip");
  const days = kidsOf(strip);
  assert.equal(days.length, 30, "30 days");
  assert.equal(days[0].id, "home-strip-2026-11-13");
  assert.equal(days[29].id, "home-strip-2026-12-12");
  const pips = (m) => kidsOf(m).filter((k) => has(k, "size", 8));
  const perDay = Object.fromEntries(days.map((m) => [m.id.slice("home-strip-".length), pips(m).length]).filter(([, n]) => n));
  assert.deepEqual(perDay, { "2026-11-13": 1, "2026-11-16": 1, "2026-11-18": 1, "2026-12-10": 2 }, "open deadlines per day; Submitted and Queued are not");
  assert.equal(arg(days[0], "accessibility_label"), "Today, Nov 13: 1 deadline");
  assert.equal(arg(one(home, "home-strip-2026-12-10"), "accessibility_label"), "Dec 10: 2 deadlines");
  assert.equal(arg(one(home, "home-strip-2026-11-17"), "accessibility_label"), "Nov 17: nothing due");
  // A draft is accent, within 7 days warning, later neutral.
  const tones = (iso) => pips(one(home, `home-strip-${iso}`)).map((m) => arg(m, "bg"));
  assert.deepEqual(tones("2026-11-13"), ["#2557d6"]);
  assert.deepEqual(tones("2026-11-16"), ["#a5520a"]);
  assert.deepEqual(tones("2026-12-10"), ["#71717a", "#71717a"]);

  // Nothing picked, nothing looks picked: no day is filled, and a hover only
  // outlines one (GPUI keeps a hover under a pointer that has not moved).
  for (const day of days) {
    assert.ok(!has(day, "bg"), `${day.id} is not filled before a pick`);
    const hover = day.calls.find((c) => c[0] === "hover");
    assert.ok(hover, `${day.id} answers a hover`);
    const styled = [];
    const style = new Proxy({}, { get: (_o, k) => (...a) => (styled.push(k), style) });
    hover[1](style);
    assert.ok(!styled.includes("bg"), `${day.id}: a hover does not fill the day`);
  }

  // With nothing picked the list shows every open deadline in the strip.
  assert.deepEqual(dueRows(home), [
    dueKey(D("0605", "2026-11-13")),
    dueKey(D("1701Q", "2026-11-16")),
    dueKey(D("1601C", "2026-11-18", LAKBAY)),
    dueKey(D("0619E", "2026-12-10")),
    dueKey(D("1601C", "2026-12-10", JUANA)),
  ]);

  // A day narrows it; the same day again shows all.
  click(home, "home-strip-2026-12-10");
  let again = renderHome(cx, app, 1000);
  assert.deepEqual(dueRows(again), [dueKey(D("0619E", "2026-12-10")), dueKey(D("1601C", "2026-12-10", JUANA))]);
  assert.ok(texts(nodes(again)[0]).includes("Thursday, December 10"));
  assert.ok(has(one(again, "home-strip-2026-12-10"), "bg", "#e2e2e7"), "the picked day is selected");
  click(again, "home-strip-2026-11-17");
  again = renderHome(cx, app, 1000);
  assert.deepEqual(dueRows(again), ["home-due-empty"]);
  assert.ok(texts(one(again, "home-due-empty")).includes("Nothing due this day."));
  click(again, "home-due-show-all");
  assert.equal(dueRows(renderHome(cx, app, 1000)).length, 5, "Show all");

  // A row in the list opens exactly its return.
  click(renderHome(cx, app, 1000), dueKey(D("0605", "2026-11-13")));
  assert.deepEqual(opened.pop(), { opts: { tin: SARI.tin, code: "0605", year: 2026, period: 0, returnKey: D("0605", "2026-11-13").return_key }, root: SARI.root, code: "00000" });

  // Open Calendar.
  click(renderHome(cx, app, 1000), "home-open-calendar");
  assert.equal(app.view, "calendar");
}

// ---------------------------------------------------------------- tile shortcuts
{
  const app = makeApp(DEMO);
  // Overdue and Due in 7 days narrow the list on Home and toggle off.
  click(renderHome(cx, app, 1000), "home-tile-overdue");
  let home = renderHome(cx, app, 1000);
  assert.equal(app.view, "home");
  assert.deepEqual(dueRows(home), [
    dueKey(D("1601C", "2025-12-10")),
    dueKey(D("2551Q", "2026-10-26")),
    dueKey(D("1601C", "2026-11-10", JUANA)),
  ]);
  assert.ok(has(one(home, "home-tile-overdue"), "bg", "#e2e2e7"), "the picked tile is selected");
  click(home, "home-tile-week");
  home = renderHome(cx, app, 1000);
  assert.deepEqual(dueRows(home), [dueKey(D("0605", "2026-11-13")), dueKey(D("1701Q", "2026-11-16")), dueKey(D("1601C", "2026-11-18", LAKBAY))]);
  click(home, "home-tile-week");
  assert.equal(app.homePick, null);

  // The other three open Filings through main.js go(), which hands their
  // preset to the Filings filters once.
  const cards = (statuses, month = null) =>
    [...DEMO.saved, ...DEMO.filed]
      .filter((r) => statuses.includes(r.status) && (!month || r.updated_at.startsWith(month)))
      .map((r) => `return-${r.id}`)
      .sort();
  const shown = (tree) =>
    nodes(tree)
      .map((m) => m.id)
      .filter((id) => id && /^return-\d+$/.test(id))
      .sort();
  const statusLabel = () => texts(one(app.filingsUi.statusCombo.entity.__view.render(cx), "filings-statuses-trigger")).join(" ");
  for (const [key, preset, summary, chip] of [
    ["drafts", { statuses: ["Draft"] }, "Draft", null],
    ["waiting", { statuses: ["Queued"] }, "Queued", null],
    ["filed", { statuses: ["Submitted", "Confirmed", "Paid"], updatedMonth: "2026-11" }, "3 statuses", "Changed in Nov 2026"],
  ]) {
    app.view = "home";
    app.filingsPreset = null;
    click(renderHome(cx, app, 1000), `home-tile-${key}`);
    assert.equal(app.view, "filings", `${key} opens Filings`);
    assert.equal(app.filingsPreset, null, `${key}: Filings used the preset once`);
    const f = app.filingsUi;
    assert.deepEqual([...f.statuses], preset.statuses, `${key} statuses`);
    assert.equal(f.updatedMonth, preset.updatedMonth ?? null, `${key} month`);
    const page = renderFilings(cx, app, 1000);
    assert.deepEqual(shown(page), cards(preset.statuses, preset.updatedMonth), `${key}: the board shows the preset's returns`);
    assert.ok(shown(page).length > 0, `${key}: some return matches`);
    assert.ok(statusLabel().includes(summary), `${key}: status summary ${statusLabel()}`);
    const chips = byId(page, "filings-month-chip");
    assert.equal(chips.length, chip ? 1 : 0, `${key}: month chip`);
    if (chip) assert.ok(texts(chips[0]).includes(chip), texts(chips[0]).join(" | "));
  }
}

// ---------------------------------------------------------------- taxpayers
{
  const app = makeApp(DEMO);
  const home = renderHome(cx, app, 1000);
  const rows = kidsOf(one(home, "home-taxpayers"));
  assert.deepEqual(rows.map((m) => m.id), [`home-tp-${JUANA.root}`, `home-tp-${SARI.root}`], "archived taxpayers are left out");
  const juana = texts(rows[0]);
  assert.ok(juana.includes("2 of 4 filed this year"), juana.join(" | "));
  assert.ok(juana.includes("Next: 1701Q · Nov 16"), juana.join(" | "));
  assert.ok(juana.includes("Start next"));
  const sari = texts(rows[1]);
  assert.ok(sari.includes("2 of 2 filed this year"), sari.join(" | "));
  assert.ok(sari.includes("Next: 0605 · Nov 13"), sari.join(" | "));
  assert.ok(sari.includes("Open next"), "a draft opens");
  const fill = (root) => kidsOf(one(home, `home-tp-${root}-bar`))[0];
  assert.ok(has(fill(JUANA.root), "w", 60), "half filed");
  assert.ok(has(fill(SARI.root), "w", 120), "all filed");

  click(home, `home-tp-${JUANA.root}-next`);
  assert.deepEqual(opened.pop(), { opts: { tin: JUANA.tin, code: "1701Q", year: 2026, period: 3 }, root: JUANA.root, code: "00000" });

  // Narrow: one line under the name.
  const narrow = renderHome(cx, makeApp(DEMO), 480);
  assert.ok(texts(one(narrow, `home-tp-${JUANA.root}`)).includes("2 of 4 filed this year · Next: 1701Q · Nov 16"));
}

// ---------------------------------------------------------------- zeros
{
  const app = makeApp(EMPTY);
  const home = renderHome(cx, app, 1000);
  for (const key of ["overdue", "week", "drafts", "waiting", "filed"]) assert.equal(tile(home, key)[1], "0", `${key} is 0`);
  assert.deepEqual(dueRows(home), ["home-due-empty"]);
  assert.ok(texts(one(home, "home-due-empty")).includes("Deadlines show here once a taxpayer has forms for a tax year."));
  const row = texts(one(home, `home-tp-${JUANA.root}`));
  assert.ok(row.includes("Nothing was due yet this year") && row.includes("Nothing due in 60 days"), row.join(" | "));
  assert.equal(byId(home, `home-tp-${JUANA.root}-next`).length, 0, "no next button");
  assert.ok(texts(one(home, "home-filed-empty")).includes("Filed returns show here."));
  click(home, "home-tile-overdue");
  assert.ok(texts(one(renderHome(cx, app, 1000), "home-due-empty")).includes("Nothing is overdue."));
}

// ---------------------------------------------------------------- new install
{
  const app = makeApp({ taxpayers: [], deadlines: [], saved: [], filed: [] });
  const home = renderHome(cx, app, 1000);
  assert.equal(byId(home, "home-tiles").length, 0, "no dashboard before a taxpayer");
  assert.ok(texts(nodes(home)[0]).includes("Welcome to eBIRForms"));
  assert.equal(byId(home, "welcome-add-taxpayer").length, 1);
  assert.equal(count("list_returns"), 0, "no return reads on a new install");
}

// ---------------------------------------------------------------- poll and visits
{
  const app = makeApp(DEMO);
  renderHome(cx, app, 1000);
  assert.equal(count("list_returns", "saved"), 1);
  const deadlinesBefore = count("deadlines_between");
  const filedBefore = count("list_returns", "filed");

  // The 30 s poll re-reads returns and deadlines, not the drafts.
  app.pollEvents(cx);
  renderHome(cx, app, 1000);
  assert.equal(count("list_returns", "saved"), 1, "the poll keeps the drafts count");
  assert.ok(count("list_returns", "filed") > filedBefore, "the poll re-reads filed returns");
  assert.equal(count("deadlines_between"), deadlinesBefore + 4, "the poll re-reads deadlines");
  app.pollEvents(cx);
  renderHome(cx, app, 1000);
  assert.equal(count("list_returns", "saved"), 1);

  // Leaving Home and coming back reads it once more.
  app.go("filings", cx);
  app.go("home", cx);
  renderHome(cx, app, 1000);
  renderHome(cx, app, 1000);
  assert.equal(count("list_returns", "saved"), 2, "once per visit");
  app.goBack(cx);
  app.goBack(cx);
  assert.equal(app.view, "home");
  renderHome(cx, app, 1000);
  assert.equal(count("list_returns", "saved"), 3, "Back into Home is a visit");

  // A write drops it too: the count follows a saved draft.
  host = { ...DEMO, saved: [...DEMO.saved, ret(JUANA, "1601C", "November 2026", "Draft", "2026-11-13 01:00:00")] };
  app.store.bump();
  assert.equal(tile(renderHome(cx, app, 1000), "drafts")[1], "3");
}

console.log("test_home_dashboard: ok");
