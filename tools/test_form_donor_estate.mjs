// PR 10: what the 1801 page does that the shared mechanisms do not cover,
// driven through the real app/shell/form_page.js with a mocked host and a
// small form shaped like 1801's Part I:
//  - item 11, the executor's TIN, is a TIN the filer types for someone
//    else: the host marks it `typed_tin` (screen_form_module/
//    rows_donor_estate.rs mark_schema), so it has open boxes and no
//    profile mark, while item 5, the estate's own TIN, keeps its mark; a TIN
//    item without the flag keeps the mark too.
//  - item 1, the date of death, is one date box (`one_date`, PR 5's page
//    code): on a new return the host's three boxes are blank, so the box is
//    empty (never "00/00/00"); a typed date reaches the host as MM, DD and
//    YYYY together.
// Which items carry the flags, and the blank values, are the host's to say
// (crates/bir-shell screen_form_module tests::donor_estate).
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);

const K = (tail) => `frm1801v2018:${tail}`;
const tin = (prefix, label) => [1, 2, 3].map((n) => ({ k: K(`${prefix}${n}`), t: "text", l: `${label}${n}` }));
const schema = {
  c: "TEST", t: "Test return", rev: "2018", freq: "As needed", pages: 1, n: 4, q: 1,
  parts: [
    {
      n: "Part I · Taxpayer Information",
      items: [
        { no: "5", l: "Taxpayer Identification Number (TIN)", f: [...tin("tinA_", "TIN"), { k: K("branchCode_1"), t: "text", l: "Branch code" }] },
        { no: "11", l: "TIN of Executor/Administrator", typed_tin: true, f: [...tin("txtTINE", "TIN"), { k: K("txtBranchCodeE"), t: "text", l: "Branch code" }] },
        { no: "99", l: "Another party's TIN (no flag)", f: [...tin("otherTin", "TIN"), { k: K("otherBranch"), t: "text", l: "Branch code" }] },
        {
          no: "1", l: "Date of Death", one_date: true,
          f: [
            { k: K("txtDateMonth"), t: "int", l: "Month" },
            { k: K("txtDateDay"), t: "int", l: "Day" },
            { k: K("txtDateYear"), t: "int", l: "Year" },
          ],
        },
      ],
    },
  ],
};
const [MONTH, DAY, YEAR] = ["txtDateMonth", "txtDateDay", "txtDateYear"].map(K);
const ONE = `${MONTH}#date`;

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

// A new return: the estate's TIN from the profile (locked), the executor's
// blank and open, and the date of death blank (the host shows a 0 part as
// blank).
let hostValues = {
  [K("tinA_1")]: "123", [K("tinA_2")]: "456", [K("tinA_3")]: "788", [K("branchCode_1")]: "00000",
  [K("txtTINE1")]: "", [K("txtTINE2")]: "", [K("txtTINE3")]: "", [K("txtBranchCodeE")]: "",
  [MONTH]: "", [DAY]: "", [YEAR]: "",
};
const locks = {
  [K("tinA_1")]: "derived", [K("tinA_2")]: "derived", [K("tinA_3")]: "derived", [K("branchCode_1")]: "derived",
  [K("otherTin1")]: "derived", [K("otherTin2")]: "derived", [K("otherTin3")]: "derived", [K("otherBranch")]: "derived",
};
const opened = () => ({
  draft: { id: 7 }, status: "Draft", editable: true, values: { ...hostValues }, options: {}, locks: { ...locks },
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

ui.open({ code: "TEST", tin: "12345678800000", year: 2026, period: 1 }, cx);
await settle();
ui.ensureInputs(0, cx);
ui.state.openParts = new Set([0]);
frame();

// ---- Item 11 is the filer's to type, without the profile mark.
assert.ok(ids.has("fp-mark-user-0:0"), "item 5, the estate's TIN, keeps the profile mark");
assert.ok(!ids.has("fp-mark-user-0:1"), "item 11, the executor's TIN, has no profile mark");
assert.ok(ids.has("fp-mark-user-0:2"), "a TIN item the host does not mark keeps the mark");
for (const n of [1, 2, 3]) {
  assert.ok(ui.state.inputs.has(K(`txtTINE${n}`)), `item 11 box ${n} is open`);
  assert.equal(ui.isLocked(K(`txtTINE${n}`)), false, `item 11 box ${n} takes typing`);
}
assert.equal(ui.isLocked(K("tinA_1")), true, "item 5's boxes stay locked");
ui.state.inputs.get(K("txtTINE1")).type("123");
ui.state.inputs.get(K("txtTINE1")).fire("blur");
await settle();
assert.equal(lastFill()[3][K("txtTINE1")], "123", "what is typed in item 11 reaches the host as typed");
console.log("PASS 1801 item 11: the executor's TIN is typed on the return, without the profile mark.");

// ---- Item 1 is one date box, empty on a new return.
const one = ui.state.inputs.get(ONE);
assert.ok(one, "item 1 has one date box");
assert.equal(one.value(), "", "blank on a new return, never 00/00/00");
for (const k of [MONTH, DAY, YEAR]) assert.equal(ui.state.inputs.has(k), false, `${k} has no box of its own`);
one.type("06152025");
one.fire("blur");
await settle();
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [MONTH]: "06", [DAY]: "15", [YEAR]: "2025" }), "one batch with the three boxes");
assert.equal(one.value(), "Jun 15, 2025", "shown as Jun 15, 2025");
console.log("PASS 1801 item 1: the date of death is one box, blank until typed.");
