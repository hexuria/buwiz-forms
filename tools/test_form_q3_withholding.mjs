// The form page's pieces the Q3 withholding remittances (PR 2) added, driven
// through the real app/shell/form_page.js with a mocked host. Which boxes
// they are on 1601EQ, 1601FQ, 1602Q and 1603Q is the host's to say
// (crates/bir-shell screen_form_module q3_* and form_160* tests); this
// proves what the page does with them:
//  - a printed item with no box (1601EQ item 24) shows its number, caption
//    and note, and draws no control;
//  - a schedule table with preprinted row captions (1603Q Schedule I's
//    recipients) shows them in their own column;
//  - a locked, blank rate says what it waits for (the host's `waits`), in
//    its box and not as a lock note; with a value it shows the value;
//  - a dropdown whose value is the official blank 0 (1602Q's 13A) shows
//    Choose..., never a raw 0.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);

const K = (tail) => `frmTEST:${tail}`;
const WAITS = "Shows the ATC's rate once this row's ATC is chosen.";
const NOTE = "The eBIRForms program has no box for this item, so nothing is filed here and it prints blank.";
const schema = {
  c: "TEST", t: "Test return", rev: "2026", freq: "Quarterly", pages: 1, n: 4, q: 1,
  parts: [
    {
      n: "Part II · Computation of Tax",
      items: [
        { no: "14", l: "ATC (Items 14 to 19)", rows: 2, start: 14, cols: [{ l: "ATC", t: "sel" }, { l: "Tax Base", t: "money" }, { l: "Tax Rate", t: "pct", c: true }], keys: [[K("atc1"), K("base1"), K("rate1")], [K("atc2"), K("base2"), K("rate2")]] },
        { no: "23", l: "Over-remittance", f: [{ k: K("over"), t: "money", l: "Over-remittance" }] },
        { no: "24", l: "Other Payments Made", p: 1, f: [], note: NOTE },
        { no: "13A", l: "If yes, specify", f: [{ k: K("relief"), t: "sel", l: "If yes, specify", o: [{ value: "1", label: "Special Rate" }, { value: "2", label: "International Tax Treaty" }, { value: "3", label: "Both" }] }] },
      ],
    },
    {
      n: "Part IV · Schedule 1",
      items: [
        { no: "", l: "Schedule 1 (Items 1 and 2)", rows: 2, cols: [{ l: "Monetary Value (C)", t: "money" }], keys: [[K("value1")], [K("value2")]], row_labels: { caption: "Recipients (A)", rows: ["In General, for Citizen", "Non-Resident Alien Not Engaged"] } },
      ],
    },
  ],
};

