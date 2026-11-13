// Calendar and Filings filter with one combo box (round two, claim 4),
// driven through the real app/shell/ui.js, calendar_page.js, filings.js,
// home.js and store.js with a mocked kit and `bir` host and demo data
// (Juana Santos plus invented taxpayers).
//
// The combo box: closed summary, open, search, Select all, Clear, outside
// press and Esc close it, opening and typing redraw only the combo, and all
// five filters are the same class. Calendar: each How-often choice, a month
// chip by the month a return is due, a quarter chip by the quarter it
// covers, the year stepper, List and Month (six weeks, today ring, a picked
// day's list), the count line and the empty state. Filings: the board's
// columns from the real statuses with Paid inside Confirmed, the four
// filters and search together, the count and Clear filters, the card
// actions the list had, and Home's three presets.
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
    if (k === "child" || k === "content") m.kids.push(args[0]);
    else if (k === "children") m.kids.push(...args[0]);
    else m.calls.push([k, ...args]);
    if (k === "id") m.id = String(args[0]);
    if (k === "when" && args[0]) args[1](self);
  });
  meta.set(e, { id: null, calls: [], kids: [] });
  return e;
}
/** Every node, rendering each retained entity it meets (as the host would). */
function nodes(e, out = []) {
  if (e && e.__view) return nodes(e.__view.render(cx), out);
  const m = meta.get(e);
  if (!m) return out;
  out.push(m);
  for (const k of m.kids) nodes(k, out);
  return out;
}
const byId = (tree, id) => nodes(tree).filter((m) => m.id === id);
const one = (tree, id) => {
  const all = byId(tree, id);
  assert.equal(all.length, 1, `exactly one #${id}`);
  return all[0];
};
const handler = (m, name) => {
  const c = m.calls.find((x) => x[0] === name);
  assert.ok(c, `#${m.id} has ${name}`);
  return c[1];
};
const click = (tree, id) => handler(one(tree, id), "on_click")({}, cx);
/** All text under a node, in order. */
function text(m) {
  let out = "";
  for (const k of m.kids) {
    if (typeof k === "string") out += `${k} `;
    else if (k && k.__view) out += text(meta.get(k.__view.render(cx)) || { kids: [] });
    else if (meta.get(k)) out += text(meta.get(k));
  }
  return out;
}
const idsUnder = (m, prefix) => nodes2(m).map((n) => n.id).filter((id) => id && id.startsWith(prefix));
function nodes2(m, out = []) {
  out.push(m);
  for (const k of m.kids) {
    if (k && k.__view) nodes(k, out);
    else if (meta.get(k)) nodes2(meta.get(k), out);
  }
  return out;
}

class View {}
function makeInputState(opts = {}) {
  return {
    v: opts.value || "",
    handlers: {},
    value() {
      return this.v;
    },
    set_value(x) {
      this.v = String(x);
    },
    on(ev, h) {
      this.handlers[ev] = h;
      return true;
    },
  };
}
const InputState = { new: makeInputState };
const Input = {
  new: (state) => {
    const e = el();
    meta.get(e).input = state;
    return e;
  },
};
const Popup = {
  new: (id, trigger) => {
    const e = el();
    e.id(id);
    meta.get(e).kids.push(trigger);
    return e;
  },
};

// The context: which views a call asked to redraw, and the views it made.
const made = [];
const cx = {
  notified: [],
  stopped: 0,
  notify(target) {
    this.notified.push(target === undefined ? "page" : target);
  },
  stop_propagation() {
    this.stopped += 1;
  },
  theme: () => ({ is_dark: false }),
  timer: { after() {}, every() {} },
  new(Class, props) {
    const view = new Class();
    view.init(props, cx);
    const entity = {
      __view: view,
      set_props(p) {
        view.update?.(p);
      },
      release: () => true,
    };
    made.push({ Class, entity, id: props.id });
    return entity;
  },
};

// ---------------------------------------------------------------- the host

