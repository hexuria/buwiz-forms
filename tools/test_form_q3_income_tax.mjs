// PR 5: what the 1701Q page does that the shared mechanisms do not cover,
// driven through the real app/shell/form_page.js with a mocked host and a
// small form: item 11's month, day and year boxes (the host marks the item
// `one_date`, screen_form_module/rows_income_q.rs mark_one_date) are one
// date box. Typed (05171980, 05/17/1980, "May 17, 1980") or picked, the
// date reaches the host as its three boxes in one batch, MM, DD and YYYY;
// a blank box clears the three; a date that is not one says so and sends
// nothing; the box follows the three boxes when the host changes them (an
// Undo); a locked item shows the date. Which keys are parts of one date is
// the host's to say (crates/bir-shell screen_form_module tests).
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);

const K = (tail) => `frmTEST:${tail}`;
const schema = {
  c: "TEST", t: "Test return", rev: "2026", freq: "Quarterly", pages: 1, n: 4, q: 1,
  parts: [
    {
      n: "Part I · Background Information",
      items: [
        { no: "9", l: "Taxpayer/Filer's Name", f: [{ k: K("name"), t: "text", l: "Taxpayer/Filer's Name" }] },
        {
          no: "11", l: "Date of Birth (MM/DD/YYYY)", one_date: true,
          f: [
            { k: K("txtBirthMonth"), t: "int", l: "Date of Birth" },
            { k: K("txtBirthDay"), t: "int", l: "Date of Birth" },
            { k: K("txtBirthYear"), t: "int", l: "Date of Birth" },
          ],
        },
        {
          no: "12", l: "Date of Registration (three boxes)",
          f: [
            { k: K("regMonth"), t: "int", l: "Month" },
            { k: K("regDay"), t: "int", l: "Day" },
            { k: K("regYear"), t: "int", l: "Year" },
          ],
        },
      ],
    },
  ],
};
const [MONTH, DAY, YEAR] = ["txtBirthMonth", "txtBirthDay", "txtBirthYear"].map(K);
const ONE = `${MONTH}#date`;
const idOf = () => "0:1";

