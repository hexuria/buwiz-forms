// PR 14 review F9 (decision P4-16, "Untick: leave it unanswered"): the page
// unticks the ticked box of a one-answer group of three or more by the
// group's size the host gives each box (`rg`, screen_form_module.rs
// mark_radio_groups), not by the radios drawn in one item. 2200S's manner of
// payment is items 15 and 16 (radios) and item 17 (a tick box); before,
// each item counted one radio, so re-clicking Payment on Actual Removal or
// Prepayment sent the answer again and the manner could not be unticked.
// Driven through the real app/shell/form_page.js with a mocked host (the
// harness of tools/test_form_as_needed.mjs). What the host then writes is
// its own (crates/bir-shell screen_form_module tests::radio_untick_rule).
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);

const K = (tail) => `frm2200S:${tail}`;
const [ACTUAL, PREPAY, OTHER] = ["chkPymntManner_1", "chkPymntManner_2", "chkPymntManner_3"].map(K);
const [YES, NO] = ["optAmended_1", "optAmended_2"].map(K);
const schema = {
  c: "2200S", t: "Excise Tax Return for Sweetened Beverages", rev: "2018", freq: "As needed", pages: 1, n: 2,
  parts: [
    {
      n: "Part I · Background Information",
      items: [
        { no: "2", l: "Amended Return?", f: [{ k: YES, t: "radio", l: "Yes", rg: 2 }, { k: NO, t: "radio", l: "No", rg: 2 }] },
      ],
    },
    {
      n: "Part II · Manner Of Payment",
      items: [
        { no: "15", l: "", f: [{ k: ACTUAL, t: "radio", l: "Payment on Actual Removal", rg: 3 }] },
        { no: "16", l: "", f: [{ k: PREPAY, t: "radio", l: "Prepayment/Advance Deposit", rg: 3 }] },
        { no: "17", l: "", f: [{ k: OTHER, t: "chk", l: "Other Similar Schemes", rg: 3 }, { k: K("txtOthMannerofPymnt"), t: "text", l: "Specify" }] },
      ],
    },
  ],
};

const entities = [], tasks = [], texts = [], timers = [];
const clicks = new Map(), downOut = new Map(), opacity = new Map(), ids = new Set();
const checkboxes = new Map();
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
const CalendarState = { new() { return { set_value() {}, value: () => null, year: () => 2026, month: () => 10, today: () => "2026-10-17", month_days: () => [[]], next_month() {}, prev_month() {}, on() {}, pick() {}, release() { return true; } }; } };
const contextVM = vm.createContext({ console, Map, Set, JSON, Date, Math, Number, String, Object, Array, window: { viewport_size: () => ({ width: 1400, height: 900 }) } });
function mock(name, exports) { return new vm.SyntheticModule(Object.keys(exports), function () { for (const [k, v] of Object.entries(exports)) this.setExport(k, v); }, { context: contextVM, identifier: name }); }
const kit = mock("gpui-kit", { div: element, svg: element, View });
const base = mock("gpui-base", { InputState, CalendarState, Popup: { new: element } });
const components = mock("gpui-component", { Spinner: { new: element } });
const uiExports = { tokens: () => new Proxy({}, { get: () => "#000" }), SPACE: [0, 4, 8, 12, 16, 20, 24], RADIUS: {}, SIZE: { validationLine: 18 }, BREAK: { formNav: 760 } };
for (const n of ["Text", "Line", "Code", "Icon", "AppButton", "IconButton", "Avatar", "Chip", "Switch", "Checkbox", "TextField", "Dropdown", "PageHeader", "MetaLine", "Overlay", "CheckboxLabel"]) uiExports[n] = element;
uiExports.AppButton = (_cx, id, _label, opts = {}) => { const el = element(); el.id(id); if (opts.onClick) el.on_click(opts.onClick); return el; };
uiExports.IconButton = (_cx, id, _icon, opts = {}) => { const el = element(); el.id(id); if (opts.onClick) el.on_click(opts.onClick); return el; };
uiExports.Checkbox = (_cx, id, checked, onChange) => { checkboxes.set(id, { checked, onChange }); const el = element(); el.id(id); return el; };
uiExports.Text = (_cx, _variant, text) => { texts.push(String(text)); return element(); };
uiExports.Line = uiExports.Text;
const store = mock("./store.js", { formatTin: (x) => x, errorText: (e) => String(e && e.message ? e.message : e) });
const periods = mock("./periods.js", { periodLabel: () => "October 2026" });
const mod = new vm.SourceTextModule(fs.readFileSync(new URL("app/shell/form_page.js", root), "utf8"), { context: contextVM });
await mod.link((s) => ({ "gpui-kit": kit, "gpui-base": base, "gpui-component": components, "./ui.js": mock("./ui.js", uiExports), "./store.js": store, "./periods.js": periods })[s]);
await mod.evaluate();
const { FormPageActions, renderFormPage } = mod.namespace;

