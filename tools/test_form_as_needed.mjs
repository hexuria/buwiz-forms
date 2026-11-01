// PR 9: 1706 Part IV Schedule 2 item 7, Date of Installment (MM/DD/YYYY),
// driven through the real app/shell/form_page.js with a mocked host (the
// harness of tools/test_form_q3_income_tax.mjs, PR 5). The host marks the
// item `one_date` (screen_form_module/rows_as_needed.rs mark_one_date) over
// its three boxes, frm1706:txtDateMonthI, txtDateDayI and txtDateYearI.
// Typed or picked, the date reaches the host as its three boxes in one
// batch, MM, DD and YYYY; a blank box clears the three; a locked return
// shows the date; a blank locked date says why it waits (the host's
// `waits`, as 2552's item 1 sends), as every locked box does. Which keys
// are parts of one date, and the whole-date check, are the host's to say
// (crates/bir-shell screen_form_module tests::as_needed).
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);

const K = (tail) => `frm1706:${tail}`;
const schema = {
  c: "1706", t: "Capital Gains Tax Return (For Onerous Transfer of Real Property Classified as Capital Asset)", rev: "2018", freq: "As needed", pages: 1, n: 2,
  parts: [
    {
      n: "Part IV · Schedule 2",
      items: [
        { no: "6", l: "Total", f: [{ k: K("txtTotalN"), t: "money", l: "Total" }] },
        {
          no: "7", l: "Date of Installment (MM/DD/YYYY)", one_date: true,
          f: [
            { k: K("txtDateMonthI"), t: "int", l: "Month" },
            { k: K("txtDateDayI"), t: "int", l: "Day" },
            { k: K("txtDateYearI"), t: "int", l: "Year" },
          ],
        },
      ],
    },
  ],
};
const [MONTH, DAY, YEAR] = ["txtDateMonthI", "txtDateDayI", "txtDateYearI"].map(K);
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
const kit = mock("gpui-kit", { div: element, svg: element, View, list: (_id, n, _key, render) => { const el = element(); for (let i = 0; i < n; i++) el.child(render(i)); return el; } });
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

let hostValues = { [K("txtTotalN")]: "0.00", [MONTH]: "03", [DAY]: "05", [YEAR]: "2026" };
let editable = true;
let locks = {};
let waits = {};
const opened = () => ({
  draft: { id: 7 }, status: editable ? "Draft" : "Submitted", editable, values: { ...hostValues }, options: {}, locks: { ...locks },
  waits: { ...waits }, profile: [], profile_edit: [], sample: false, blank: [], needs: {}, off: {}, penalties: [], pools: {}, errors: [],
});
const app = {
  prefs: {}, branchFor: () => null, taxpayers: () => [], layout: () => ({ column: 1056 }),
  invalidate() {}, toast() {}, startNewReturn() {},
  toastState: null,
  toastUndo(what, run) { this.toastState = { text: what, action: { run } }; },
  store: {
    templates: () => [],
    write(name, ...args) { calls.push([name, ...args]); return { id: 7, status: "Draft", draft: args[1] }; },
    call(name, ...args) {
      calls.push([name, ...args]);
      if (name === "screen_form_schema") return schema;
      if (name === "screen_form_open") return Promise.resolve(opened());
      if (name === "screen_form_fill") {
        // Like the host, a fill works on the draft it is given (an Undo
        // refills the draft it was offered with).
        const values = { ...(args[1].values || hostValues), ...args[2] };
        return { draft: { id: 7, values }, values: { ...values }, options: {}, rejected: {}, penalties: [], pools: {}, removes: null };
      }
      if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: { ...(args[1].values || hostValues) }, options: {}, errors: [], off: {}, penalties: [], pools: {} });
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
  ui.open({ code: "1706", tin: "12345678800000", year: 2026, period: 1 }, cx);
  await settle();
  ui.ensureInputs(0, cx);
  ui.state.openParts = new Set([0]);
  frame();
}


