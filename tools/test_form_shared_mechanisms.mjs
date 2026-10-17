// The form page's shared mechanisms (docs/form-findings rules page, rules
// 4-8), driven through the real app/shell/form_page.js with a mocked host
// and a small form that has one box of each kind. Which boxes are money,
// dates, conditional or penalties is the host's to say (crates/bir-shell
// screen_form_module tests prove that on all 43 forms); this proves what the
// page does with it:
//  - rule 7: only a money box gets pesos; a whole-peso box shows no
//    centavos; what is typed reaches the host as typed.
//  - rule 4: 12102020, 12-10-2020 and 12/10/2020 reach the draft as
//    12/10/2020 and the box shows "Dec 10, 2020" (locked and table boxes
//    too); the calendar's month and year lists move its grid.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);

const K = (tail) => `frmTEST:${tail}`;
const schema = {
  c: "TEST", t: "Test return", rev: "2026", freq: "Monthly", pages: 1, n: 4, q: 1,
  parts: [
    {
      n: "Part I · Background Information",
      items: [
        { no: "1", l: "Contact Number", f: [{ k: K("phone"), t: "text", l: "Contact Number" }] },
        { no: "2", l: "Date of Transaction", f: [{ k: K("date"), t: "date", l: "Date of Transaction" }] },
        { no: "3", l: "Date of Incorporation", f: [{ k: K("incorporated"), t: "date", l: "Date of Incorporation" }] },
      ],
    },
    {
      n: "Part II · Computation of Tax",
      items: [
        { no: "14", l: "Sales", f: [{ k: K("sales"), t: "money", l: "Sales" }] },
        { no: "15", l: "Gross receipts (whole pesos)", f: [{ k: K("whole"), t: "money", w: true, l: "Gross receipts" }] },
        { no: "16", l: "Tax due", f: [{ k: K("due"), t: "money", c: true, l: "Tax due" }] },
      ],
    },
    {
      n: "Schedule 1 · Payments",
      items: [
        { no: "", l: "Payments", rows: 2, cols: [{ l: "Date paid", t: "date" }, { l: "Amount", t: "money" }], keys: [[K("paid0"), K("amount0")], [K("paid1"), K("amount1")]] },
      ],
    },
  ],
};
const partOf = (key) => schema.parts.findIndex((p) => p.items.some((it) => (it.f || []).some((f) => f.k === key) || (it.keys || []).flat().includes(key)));
const itemOf = (key) => schema.parts[partOf(key)].items.findIndex((it) => (it.f || []).some((f) => f.k === key) || (it.keys || []).flat().includes(key));
const idOf = (key) => `${partOf(key)}:${itemOf(key)}`;