let hostValues = {};
const opened = () => ({
  draft: { id: 7 }, status: "Draft", editable: true, values: { ...hostValues }, options: {}, locks: {},
  waits: {}, profile: [], profile_edit: [], sample: false, blank: [], needs: {}, off: {}, penalties: [], pools: {}, errors: [],
});
const app = {
  prefs: {}, branchFor: () => null, taxpayers: () => [], layout: () => ({ column: 1056 }),
  invalidate() {}, toast() {}, startNewReturn() {}, toastState: null,
  toastUndo(what, run) { this.toastState = { text: what, action: { run } }; },
  store: {
    templates: () => [],
    write(name, ...args) { calls.push([name, ...args]); return { id: 7, status: "Draft", draft: args[1] }; },
    call(name, ...args) {
      calls.push([name, ...args]);
      if (name === "screen_form_schema") return schema;
      if (name === "screen_form_open") return Promise.resolve(opened());
      if (name === "screen_form_fill") {
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
function frame() { owner = null; texts.length = 0; ids.clear(); opacity.clear(); clicks.clear(); checkboxes.clear(); for (const e of entities) e.dirty = true; renderFormPage(cx, app, 1056); }
const fills = () => calls.filter((c) => c[0] === "screen_form_fill");
const lastEdits = () => JSON.stringify(fills().at(-1)[3]);
async function openWith(values) {
  hostValues = values;
  ui.open({ code: "2200S", tin: "12345678800000", year: 2026, period: 1 }, cx);
  await settle();
  ui.state.openParts = new Set([0, 1]);
  frame();
}

// ---- Re-clicking each ticked radio of the manner sends its untick.
for (const key of [ACTUAL, PREPAY]) {
  await openWith({ [key]: "true" });
  assert.ok(clicks.has(`fp-${key}`), `${key} is drawn as a radio`);
  const before = fills().length;
  act(clicks.get(`fp-${key}`).fn, {});
  assert.equal(fills().length, before + 1, `${key}: one fill`);
  assert.equal(lastEdits(), JSON.stringify({ [key]: "false" }), `${key}: re-clicked, it is unticked (the manner is left unanswered)`);
}
// An unticked radio of the group still picks its answer.
await openWith({ [ACTUAL]: "true" });
act(clicks.get(`fp-${PREPAY}`).fn, {});
assert.equal(lastEdits(), JSON.stringify({ [PREPAY]: "true" }), "picking another manner ticks it");
console.log("PASS 2200S manner of payment: re-clicking a ticked radio of items 15-16 unticks it.");

// ---- Item 17's tick box unticks as a tick box.
await openWith({ [PREPAY]: "true", [OTHER]: "true" });
const other = checkboxes.get(`fp-${OTHER}`);
assert.ok(other && other.checked, "item 17 shows ticked");
act(other.onChange, false);
assert.equal(lastEdits(), JSON.stringify({ [OTHER]: "false" }), "item 17 unticked sends its untick");
console.log("PASS 2200S manner of payment: item 17's tick box unticks.");

// ---- A yes/no pair keeps one answer: re-picking it sends it again.
await openWith({ [YES]: "true" });
act(clicks.get(`fp-${YES}`).fn, {});
assert.equal(lastEdits(), JSON.stringify({ [YES]: "true", [NO]: "false" }), "a pair's answer given again runs again");
console.log("PASS a yes/no pair keeps its official re-pick.");