// ---- The three boxes read as one date.
await openAll();
const one = ui.state.inputs.get(ONE);
assert.ok(one, "item 7 has one date box");
assert.equal(one.value(), "Mar 5, 2026", "it shows the three boxes as Mar 5, 2026");
for (const k of [MONTH, DAY, YEAR]) assert.equal(ui.state.inputs.has(k), false, `${k} has no box of its own`);
assert.ok(clicks.has(`fp-date-${idOf()}`), "with one calendar");
console.log("PASS 1706 Schedule 2 item 7: the month, day and year boxes show as one date.");

// ---- Typed: the three boxes go to the host together.
for (const typed of ["04152026", "04/15/2026", "Apr 15, 2026"]) {
  one.type("01012000");
  one.type(typed);
  assert.deepEqual([ui.state.pending[MONTH], ui.state.pending[DAY], ui.state.pending[YEAR]], ["04", "15", "2026"], `${typed} stages MM, DD and YYYY`);
  one.fire("blur");
  assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [MONTH]: "04", [DAY]: "15", [YEAR]: "2026" }), "one batch with the three boxes");
  assert.equal(one.value(), "Apr 15, 2026", `${typed} shows as Apr 15, 2026`);
}
await settle();
const before = fills().length;
one.type("02312026");
assert.equal(Object.keys(ui.state.pending).length, 0, "a date that is not one sends nothing");
one.fire("blur");
assert.equal(fills().length, before, "and fills nothing");
assert.equal(ui.state.dateErrors[idOf()], "Enter a valid date as MM/DD/YYYY (e.g. 12/01/2020 or 12012020)", "and says why, as the host does");
one.type("");
assert.deepEqual([ui.state.pending[MONTH], ui.state.pending[DAY], ui.state.pending[YEAR]], ["", "", ""], "a blank box clears the three");
one.fire("blur");
await settle();
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [MONTH]: "", [DAY]: "", [YEAR]: "" }));
console.log("PASS 1706 Schedule 2 item 7: a typed date reaches the draft as its three boxes; a blank clears them.");

// ---- Picked: the calendar fills the three and the box shows the day.
one.type("03052026");
one.fire("blur");
await settle();
frame();
act(clicks.get(`fp-date-${idOf()}`).fn, {});
const cal = ui.state.picker.cal;
assert.deepEqual([cal.year(), cal.month()], [2026, 3], "the calendar opens on the date");
cal.pick("2026-06-30");
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [MONTH]: "06", [DAY]: "30", [YEAR]: "2026" }), "a picked day fills the three boxes");
assert.equal(one.value(), "Jun 30, 2026", "and shows as Jun 30, 2026");
console.log("PASS 1706 Schedule 2 item 7: a picked day fills the three boxes.");

// ---- A return that is no longer a draft shows the date, locked.
editable = false;
hostValues = { ...hostValues, [MONTH]: "03", [DAY]: "05", [YEAR]: "2026" };
await openAll();
assert.ok(texts.includes("Mar 5, 2026"), "a locked item shows the date");
assert.ok(!ids.has(`fp-date-${idOf()}`), "and no calendar");
console.log("PASS 1706 Schedule 2 item 7: a locked return shows the date.");

// ---- A blank locked date says why it waits, in the box's place.
editable = true;
const why = "Set by the date of transaction you start this return with (New return).";
hostValues = { ...hostValues, [MONTH]: "", [DAY]: "", [YEAR]: "" };
locks = { [MONTH]: "identity", [DAY]: "identity", [YEAR]: "identity" };
waits = { [MONTH]: why, [DAY]: why, [YEAR]: why };
await openAll();
assert.ok(texts.includes(why), "the blank locked date shows its reason");
assert.ok(!ids.has(`fp-date-${idOf()}`), "and no calendar");
hostValues = { ...hostValues, [MONTH]: "03", [DAY]: "05", [YEAR]: "2026" };
await openAll();
assert.ok(texts.includes("Mar 5, 2026") && !texts.includes(why), "a locked date that has one shows the date, not the reason");
console.log("PASS one date box: a blank locked date shows what it waits for.");