const entities = [], tasks = [], texts = [], timers = [];
const clicks = new Map(), opacity = new Map();
const calls = [];
let owner = null;
class View {}
function context(entity = null) {
  return {
    notify() { if (entity) entity.dirty = true; },
    new(Class, props) {
      const e = { dirty: true, released: false, set_props(p) { e.props = p; e.view.update?.(p); e.dirty = true; }, release() { e.released = true; return true; } };
      e.view = new Class(props); e.props = props; entities.push(e);
      const old = owner; owner = e; e.view.init?.(props, context(e)); owner = old;
      return e;
    },
    spawn(fn) { const p = Promise.resolve(fn(context(entity))); tasks.push(p); return { cancel() {} }; },
    timer: { after(_ms, fn) { timers.push(fn); return { cancel() {} }; }, every() { return { cancel() {} }; } },
    focus_handle() { return { focus() {}, is_focused: () => false, release() { return true; } }; },
    stop_propagation() {},
    theme() { return { appearance: "light", is_dark: false, colors: {} }; },
  };
}
function runTimers() { while (timers.length) timers.shift()(context()); }
function element() {
  const creationOwner = owner;
  const target = {};
  const el = new Proxy(target, { get(o, k) {
    if (k === "then") return undefined;
    return (...args) => {
      if (k === "id") o.id = args[0];
      if (k === "on_click") clicks.set(o.id, { fn: args[0], entity: creationOwner });
      if (k === "opacity") opacity.set(o.id, args[0]);
      if (k === "when" && args[0]) args[1](el);
      if (k === "when_some" && args[0] != null) args[1](el, args[0]);
      if (k === "child") mount(args[0]);
      if (k === "children") for (const c of args[0]) mount(c);
      return el;
    };
  } });
  return el;
}
function mount(e) { if (e?.view && e.dirty && !e.released) { e.dirty = false; const old = owner; owner = e; e.view.render(context(e)); owner = old; } }
const InputState = { new({ value = "" } = {}) { let text = value; const on = {}; return { value: () => text, set_value: (v) => { text = v; }, on(ev, fn) { on[ev] = fn; }, fire(ev) { on[ev]?.({}, context()); }, type(v) { text = v; on.change?.({}, context()); }, release() { return true; } }; } };
// The calendar's grid: a month that the arrows (and the lists) step.
const calendars = [];
const CalendarState = { new() {
  let value = null, y = 2026, m = 10, onChange = null;
  const cal = {
    set_value(v) { value = v; if (typeof v === "string") { y = Number(v.slice(0, 4)); m = Number(v.slice(5, 7)); } },
    value: () => value, year: () => y, month: () => m, today: () => "2026-10-17", month_days: () => [[]],
    next_month() { m += 1; if (m > 12) { m = 1; y += 1; } },
    prev_month() { m -= 1; if (m < 1) { m = 12; y -= 1; } },
    on(_ev, fn) { onChange = fn; }, pick(day) { value = day; onChange?.(day, context()); }, release() { return true; },
  };
  calendars.push(cal);
  return cal;
} };
const contextVM = vm.createContext({ console, Map, Set, JSON, Date, Math, Number, String, Object, Array, window: { viewport_size: () => ({ width: 1400, height: 900 }) } });
function mock(name, exports) { return new vm.SyntheticModule(Object.keys(exports), function () { for (const [k, v] of Object.entries(exports)) this.setExport(k, v); }, { context: contextVM, identifier: name }); }
const kit = mock("gpui-kit", { div: element, svg: element, View });
const base = mock("gpui-base", { InputState, CalendarState, Popup: { new: element } });
const components = mock("gpui-component", { Spinner: { new: element } });
const uiExports = { tokens: () => new Proxy({}, { get: () => "#000" }), SPACE: [0, 4, 8, 12, 16, 20, 24], RADIUS: {}, SIZE: { validationLine: 18 }, BREAK: { formNav: 760 } };
for (const n of ["Text", "Line", "Code", "Icon", "AppButton", "IconButton", "Avatar", "Chip", "Switch", "Checkbox", "TextField", "Dropdown", "PageHeader", "MetaLine"]) uiExports[n] = element;
const dropdowns = new Map();
uiExports.AppButton = (_cx, id, _label, opts = {}) => { const el = element(); el.id(id); if (opts.onClick) el.on_click(opts.onClick); return el; };
uiExports.IconButton = (_cx, id, _icon, opts = {}) => { const el = element(); el.id(id); if (opts.onClick) el.on_click(opts.onClick); return el; };
uiExports.Dropdown = (_cx, id, opts) => { dropdowns.set(id, opts); const el = element(); el.id(id); return el; };
uiExports.Text = (_cx, _variant, text) => { texts.push(String(text)); return element(); };
uiExports.Line = uiExports.Text;
const store = mock("./store.js", { formatTin: (x) => x, errorText: (e) => String(e && e.message ? e.message : e) });
const periods = mock("./periods.js", { periodLabel: () => "October 2026" });
const mod = new vm.SourceTextModule(fs.readFileSync(new URL("app/shell/form_page.js", root), "utf8"), { context: contextVM });
await mod.link((s) => ({ "gpui-kit": kit, "gpui-base": base, "gpui-component": components, "./ui.js": mock("./ui.js", uiExports), "./store.js": store, "./periods.js": periods })[s]);
await mod.evaluate();
const { FormPageActions, renderFormPage, formatMoney, formatLongDate, shownDate, moveCalendar } = mod.namespace;

// The host: values as the field map writes them; a fill takes edits as given.
let hostValues = {
  [K("phone")]: "0281234567",
  [K("sales")]: "1234567.5",
  [K("whole")]: "1234567",
  [K("due")]: "0",
  [K("date")]: "12/10/2020",
  [K("incorporated")]: "03/05/2021",
  [K("paid0")]: "01/31/2026",
  [K("amount0")]: "100",
  [K("paid1")]: "",
  [K("amount1")]: "",
};
let opened = () => ({
  draft: { id: 7 }, status: "Draft", editable: true, values: { ...hostValues }, options: {},
  locks: { [K("due")]: "computed by BIR's own formulas", [K("incorporated")]: "comes from the taxpayer profile; change it in the profile" },
  profile: [], profile_edit: [], sample: false, blank: [], needs: {}, errors: [],
});
const app = {
  prefs: {}, branchFor: () => null, taxpayers: () => [], layout: () => ({ column: 1056 }),
  invalidate() {}, toast() {}, startNewReturn() {},
  store: {
    templates: () => [],
    write(name, ...args) { calls.push([name, ...args]); return { id: 7, status: "Draft", draft: args[1] }; },
    call(name, ...args) {
      calls.push([name, ...args]);
      if (name === "screen_form_schema") return schema;
      if (name === "screen_form_open") return Promise.resolve(opened());
      if (name === "screen_form_fill") {
        hostValues = { ...hostValues, ...args[2] };
        return { draft: args[1], values: { ...hostValues }, options: {}, rejected: {} };
      }
      if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: { ...hostValues }, options: {}, errors: [] });
      if (name === "screen_form_preview_sync" || name === "screen_form_print") return true;
      throw Error(name);
    },
  },
};
app.formUi = new FormPageActions(app);
const ui = app.formUi;
const cx = context();
async function settle() { while (tasks.length) await Promise.all(tasks.splice(0)); }
function frame() { owner = null; texts.length = 0; opacity.clear(); renderFormPage(cx, app, 1056); }
const lastFill = () => calls.filter((c) => c[0] === "screen_form_fill").at(-1);
async function openAll() {
  ui.open({ code: "TEST", tin: "12345678800000", year: 2026, period: 10 }, cx);
  await settle();
  for (let i = 0; i < schema.parts.length; i++) ui.ensureInputs(i, cx);
  ui.state.openParts = new Set(schema.parts.map((_p, i) => i));
  frame();
}