const Y = new Date().getFullYear();
const JUANA = { tin: "00012345300000", tin_root: "000123453", branch_code: "00000", branch_name: "Head office", taxpayer_name: "Juana Santos", branch_count: 1 };
const SARI = { tin: "00077788800000", tin_root: "000777888", branch_code: "00000", branch_name: "Head office", taxpayer_name: "Santos Sari-Sari", branch_count: 1 };
const LAKBAY = { tin: "00055566600000", tin_root: "000555666", branch_code: "00000", branch_name: "Head office", taxpayer_name: "Lakbay Travel", branch_count: 1 };
const dl = (who, code, frequency, year, period, label, due, status = "Not started") => ({
  ...who,
  form_code: code,
  form_name: code,
  frequency,
  taxable_year: year,
  period,
  period_label: label,
  due_date: due,
  status,
  draft_id: null,
  return_key: null,
});
const DEADLINES = [
  dl(JUANA, "1601C", "Monthly", Y - 1, 12, `December ${Y - 1}`, `${Y}-01-12`, "Confirmed"),
  dl(JUANA, "1601C", "Monthly", Y, 1, `January ${Y}`, `${Y}-02-10`, "Submitted"),
  dl(JUANA, "1601C", "Monthly", Y, 12, `December ${Y}`, `${Y + 1}-01-12`),
  dl(JUANA, "2551Q", "Quarterly", Y, 1, `Q1 ${Y}`, `${Y}-04-25`, "Draft"),
  dl(JUANA, "2551Q", "Quarterly", Y, 4, `Q4 ${Y}`, `${Y + 1}-01-25`),
  dl(SARI, "1701", "Annual", Y - 1, 0, `TY ${Y - 1}`, `${Y}-04-15`, "Paid"),
  dl(SARI, "1701", "Annual", Y, 0, `TY ${Y}`, `${Y + 1}-04-15`),
  dl(LAKBAY, "1601EQ", "Quarterly", Y, 3, `Q3 ${Y}`, `${Y}-10-31`),
];
const utc = (d) => d.toISOString().slice(0, 19).replace("T", " ");
const now = new Date();
const thisMonth = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}`;
const longAgo = utc(new Date(now.getFullYear(), now.getMonth() - 3, 10, 12));
const ret = (id, who, code, year, period, label, status, updated) => ({ id, ...who, form_code: code, taxable_year: year, period, period_label: label, status, updated_at: updated, return_key: id + 100 });
const FILED = [
  ret(1, JUANA, "1601C", Y, 1, `January ${Y}`, "Submitted", utc(now)),
  ret(2, JUANA, "1601C", Y - 1, 12, `December ${Y - 1}`, "Confirmed", longAgo),
  ret(3, SARI, "1701", Y - 1, 0, `TY ${Y - 1}`, "Paid", utc(now)),
  ret(4, LAKBAY, "1601EQ", Y, 2, `Q2 ${Y}`, "Queued", utc(now)),
  ret(5, LAKBAY, "1601EQ", Y, 1, `Q1 ${Y}`, "Confirmed", utc(now)),
];
const SAVED = [ret(6, JUANA, "2551Q", Y, 1, `Q1 ${Y}`, "Draft", longAgo), ret(7, SARI, "1701", Y, 0, `TY ${Y}`, "Draft", utc(now))];
const hostCalls = [];
const birExports = {
  deadlines_between: (from, to) => {
    hostCalls.push(["deadlines_between", from, to]);
    return DEADLINES.filter((d) => d.due_date >= from && d.due_date <= to);
  },
  list_returns: (which) => {
    hostCalls.push(["list_returns", which]);
    return which === "filed" ? FILED : which === "saved" ? SAVED : [...FILED, ...SAVED];
  },
  list_taxpayers: () =>
    [JUANA, SARI, LAKBAY].map((w) => ({ tin_root: w.tin_root, name: w.taxpayer_name, tin_display: `${w.tin_root.slice(0, 3)}-${w.tin_root.slice(3, 6)}-${w.tin_root.slice(6)}-00000`, branches: [] })),
  tax_year_bounds: () => ({ min: Y - 2, max: Y + 1 }),
  screen_forms_index: () => [
    { c: "1601C", t: "Monthly Remittance Return of Income Taxes Withheld on Compensation", freq: "Monthly" },
    { c: "1601EQ", t: "Quarterly Remittance Return of Creditable Income Taxes Withheld", freq: "Quarterly" },
    { c: "2551Q", t: "Quarterly Percentage Tax Return", freq: "Quarterly" },
    { c: "1701", t: "Annual Income Tax Return", freq: "Annual" },
  ],
  // Lakbay already keeps a 1601EQ template.
  list_form_templates: (tin) => (tin === LAKBAY.tin ? [{ form_code: "1601EQ" }] : []),
};

// ---------------------------------------------------------------- modules

const context = vm.createContext({ console, Map, Set, JSON, Date, Math, Number, String, Proxy, WeakMap, Array, Object });
const mock = (name, exports) =>
  new vm.SyntheticModule(Object.keys(exports), function () {
    for (const [k, v] of Object.entries(exports)) this.setExport(k, v);
  }, { context, identifier: name });
const source = (path) => new vm.SourceTextModule(fs.readFileSync(new URL(path, root), "utf8"), { context, identifier: path });
const mods = {
  "./ui.js": source("app/shell/ui.js"),
  "./store.js": source("app/shell/store.js"),
  "./home.js": source("app/shell/home.js"),
  "./dashboard.js": source("app/shell/dashboard.js"),
  "./periods.js": source("app/shell/periods.js"),
  "./calendar_page.js": source("app/shell/calendar_page.js"),
  "./filings.js": source("app/shell/filings.js"),
};
const linked = {
  ...mods,
  "gpui-kit": mock("gpui-kit", { View, div: el, svg: el }),
  "gpui-base": mock("gpui-base", { Input, InputState, Popup, OtpInput: { new: el }, OtpState: InputState }),
  bir: mock("bir", birExports),
};
// A module's link links what it imports, so one an earlier module already
// pulled in (dashboard.js through home.js) is skipped here.
for (const m of Object.values(mods)) if (m.status === "unlinked") await m.link((s) => linked[s]);
for (const m of Object.values(mods)) await m.evaluate();
const { MultiCombo } = mods["./ui.js"].namespace;
const { Store } = mods["./store.js"].namespace;
const { renderCalendar, CalendarFilters } = mods["./calendar_page.js"].namespace;
const { renderFilings, FilingsFilters } = mods["./filings.js"].namespace;

const opened = [];
const cancelled = [];
const templated = [];
function makeApp() {
  const app = {
    store: new Store(),
    invalidations: 0,
    invalidate() {
      this.invalidations += 1;
    },
    isFresh: () => false,
    openForm: (opts) => opened.push(JSON.parse(JSON.stringify(opts))),
    cancelQueuedEntry: (entry) => cancelled.push(JSON.parse(JSON.stringify(entry))),
    taxpayerUi: { useAsTemplate: (r) => templated.push(r.id) },
  };
  app.calendarUi = new CalendarFilters(app, cx);
  app.filingsUi = new FilingsFilters(app, cx);
  return app;
}
const app = makeApp();
const cal = () => renderCalendar(cx, app, 1000);
const fil = (width = 1000) => renderFilings(cx, app, width);
const f = app.calendarUi;
const g = app.filingsUi;

// ---------------------------------------------------------------- one combo box, five times

assert.equal(made.length, 5, "one combo on Calendar, four on Filings");
assert.ok(made.every((m) => m.Class === MultiCombo), "every filter is the one MultiCombo class");
assert.deepEqual(made.map((m) => m.id), ["calendar-taxpayers", "filings-taxpayers", "filings-forms", "filings-periods", "filings-statuses"]);

const label = (tree, id) => text(one(tree, `${id}-trigger`)).trim();
{
  let tree = cal();
  // First open: the List view of every taxpayer, with the year stepper.
  assert.deepEqual([f.view, f.tps, f.freq], ["list", null, "all"], "Calendar starts on List, all taxpayers");
  assert.equal(byId(tree, "calendar-year").length, 1, "List shows the year stepper");
  assert.equal(byId(tree, "calendar-grid").length, 0, "no month grid at first");
  // A taxpayer chosen on Filings stays on Filings.
  g.tps = new Set([JUANA.tin_root]);
  assert.equal(label(cal(), "calendar-taxpayers"), "All taxpayers", "Filings' choice does not narrow the Calendar");
  g.tps = null;
  assert.equal(label(tree, "calendar-taxpayers"), "All taxpayers");
  assert.equal(byId(tree, "calendar-taxpayers-pop").length, 0, "closed at first");

  // Opening redraws the combo only: no page redraw, no untargeted notify.
  cx.notified = [];
  const before = app.invalidations;
  click(tree, "calendar-taxpayers-trigger");
  assert.equal(app.invalidations, before, "opening does not redraw the page");
  assert.deepEqual(cx.notified, [f.tpCombo.entity], "opening redraws the combo alone");
  tree = cal();
  const pop = one(tree, "calendar-taxpayers-pop");
  assert.deepEqual(idsUnder(pop, "calendar-taxpayers-opt-").filter((id) => !id.endsWith("-box")), [
    "calendar-taxpayers-opt-000123453",
    "calendar-taxpayers-opt-000777888",
    "calendar-taxpayers-opt-000555666",
  ]);
  assert.match(text(one(tree, "calendar-taxpayers-opt-000123453")), /Juana Santos 000-123-453-00000/, "name over TIN");
  for (const id of ["calendar-taxpayers-search", "calendar-taxpayers-all", "calendar-taxpayers-clear"]) one(tree, id);

  // Typing filters the list and redraws the combo only.
  const search = f.tpCombo.view.search;
  cx.notified = [];
  search.set_value("lakbay");
  search.handlers.change({}, cx);
  assert.deepEqual(cx.notified, [f.tpCombo.entity], "typing redraws the combo alone");
  assert.equal(app.invalidations, before, "typing does not redraw the page");
  tree = cal();
  assert.deepEqual(idsUnder(one(tree, "calendar-taxpayers-pop"), "calendar-taxpayers-opt-").filter((id) => !id.endsWith("-box")), ["calendar-taxpayers-opt-000555666"]);
  search.set_value("nobody");
  tree = cal();
  assert.match(text(one(tree, "calendar-taxpayers-pop")), /Nothing matches\./);

  // Clear with no search: nothing on. Then Select all over a search adds its matches only.
  search.set_value("");
  click(cal(), "calendar-taxpayers-clear");
  assert.equal(app.invalidations, before + 1, "a changed selection redraws the page");
  assert.equal(f.tps.size, 0);
  assert.equal(label(cal(), "calendar-taxpayers"), "No taxpayers");
  search.set_value("santos");
  click(cal(), "calendar-taxpayers-all");
  assert.deepEqual([...f.tps].sort(), ["000123453", "000777888"]);
  assert.equal(label(cal(), "calendar-taxpayers"), "2 taxpayers");
  search.set_value("");
  click(cal(), "calendar-taxpayers-opt-000777888");
  assert.equal(label(cal(), "calendar-taxpayers"), "JS Juana Santos", "one on: its avatar and name");
  // The box itself toggles once and keeps the click off the row.
  const stopped = cx.stopped;
  click(cal(), "calendar-taxpayers-opt-000555666-box");
  assert.equal(cx.stopped, stopped + 1);
  assert.equal(label(cal(), "calendar-taxpayers"), "2 taxpayers");
  click(cal(), "calendar-taxpayers-all");
  assert.equal(f.tps, null, "every value on is null: All");
  assert.equal(label(cal(), "calendar-taxpayers"), "All taxpayers");

  // An outside press closes it; the click that follows on the trigger does not reopen it.
  tree = cal();
  handler(one(tree, "calendar-taxpayers-pop"), "on_mouse_down_out")({}, cx);
  assert.equal(f.tpCombo.isOpen(), false, "outside press closes");
  click(cal(), "calendar-taxpayers-trigger");
  assert.equal(f.tpCombo.isOpen(), false, "the press on the trigger that closed it does not reopen it");
  f.tpCombo.view.closedAt = 0;
  click(cal(), "calendar-taxpayers-trigger");
  assert.equal(f.tpCombo.isOpen(), true);

  // Esc closes it once (BirApp.dismiss asks the pages first).
  assert.equal(f.dismiss(cx), true);
  assert.equal(f.tpCombo.isOpen(), false);
  assert.equal(f.dismiss(cx), false, "a second Esc goes on to the app");
  const mainSrc = fs.readFileSync(new URL("app/main.js", root), "utf8");
  assert.match(mainSrc, /dismiss\(cx\) \{\n\s+if \(this\.calendarUi\?\.dismiss\(cx\) \|\| this\.filingsUi\?\.dismiss\(cx\)\) return;/, "Esc reaches the combos first");

  // Opening one closes another.
  click(cal(), "calendar-taxpayers-trigger");
  click(fil(), "filings-forms-trigger");
  assert.equal(f.tpCombo.isOpen(), false, "opening a second combo closes the first");
  assert.equal(g.formCombo.isOpen(), true);
  g.dismiss(cx);
}

// ---------------------------------------------------------------- Calendar

const calRows = (tree) =>
  nodes(tree)
    .filter((m) => m.id && m.id.startsWith("due-"))
    .map((m) => m.id);
const count = (tree) => text(one(tree, "calendar-count")).trim();
const due = (who, code, year, period) => `due-${who.tin}-${code}-${year}-${period}`;
{
  hostCalls.length = 0;
  app.store.bump();
  let tree = cal();
  assert.deepEqual(hostCalls.filter((c) => c[0] === "deadlines_between"), [["deadlines_between", `${Y}-01-01`, `${Y + 1}-12-31`]], "one read for the year");
  assert.deepEqual(calRows(tree), [
    due(JUANA, "1601C", Y, 1),
    due(JUANA, "2551Q", Y, 1),
    due(LAKBAY, "1601EQ", Y, 3),
    due(JUANA, "1601C", Y, 12),
    due(JUANA, "2551Q", Y, 4),
    due(SARI, "1701", Y, 0),
  ], "All: the year's returns in due order");
  assert.equal(count(tree), `6 deadlines · 3 taxpayers · ${Y} returns`);
  assert.equal(byId(tree, "calendar-month").length + byId(tree, "calendar-quarter").length, 0, "no chips under All");

  click(tree, "calendar-freq-yearly");
  assert.deepEqual(calRows(cal()), [due(SARI, "1701", Y, 0)], "Yearly: annual forms only");

  click(cal(), "calendar-freq-monthly");
  tree = cal();
  assert.deepEqual(calRows(tree), [due(JUANA, "1601C", Y, 1), due(JUANA, "1601C", Y, 12)], "Monthly: monthly forms only");
  one(tree, "calendar-month-0");
  click(tree, "calendar-month-1");
  tree = cal();
  assert.deepEqual(calRows(tree), [due(JUANA, "1601C", Y - 1, 12)], "January is the month due: last December's 1601C");
  assert.equal(count(tree), `1 deadline · 1 taxpayer · due in January ${Y}`);

  click(tree, "calendar-freq-quarterly");
  tree = cal();
  assert.equal(f.month, 0, "a new How often clears the month");
  assert.deepEqual(calRows(tree), [due(JUANA, "2551Q", Y, 1), due(LAKBAY, "1601EQ", Y, 3), due(JUANA, "2551Q", Y, 4)], "Quarterly: grouped per quarter");
  for (const q of [1, 3, 4]) one(tree, `calendar-Q${q}-${Y}`);
  click(tree, "calendar-quarter-4");
  tree = cal();
  assert.deepEqual(calRows(tree), [due(JUANA, "2551Q", Y, 4)], "Q4 is the quarter covered, due next January");
  assert.equal(count(tree), `1 deadline · 1 taxpayer · returns for Q4 ${Y}`);

  // The year stepper: the year the returns cover.
  click(tree, "calendar-freq-all");
  hostCalls.length = 0;
  click(cal(), "calendar-year-prev");
  tree = cal();
  assert.equal(f.year, Y - 1);
  assert.deepEqual(hostCalls.filter((c) => c[0] === "deadlines_between"), [["deadlines_between", `${Y - 1}-01-01`, `${Y}-12-31`]]);
  assert.deepEqual(calRows(tree), [due(JUANA, "1601C", Y - 1, 12), due(SARI, "1701", Y - 1, 0)]);
  click(tree, "calendar-year-next");
  assert.equal(f.year, Y);

  // Empty: a friendly line and Clear filters, which resets every filter.
  f.tps = new Set([LAKBAY.tin_root]);
  click(cal(), "calendar-freq-yearly");
  tree = cal();
  assert.deepEqual(calRows(tree), []);
  assert.match(text(one(tree, "calendar-empty-row")), /No deadlines match these filters\./);
  cx.notified = [];
  click(tree, "calendar-clear");
  assert.ok(cx.notified.includes(f.tpCombo.entity), "Clear filters redraws the combo's summary");
  assert.deepEqual([f.tps, f.freq, f.month, f.quarter, f.year], [null, "all", 0, 0, Y]);
  assert.equal(calRows(cal()).length, 6);

  // The rows open their return, as before.
  opened.length = 0;
  click(cal(), due(JUANA, "2551Q", Y, 1));
  assert.deepEqual(opened.pop(), { tin: JUANA.tin, code: "2551Q", year: Y, period: 1 });

  // Month: a six-week grid, one read for it, the picked day's deadlines.
  click(cal(), "calendar-view-month");
  f.gridYear = Y;
  f.gridMonth = 3;
  hostCalls.length = 0;
  tree = cal();
  assert.equal(byId(tree, "calendar-year").length, 0, "no year stepper on the grid");
  assert.match(text(one(tree, "calendar-month-title")), new RegExp(`April ${Y}`), "the grid names its month and year");
  const cells = nodes(tree).filter((m) => m.id && /^calendar-day-\d{4}-\d{2}-\d{2}$/.test(m.id));
  assert.equal(cells.length, 42, "six weeks");
  assert.equal(cells[0].id, `calendar-day-${mods["./store.js"].namespace.isoDate(new Date(Y, 3, 1 - new Date(Y, 3, 1).getDay()))}`, "starts on the Sunday on or before the 1st");
  const reads = hostCalls.filter((c) => c[0] === "deadlines_between");
  assert.equal(reads.length, 1, "one read per month on screen");
  assert.deepEqual(reads[0].slice(1), [cells[0].id.slice(13), cells[41].id.slice(13)]);
  assert.match(text(one(tree, "calendar-month-count")), /^2 deadlines this month/, "April: 2551Q and 1701");
  assert.match(text(one(tree, `calendar-day-${Y}-04-25`)), /2551Q JS/, "a day shows its deadlines");
  click(tree, `calendar-day-${Y}-04-25`);
  tree = cal();
  const list = one(tree, "calendar-day-list");
  assert.deepEqual(idsUnder(list, "calendar-day-row-"), [`calendar-day-row-${JUANA.tin}-2551Q-${Y}-1`]);
  click(tree, `calendar-day-row-${JUANA.tin}-2551Q-${Y}-1`);
  assert.deepEqual(opened.pop(), { tin: JUANA.tin, code: "2551Q", year: Y, period: 1 }, "the day's row opens its return");
  click(tree, `calendar-day-${Y}-04-26`);
  assert.match(text(one(cal(), "calendar-day-list")), /Nothing due this day\./);
  // Another month: the chosen day does not linger in the panel. It moves to
  // today when the month holds today, and is cleared otherwise.
  const { isoDate } = mods["./store.js"].namespace;
  const todayIso = isoDate(new Date());
  const dayIn = (month) => (new Date().getMonth() === month ? todayIso : null);
  const panel = () => text(one(cal(), "calendar-day")).trim();
  click(cal(), "calendar-next-month");
  assert.deepEqual([f.gridYear, f.gridMonth], [Y, 4]);
  assert.equal(f.day, dayIn(4), "next month: April 26 is no longer the chosen day");
  if (!f.day) {
    assert.match(panel(), new RegExp(`^May ${Y} Pick a day to see what is due\\.`), `a neutral prompt: ${panel()}`);
    assert.doesNotMatch(panel(), /April 26/);
  }
  click(cal(), "calendar-prev-month");
  click(cal(), "calendar-prev-month");
  assert.deepEqual([f.gridYear, f.gridMonth], [Y, 2]);
  assert.equal(f.day, dayIn(2), "previous month: the same rule");
  for (let i = 0; i < 10; i++) click(cal(), "calendar-next-month");
  assert.deepEqual([f.gridYear, f.gridMonth], [Y + 1, 0], "across the year end");
  assert.equal(f.day, null);
  click(cal(), `calendar-day-${Y + 1}-01-12`);
  assert.match(panel(), /January 12/);
  click(cal(), "calendar-today");
  tree = cal();
  assert.equal(f.day, todayIso, "Today chooses today");
  assert.equal(byId(tree, "calendar-today-ring").length, 1, "today is ringed once");
  const now2 = new Date();
  assert.match(panel(), new RegExp(`${["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"][now2.getMonth()]} ${now2.getDate()}`), "the panel lists today");
  // The grid keeps the taxpayer and How often filters.
  f.gridMonth = 3;
  f.tps = new Set([SARI.tin_root]);
  assert.doesNotMatch(text(one(cal(), `calendar-day-${Y}-04-25`)), /2551Q/);
  f.tps = null;
  click(cal(), "calendar-view-list");
  assert.equal(calRows(cal()).length, 6, "back to the list");
}

// ---------------------------------------------------------------- Filings

const COLS = ["draft", "queued", "submitted", "confirmed"];
const board = (tree) =>
  Object.fromEntries(
    COLS.map((c) => {
      const col = byId(tree, `filings-col-${c}`);
      return [c, col.length ? idsUnder(col[0], "return-").filter((id) => /^return-\d+$/.test(id)) : null];
    }),
  );
const filCount = (tree) => text(one(tree, "filings-count")).trim();
{
  hostCalls.length = 0;
  app.store.bump();
  let tree = fil();
  const reads = hostCalls.filter((c) => c[0] === "list_returns").map((c) => c[1]);
  assert.deepEqual([...new Set(reads)].sort(), ["filed", "saved"], "drafts are read too; never the wider 'all'");
  assert.deepEqual(board(tree), {
    draft: ["return-7", "return-6"],
    queued: ["return-4"],
    submitted: ["return-1"],
    confirmed: ["return-3", "return-5", "return-2"],
  }, "four columns from the real statuses; Paid sits in Confirmed");
  assert.match(text(one(tree, "return-3")), /Paid/, "the paid return is marked paid");
  assert.match(text(one(tree, "filings-col-confirmed")), /^Confirmed 3 /);
  assert.equal(filCount(tree), "7 returns");
  assert.equal(byId(tree, "filings-clear").length, 0, "no Clear filters with nothing narrowed");
  for (const [id, all] of [["filings-taxpayers", "All taxpayers"], ["filings-forms", "All forms"], ["filings-periods", "All periods"], ["filings-statuses", "All statuses"]]) {
    assert.equal(label(tree, id), all);
  }

  // The options: forms with their title, periods newest first, the five statuses.
  click(tree, "filings-periods-trigger");
  tree = fil();
  assert.deepEqual(idsUnder(one(tree, "filings-periods-pop"), "filings-periods-opt-").filter((id) => !id.endsWith("-box")), [
    `filings-periods-opt-TY-${Y}`,
    `filings-periods-opt-Q2-${Y}`,
    `filings-periods-opt-Q1-${Y}`,
    `filings-periods-opt-TY-${Y - 1}`,
    `filings-periods-opt-Q4-${Y - 1}`,
  ]);
  g.dismiss(cx);
  click(fil(), "filings-statuses-trigger");
  tree = fil();
  assert.deepEqual(idsUnder(one(tree, "filings-statuses-pop"), "filings-statuses-opt-").filter((id) => !id.endsWith("-box")), ["Draft", "Queued", "Submitted", "Confirmed", "Paid"].map((s) => `filings-statuses-opt-${s}`));
  assert.match(text(one(tree, "filings-statuses-opt-Confirmed")), /2 returns/);
  g.dismiss(cx);
  click(fil(), "filings-forms-trigger");
  assert.match(text(one(fil(), "filings-forms-opt-2551Q")), /2551Q Quarterly Percentage Tax Return/, "code over title");
  g.dismiss(cx);

  // Each filter, then all four and the search together.
  g.tps = new Set([JUANA.tin_root]);
  assert.equal(filCount(fil()), "3 returns");
  g.forms = new Set(["1601C"]);
  assert.equal(filCount(fil()), "2 returns");
  g.periods = new Set([`Q1 ${Y}`]);
  tree = fil();
  assert.deepEqual(board(tree), { draft: [], queued: [], submitted: ["return-1"], confirmed: [] }, "empty columns stay, saying so");
  assert.match(text(one(tree, "filings-col-draft")), /Nothing here\./);
  g.statuses = new Set(["Confirmed", "Paid"]);
  tree = fil();
  assert.match(text(one(tree, "filings-empty")), /No returns match these filters\./);
  assert.equal(filCount(tree), "0 returns");
  g.reset();
  g.statuses = new Set(["Confirmed", "Paid"]);
  g.query.set_value("lakbay");
  tree = fil();
  assert.deepEqual(board(tree).confirmed, ["return-5"], "search and a status together");
  assert.equal(filCount(tree), "1 return");
  assert.equal(label(tree, "filings-statuses"), "2 statuses");
  click(tree, "filings-clear");
  tree = fil();
  assert.equal(filCount(tree), "7 returns", "Clear filters shows every return");
  assert.equal(g.query.value(), "");
  assert.equal(label(tree, "filings-statuses"), "All statuses");
  assert.equal(byId(tree, "filings-clear").length, 0);

  // The actions a row had are on the cards.
  for (const id of [1, 2, 3, 4, 5, 6, 7]) one(tree, `return-${id}-open`);
  assert.deepEqual(byId(tree, "return-4-cancel").length, 1, "Cancel queue on the Queued card");
  assert.equal(nodes(tree).filter((m) => m.id && m.id.endsWith("-cancel")).length, 1);
  assert.deepEqual(
    nodes(tree).filter((m) => m.id && m.id.endsWith("-use-template")).map((m) => m.id).sort(),
    ["return-1-use-template", "return-2-use-template", "return-3-use-template"],
    "Use as template on filed returns without one (Lakbay keeps a 1601EQ template)",
  );
  const stopped = cx.stopped;
  click(tree, "return-4-cancel");
  assert.deepEqual(cancelled.pop(), { tin: LAKBAY.tin, form_code: "1601EQ", taxable_year: Y, quarter: 2 });
  click(tree, "return-1-use-template");
  assert.equal(templated.pop(), 1);
  click(tree, "return-6-open");
  assert.deepEqual(opened.pop(), { tin: JUANA.tin, code: "2551Q", year: Y, period: 1, returnKey: 106 });
  assert.equal(cx.stopped, stopped + 3, "a card's buttons keep their click off the card");
  click(tree, "return-2");
  assert.deepEqual(opened.pop(), { tin: JUANA.tin, code: "1601C", year: Y - 1, period: 12, returnKey: 102 }, "the card opens its return");

  // Narrow: the columns stack.
  const narrow = one(fil(500), "filings-board");
  assert.ok(narrow.calls.some((c) => c[0] === "flex_col"));

  // A card's status and date cannot run past its edge. The columns the
  // board really gets: 816 px of content at 1800 and 1200 px windows (the
  // 880 px content cap less 32 px padding each side) and 578 px at 900
  // (stacked). Four columns of 816 leave a card (816 - 3 * 12) / 4 - 2 * 10
  // - 2 * 12 - 2 = 149 px inside; the status chip with its icon and
  // "Nov 13, 2026" need about 160, so the row must wrap, its parts must not
  // shrink, and nothing may stretch between them.
  const has = (m, name, ...args) => m.calls.some((c) => c[0] === name && args.every((a, i) => c[i + 1] === a));
  const inside = (816 - 3 * 12) / 4 - 2 * 10 - 2 * 12 - 2;
  assert.ok(inside < 160, `precondition: one line cannot hold both at ${inside} px`);
  for (const width of [816, 578]) {
    const t2 = fil(width);
    for (const id of [1, 2, 3, 4, 5, 6, 7]) {
      const row = one(t2, `return-${id}-meta`);
      assert.ok(has(row, "flex_wrap") && has(row, "min_w", 0), `return ${id}: the status and date row wraps at ${width}`);
      const [chip, date] = row.kids.map((k) => meta.get(k));
      assert.equal(row.kids.length, 2, "no spacer stretched between them");
      assert.ok(has(chip, "flex_none"), "the chip keeps its size");
      assert.ok(has(date, "whitespace_nowrap") && has(date, "flex_none") && has(date, "ml_auto"), "the date stays whole, right-aligned on its line");
      assert.ok(!nodes(row).some((m) => has(m, "flex_1")), "nothing in the row grows past the card");
    }
  }
}

// ---------------------------------------------------------------- Home's presets
{
  const presets = [
    [{ statuses: ["Draft"] }, { draft: ["return-7", "return-6"], queued: [], submitted: [], confirmed: [] }, "Draft", null],
    [{ statuses: ["Queued"] }, { draft: [], queued: ["return-4"], submitted: [], confirmed: [] }, "Queued", null],
    [
      { statuses: ["Submitted", "Confirmed", "Paid"], updatedMonth: thisMonth },
      { draft: [], queued: [], submitted: ["return-1"], confirmed: ["return-3", "return-5"] },
      "3 statuses",
      "Changed in",
    ],
  ];
  for (const [preset, cards, summary, chip] of presets) {
    // A filter left from before is replaced.
    g.tps = new Set([SARI.tin_root]);
    g.query.set_value("x");
    app.filingsPreset = preset;
    cx.notified = [];
    g.applyPreset(app, cx);
    assert.equal(app.filingsPreset, null, "a preset is used once");
    for (const c of g.combos) assert.ok(cx.notified.includes(c.entity), "each summary redraws");
    let tree = fil();
    assert.deepEqual(board(tree), cards, JSON.stringify(preset));
    assert.equal(label(tree, "filings-statuses"), summary);
    assert.equal(label(tree, "filings-taxpayers"), "All taxpayers", "every other filter cleared");
    assert.equal(g.query.value(), "");
    assert.equal(byId(tree, "filings-month-chip").length, chip ? 1 : 0);
    if (chip) assert.match(text(one(tree, "filings-month-chip")), /Changed in \w{3} \d{4}/);
    g.applyPreset(app, cx);
    assert.deepEqual(board(fil()), cards, "a second visit without a preset changes nothing");
    click(tree, "filings-clear");
    tree = fil();
    assert.equal(filCount(tree), "7 returns", "Clear filters removes the preset");
    assert.equal(byId(tree, "filings-month-chip").length, 0);
  }
  // The month chip removes itself alone.
  app.filingsPreset = presets[2][0];
  g.applyPreset(app, cx);
  click(fil(), "filings-month-chip");
  assert.equal(g.updatedMonth, null);
  assert.equal(label(fil(), "filings-statuses"), "3 statuses");
  g.clear(cx);
  // No preset at all is no change.
  app.filingsPreset = undefined;
  g.applyPreset(app, cx);
  assert.equal(filCount(fil()), "7 returns");
  const mainSrc = fs.readFileSync(new URL("app/main.js", root), "utf8");
  assert.match(mainSrc, /if \(view === "filings"\) this\.filingsUi\?\.applyPreset\(this, cx\);/, "go() applies the preset as Filings opens");
}

console.log("test_calendar_filings_filters: ok");