const entities = [], tasks = [], texts = [], timers = [];
const ids = new Set();
const calls = [];
let owner = null;
class View {}
function context(entity = null) {
  return {
    stale: false,
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
function element() {
  const target = {};
  const el = new Proxy(target, { get(o, k) {
    if (k === "then") return undefined;
    return (...args) => {
      if (k === "id") { o.id = args[0]; ids.add(o.id); }
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
const InputState = { new({ value = "" } = {}) { let text = value; const on = {}; return { value: () => text, set_value: (v) => { text = v; }, on(ev, fn) { on[ev] = fn; }, release() { return true; } }; } };
const CalendarState = { new() { return { set_value() {}, value: () => null, year: () => 2026, month: () => 10, today: () => "2026-10-17", month_days: () => [[]], on() {}, release() { return true; } }; } };
const contextVM = vm.createContext({ console, Map, Set, JSON, Date, Math, Number, String, Object, Array, window: { viewport_size: () => ({ width: 1400, height: 900 }) } });
function mock(name, exports) { return new vm.SyntheticModule(Object.keys(exports), function () { for (const [k, v] of Object.entries(exports)) this.setExport(k, v); }, { context: contextVM, identifier: name }); }
const kit = mock("gpui-kit", { div: element, svg: element, View });
const base = mock("gpui-base", { InputState, CalendarState, Popup: { new: element } });
const components = mock("gpui-component", { Spinner: { new: element } });
const uiExports = { tokens: () => new Proxy({}, { get: () => "#000" }), SPACE: [0, 4, 8, 12, 16, 20, 24], RADIUS: {}, SIZE: { validationLine: 18 }, BREAK: { formNav: 760 } };
for (const n of ["Text", "Line", "Code", "Icon", "AppButton", "IconButton", "Avatar", "Chip", "Switch", "Checkbox", "TextField", "Dropdown", "PageHeader", "MetaLine", "Overlay", "CheckboxLabel"]) uiExports[n] = element;
const dropdowns = new Map();
uiExports.AppButton = (_cx, id) => { const el = element(); el.id(id); return el; };
uiExports.IconButton = (_cx, id) => { const el = element(); el.id(id); return el; };
uiExports.Dropdown = (_cx, id, opts) => { dropdowns.set(id, opts); const el = element(); el.id(id); return el; };
uiExports.TextField = (_cx, _input, opts = {}) => { const el = element(); el.id(opts.id); return el; };
uiExports.Text = (_cx, _variant, text) => { texts.push(String(text)); return element(); };
uiExports.Line = uiExports.Text;
const store = mock("./store.js", { formatTin: (x) => x, errorText: (e) => String(e && e.message ? e.message : e) });
const periods = mock("./periods.js", { periodLabel: () => "Q3 2026" });
const mod = new vm.SourceTextModule(fs.readFileSync(new URL("app/shell/form_page.js", root), "utf8"), { context: contextVM });
await mod.link((s) => ({ "gpui-kit": kit, "gpui-base": base, "gpui-component": components, "./ui.js": mock("./ui.js", uiExports), "./store.js": store, "./periods.js": periods })[s]);
await mod.evaluate();
const { FormPageActions, renderFormPage } = mod.namespace;

let hostValues = {
  [K("atc1")]: "WI330", [K("base1")]: "1000", [K("rate1")]: " 25.0",
  [K("atc2")]: "", [K("base2")]: "", [K("rate2")]: "",
  [K("over")]: "0.00", [K("relief")]: "0", [K("value1")]: "0.00", [K("value2")]: "0.00",
};
const opened = () => ({
  draft: { id: 7 }, status: "Draft", editable: true, values: { ...hostValues }, options: {},
  locks: { [K("rate1")]: "cannot be filled here: derived by the app", [K("rate2")]: "cannot be filled here: derived by the app" },
  lock_notes: { [K("rate1")]: "Can't be filled on this page yet.", [K("rate2")]: "Can't be filled on this page yet." },
  profile: [], profile_edit: [], sample: false, blank: [], needs: {}, waits: { [K("rate1")]: WAITS, [K("rate2")]: WAITS },
  off: {}, penalties: [], pools: {}, errors: [],
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
      if (name === "screen_form_fill") return { draft: args[1], values: { ...hostValues, ...args[2] }, options: {}, rejected: {}, penalties: [], pools: {} };
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
function frame() { owner = null; texts.length = 0; ids.clear(); for (const e of entities) e.dirty = true; renderFormPage(cx, app, 1056); }
ui.open({ code: "TEST", tin: "12345678800000", year: 2026, period: 3 }, cx);
await settle();
for (let i = 0; i < schema.parts.length; i++) ui.ensureInputs(i, cx);
ui.state.openParts = new Set(schema.parts.map((_p, i) => i));
frame();

// ---- A printed item with no box.
assert.ok(texts.includes("Other Payments Made"), "item 24's caption shows");
assert.ok(ids.has("fp-item-0:2"), "as an item row of its own");
assert.ok(texts.includes(NOTE), "and its note");
assert.ok(![...ids].some((id) => /^fp-frmTEST:/.test(id) && id.includes("24")), "and no control");
const part = ui.state.schema.parts[0];
assert.ok(part.items.some((it) => it.no === "24"), "the page keeps an item that has only its note");
console.log("PASS a printed item with no box shows its caption and note.");

// ---- Preprinted row captions.
assert.ok(texts.includes("Recipients (A)"), "the row caption column has its heading");
assert.ok(texts.includes("In General, for Citizen") && texts.includes("Non-Resident Alien Not Engaged"), "each row shows its printed caption");
console.log("PASS a schedule shows its preprinted row captions.");

// ---- A rate that waits for its ATC.
assert.ok(texts.includes(" 25.0"), "a chosen row's rate shows its value");
assert.equal(texts.filter((t) => t === WAITS).length, 1, "the blank row's rate says what it waits for, once");
assert.equal(ui.reasonFor(K("rate2")), WAITS);
assert.equal(ui.reasonFor(K("rate1")), WAITS, "the reason is there; the box shows its value instead");
assert.ok(!texts.includes("Can't be filled on this page yet."), "no lock note under the table for a box that says what it waits for");
assert.equal(ui.lockNotesFor(part.items[0]).length, 0, "the table has no lock note");
console.log("PASS a blank rate says it waits for its row's ATC.");

// ---- The official blank 0 of a dropdown.
const relief = dropdowns.get(`fp-${K("relief")}`);
assert.equal(relief.value, "", "13A's 0 shows as Choose...");
assert.equal(JSON.stringify(relief.options.map((o) => o.label)), JSON.stringify(["Choose…", "Special Rate", "International Tax Treaty", "Both"]), "and lists the reliefs by name");
console.log("PASS a dropdown's official blank 0 shows Choose..., never a raw 0.");