// ---- Rule 7: only money looks like money.
assert.equal(formatMoney("1234567.5"), "1,234,567.50");
assert.equal(formatMoney("1234567", true), "1,234,567", "whole pesos: no centavos");
assert.equal(formatMoney("-1234", true), "-1,234");
assert.equal(formatMoney("12.5", true), "12.5", "a fraction in a whole-peso box stays as typed");
assert.equal(formatMoney("", true), "");
await openAll();
const input = (k) => ui.state.inputs.get(k);
assert.equal(input(K("sales")).value(), "1,234,567.50", "a peso amount shows pesos and centavos");
assert.equal(input(K("whole")).value(), "1,234,567", "a whole-peso amount shows no centavos");
assert.equal(input(K("phone")).value(), "0281234567", "a contact number shows as typed");
assert.ok(texts.includes("0.00"), "a computed amount shows 0.00");
input(K("whole")).type("2500000");
input(K("whole")).fire("blur");
assert.equal(input(K("whole")).value(), "2,500,000", "leaving a whole-peso box groups the digits only");
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [K("whole")]: "2500000" }), "the host gets what was typed");
input(K("phone")).type("+639171234567");
input(K("phone")).fire("blur");
assert.equal(input(K("phone")).value(), "+639171234567", "a text box keeps exactly what was typed");
console.log("PASS rule 7: only peso amounts look like money.");

// ---- Rule 4: dates read like dates.
assert.equal(formatLongDate({ y: 2020, m: 12, d: 10 }), "Dec 10, 2020");
assert.equal(shownDate("12/10/2020"), "Dec 10, 2020");
assert.equal(shownDate("2020-12-10"), "Dec 10, 2020");
assert.equal(shownDate(""), "", "a blank date stays blank");
assert.equal(shownDate("12/2020"), "12/2020", "what is not a whole date shows as it is");
assert.equal(input(K("date")).value(), "Dec 10, 2020", "the draft's 12/10/2020 shows as Dec 10, 2020");
assert.equal(input(K("paid0")).value(), "Jan 31, 2026", "a schedule date shows the same way");
assert.ok(texts.includes("Mar 5, 2021"), "a locked date shows as Mar 5, 2021");
for (const typed of ["12102020", "12-10-2020", "12/10/2020", "Dec 10, 2020"]) {
  input(K("date")).type("01012000");
  input(K("date")).type(typed);
  assert.equal(ui.state.pending[K("date")], "12/10/2020", `${typed} reaches the draft as 12/10/2020`);
  input(K("date")).fire("blur");
  assert.equal(input(K("date")).value(), "Dec 10, 2020", `${typed} shows as Dec 10, 2020`);
  assert.equal(lastFill()[3][K("date")], "12/10/2020", "the host gets the form's format");
}
input(K("date")).type("13312020");
input(K("date")).fire("blur");
assert.equal(input(K("date")).value(), "13312020", "a typed day that is not a date stays as typed");
assert.match(ui.state.dateErrors[idOf(K("date"))], /valid date/, "and says why");
input(K("date")).type("12102020");
input(K("date")).fire("blur");
await settle();

// The calendar: its month and year lists move the grid; a day fills the box.
const pid = idOf(K("date"));
frame();
clicks.get(`fp-date-${pid}`).fn({}, context());
const cal = ui.state.picker.cal;
assert.deepEqual([cal.year(), cal.month()], [2020, 12], "the calendar opens on the box's date");
frame();
const months = dropdowns.get(`fp-date-month-${pid}`), years = dropdowns.get(`fp-date-year-${pid}`);
assert.ok(months && years, "the calendar has a month list and a year list");
assert.equal(months.options.length, 12);
assert.ok(years.options.some((o) => o.value === "1900") && years.options.some((o) => o.value === "2036"), "years run from 1900 to ten years ahead");
months.onOpenChange(true, context());
assert.equal(ui.state.picker.select, "month", "the month list opens over the calendar");
months.onChange("3", context());
assert.deepEqual([cal.year(), cal.month()], [2020, 3], "a month changes the month");
assert.equal(ui.state.picker.select, null, "and closes its list");
years.onChange("1985", context());
assert.deepEqual([cal.year(), cal.month()], [1985, 3], "a year changes the year");
moveCalendar(cal, 2026, 1);
assert.deepEqual([cal.year(), cal.month()], [2026, 1]);
moveCalendar(cal, 1985, 3);
cal.pick("1985-03-07");
assert.equal(lastFill()[3][K("date")], "03/07/1985", "a picked day reaches the draft as MM/DD/YYYY");
assert.equal(input(K("date")).value(), "Mar 7, 1985", "and shows as Mar 7, 1985");
console.log("PASS rule 4: dates show as Dec 10, 2020 and the calendar changes month and year.");