const entities = [], tasks = [], texts = [], timers = [];
const clicks = new Map(), downOut = new Map(), opacity = new Map(), ids = new Set();
let calendarSteps = 0;
const calls = [];
let owner = null;
class View {}
function context(entity = null) {
  return {
    stale: false,
    notify() {
      if (this.stale) throw new Error("stale context: cx used after its handler returned");
      if (entity) entity.dirty = true;
    },
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
function runTimers() { while (timers.length) act(timers.shift()); }
// A host call's context lives only while the handler runs: one used after
// it returns (from a timer, say) throws, as the shell does.
function act(fn, ...args) { const c = context(); try { return fn(...args, c); } finally { c.stale = true; } }
function element() {
  const creationOwner = owner;
  const target = {};
  const el = new Proxy(target, { get(o, k) {
    if (k === "then") return undefined;
    return (...args) => {
      if (k === "id") { o.id = args[0]; ids.add(o.id); }
      if (k === "on_click") clicks.set(o.id, { fn: args[0], entity: creationOwner });
      if (k === "on_mouse_down_out") downOut.set(o.id, { fn: args[0], entity: creationOwner });
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
const InputState = { new({ value = "" } = {}) { let text = value; const on = {}; return { value: () => text, set_value: (v) => { text = v; }, on(ev, fn) { on[ev] = fn; }, fire(ev) { if (on[ev]) act(on[ev], {}); }, type(v) { text = v; if (on.change) act(on.change, {}); }, release() { return true; } }; } };
// The calendar's grid: a month that the arrows (and the lists) step.
const calendars = [];
const CalendarState = { new() {
  let value = null, y = 2026, m = 10, onChange = null;
  const cal = {
    set_value(v) { value = v; if (typeof v === "string") { y = Number(v.slice(0, 4)); m = Number(v.slice(5, 7)); } },
    value: () => value, year: () => y, month: () => m, today: () => "2026-10-17", month_days: () => [[]],
    next_month() { calendarSteps += 1; m += 1; if (m > 12) { m = 1; y += 1; } },
    prev_month() { calendarSteps += 1; m -= 1; if (m < 1) { m = 12; y -= 1; } },
    on(_ev, fn) { onChange = fn; }, pick(day) { value = day; if (onChange) act(onChange, day); }, release() { return true; },
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
for (const n of ["Text", "Line", "Code", "Icon", "AppButton", "IconButton", "Avatar", "Chip", "Switch", "Checkbox", "TextField", "Dropdown", "PageHeader", "MetaLine", "Overlay", "CheckboxLabel"]) uiExports[n] = element;
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
const { FormPageActions, renderFormPage, oneDateText } = mod.namespace;

let hostValues = { [K("name")]: "DELA CRUZ, JUAN", [MONTH]: "05", [DAY]: "17", [YEAR]: "1980", [K("regMonth")]: "01", [K("regDay")]: "02", [K("regYear")]: "2003" };
let editable = true;
const opened = () => ({
  draft: { id: 7 }, status: editable ? "Draft" : "Submitted", editable, values: { ...hostValues }, options: {}, locks: {},
  profile: [], profile_edit: [], sample: false, blank: [], needs: {}, off: {}, penalties: [], pools: {}, errors: [],
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
        return { draft: args[1], values: { ...hostValues }, options: {}, rejected: {}, penalties: [], pools: {} };
      }
      if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: { ...hostValues }, options: {}, errors: [], off: {}, penalties: [], pools: {} });
      if (name === "screen_form_preview_sync" || name === "screen_form_print") return true;
      throw Error(name);
    },
  },
};
app.formUi = new FormPageActions(app);
const ui = app.formUi;
const cx = context();
async function settle() { while (tasks.length) await Promise.all(tasks.splice(0)); }
function frame() { owner = null; texts.length = 0; ids.clear(); opacity.clear(); for (const e of entities) e.dirty = true; renderFormPage(cx, app, 1056); }
const fills = () => calls.filter((c) => c[0] === "screen_form_fill");
const lastFill = () => fills().at(-1);
async function openAll() {
  ui.open({ code: "TEST", tin: "12345678800000", year: 2026, period: 2 }, cx);
  await settle();
  ui.ensureInputs(0, cx);
  ui.state.openParts = new Set([0]);
  frame();
}

// ---- The three boxes read as one date.
assert.equal(oneDateText({ [MONTH]: "05", [DAY]: "17", [YEAR]: "1980" }, { month: { k: MONTH }, day: { k: DAY }, year: { k: YEAR } }), "05/17/1980");
assert.equal(oneDateText({}, { month: { k: MONTH }, day: { k: DAY }, year: { k: YEAR } }), "", "three blank boxes are no date");
await openAll();
const one = ui.state.inputs.get(ONE);
assert.ok(one, "item 11 has one date box");
assert.equal(one.value(), "May 17, 1980", "it shows the three boxes as May 17, 1980");
for (const k of [MONTH, DAY, YEAR]) assert.equal(ui.state.inputs.has(k), false, `${k} has no box of its own`);
assert.ok(clicks.has(`fp-date-${idOf()}`), "with one calendar");
assert.ok(ui.state.inputs.has(K("regMonth")), "an item the host does not mark keeps its three boxes");
console.log("PASS 1701Q item 11: the month, day and year boxes show as one date.");

// ---- Typed: the three boxes go to the host together.
for (const typed of ["12102020", "12-10-2020", "12/10/2020", "Dec 10, 2020"]) {
  one.type("01012000");
  one.type(typed);
  assert.deepEqual([ui.state.pending[MONTH], ui.state.pending[DAY], ui.state.pending[YEAR]], ["12", "10", "2020"], `${typed} stages MM, DD and YYYY`);
  one.fire("blur");
  assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [MONTH]: "12", [DAY]: "10", [YEAR]: "2020" }), "one batch with the three boxes");
  assert.equal(one.value(), "Dec 10, 2020", `${typed} shows as Dec 10, 2020`);
}
await settle();
const before = fills().length;
one.type("13312020");
assert.equal(Object.keys(ui.state.pending).length, 0, "a date that is not one sends nothing");
one.fire("blur");
assert.equal(fills().length, before, "and fills nothing");
assert.match(ui.state.dateErrors[idOf()], /valid date/, "and says why");
assert.equal(one.value(), "13312020", "it stays as typed");
one.type("");
assert.deepEqual([ui.state.pending[MONTH], ui.state.pending[DAY], ui.state.pending[YEAR]], ["", "", ""], "a blank box clears the three");
assert.equal(ui.state.dateErrors[idOf()], undefined, "and has nothing to say");
one.fire("blur");
await settle();
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [MONTH]: "", [DAY]: "", [YEAR]: "" }));
console.log("PASS 1701Q item 11: a typed date reaches the draft as its three boxes; a blank clears them.");

// ---- Picked: the calendar fills the three and the box shows the day.
one.type("05171980");
one.fire("blur");
await settle();
frame();
act(clicks.get(`fp-date-${idOf()}`).fn, {});
const cal = ui.state.picker.cal;
assert.deepEqual([cal.year(), cal.month()], [1980, 5], "the calendar opens on the date");
cal.pick("1985-03-07");
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [MONTH]: "03", [DAY]: "07", [YEAR]: "1985" }), "a picked day fills the three boxes");
assert.equal(one.value(), "Mar 7, 1985", "and shows as Mar 7, 1985");
console.log("PASS 1701Q item 11: a picked day fills the three boxes.");

// ---- The box follows the three boxes when the host changes them (Undo).
ui.applyFill({ draft: { id: 7 }, values: { ...hostValues, [MONTH]: "01", [DAY]: "02", [YEAR]: "2003" }, options: {}, rejected: {}, penalties: [], pools: {} }, {});
assert.equal(one.value(), "Jan 2, 2003", "the host's three boxes show in the one box");
console.log("PASS 1701Q item 11: the date box follows the draft.");

// ---- A return that is no longer a draft shows the date, locked.
editable = false;
hostValues = { ...hostValues, [MONTH]: "05", [DAY]: "17", [YEAR]: "1980" };
await openAll();
assert.ok(texts.includes("May 17, 1980"), "a locked item shows the date");
assert.ok(!ids.has(`fp-date-${idOf()}`), "and no calendar");
console.log("PASS 1701Q item 11: a locked return shows the date.");
