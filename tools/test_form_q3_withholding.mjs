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
//    Choose..., never a raw 0;
//  - an answer that removes rows the filer typed (the host's `removes`:
//    1601FQ items 4, 11 and 13, a cleared ATC with a tax base, 1602Q item
//    13) asks first: Cancel keeps the old answer and the rows, Continue
//    applies it and offers Undo, which brings both back; with nothing
//    typed it asks nothing.
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
        { no: "13", l: "Special Law or International Tax Treaty?", f: [{ k: K("yes"), t: "radio", l: "Yes" }, { k: K("no"), t: "radio", l: "No" }] },
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

const entities = [], tasks = [], texts = [], timers = [], undos = [];
// The answers that remove typed rows, with what the host says (its own
// tests pin each message: q3_withholding_removing_typed_rows_is_reported,
// q3_withholding_item_13_is_answered_on_the_page_and_prints).
const TRIGGERS = [
  { form: "1601FQ item 13", key: `frmTEST:no`, value: "true", edits: { "frmTEST:no": "true", "frmTEST:yes": "false" }, message: "Changing item 13 to No removes the 1 Schedule 1 row you typed. Continue?" },
  { form: "1601FQ item 4", key: `frmTEST:withheldNo`, value: "true", edits: { "frmTEST:withheldNo": "true" }, message: "Changing item 4 to No removes the 1 Part II ATC row you typed. Continue?" },
  { form: "1601FQ item 11", key: `frmTEST:government`, value: "true", edits: { "frmTEST:government": "true" }, message: "Changing item 11 removes the 1 Part II ATC row you typed. Continue?" },
  { form: "1601FQ cleared ATC", key: `frmTEST:atc1`, value: "", edits: { "frmTEST:atc1": "" }, message: "Clearing this ATC removes its row and its tax base of ₱1,000.00. Continue?" },
  { form: "1602Q item 13", key: `frmTEST:q2no`, value: "true", edits: { "frmTEST:q2no": "true" }, message: "Changing item 13 to No removes the 1 Schedule 2 row you typed. Continue?" },
];
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
function act(fn, ...args) { return fn(...args, context()); }
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
const { FormPageActions, renderFormPage, renderRemovalConfirm } = mod.namespace;

let hostValues = {
  [K("atc1")]: "WI330", [K("base1")]: "1000", [K("rate1")]: " 25.0",
  [K("atc2")]: "", [K("base2")]: "", [K("rate2")]: "",
  [K("over")]: "0.00", [K("relief")]: "0", [K("value1")]: "0.00", [K("value2")]: "0.00",
  [K("yes")]: "true", [K("no")]: "false", [K("value1")]: "0.00",
};
const opened = () => ({
  draft: { id: 7, rows: 1 }, status: "Draft", editable: true, values: { ...hostValues }, options: {},
  locks: { [K("rate1")]: "cannot be filled here: derived by the app", [K("rate2")]: "cannot be filled here: derived by the app" },
  lock_notes: { [K("rate1")]: "Can't be filled on this page yet.", [K("rate2")]: "Can't be filled on this page yet." },
  profile: [], profile_edit: [], sample: false, blank: [], needs: {}, waits: { [K("rate1")]: WAITS, [K("rate2")]: WAITS },
  off: {}, penalties: [], pools: {}, errors: [],
});
const app = {
  prefs: {}, branchFor: () => null, taxpayers: () => [], layout: () => ({ column: 1056 }),
  invalidate() {}, toast() {}, startNewReturn() {},
  toastUndo(message, undo) { undos.push({ message, undo }); },
  store: {
    templates: () => [],
    write(name, ...args) { calls.push([name, ...args]); return { id: 7, status: "Draft", draft: args[1] }; },
    call(name, ...args) {
      calls.push([name, ...args]);
      if (name === "screen_form_schema") return schema;
      if (name === "screen_form_open") return Promise.resolve(opened());
      if (name === "screen_form_fill") {
        // The host: a draft carries its typed Schedule 1 row count; item 13
        // No removes them and says so.
        const draft = { ...args[1] }, edits = args[2];
        let removes = null;
        const trigger = TRIGGERS.find((t) => edits[t.key] === t.value);
        if (trigger) {
          if (draft.rows > 0) removes = { message: trigger.message, done: `Removed ${draft.rows} row`, rows: draft.rows };
          draft.rows = 0;
        }
        const values = { ...hostValues, ...edits, [K("value1")]: draft.rows ? "1,000.00" : "0.00" };
        if (!Object.keys(edits).length) Object.assign(values, draft.answer || {});
        draft.answer = { [K("yes")]: values[K("yes")], [K("no")]: values[K("no")] };
        return { draft, values, options: {}, rejected: {}, penalties: [], pools: {}, removes };
      }
      if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: { ...hostValues, ...(args[1].answer || {}), [K("value1")]: args[1].rows ? "1,000.00" : "0.00" }, options: {}, errors: [], off: {}, penalties: [], pools: {} });
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
// In a sample, a held box with no `needs` of its own still says what it waits for.
ui.state.blank = new Set([K("rate2")]);
assert.equal(ui.reasonFor(K("rate2")), WAITS, "a sample falls back to waits");
ui.state.needs = { [K("rate2")]: "Shows once the period is set" };
assert.equal(ui.reasonFor(K("rate2")), "Shows once the period is set", "a sample's own needs come first");
ui.state.blank = new Set();
ui.state.needs = {};
console.log("PASS a blank rate says it waits for its row's ATC.");

// ---- The official blank 0 of a dropdown.
const relief = dropdowns.get(`fp-${K("relief")}`);
assert.equal(relief.value, "", "13A's 0 shows as Choose...");
assert.equal(JSON.stringify(relief.options.map((o) => o.label)), JSON.stringify(["Choose…", "Special Rate", "International Tax Treaty", "Both"]), "and lists the reliefs by name");
console.log("PASS a dropdown's official blank 0 shows Choose..., never a raw 0.");

// ---- An answer that removes typed rows asks first.
for (const t of TRIGGERS.slice(1)) {
  ui.state.draft = { ...ui.state.draft, rows: 1 };
  const kept = ui.state.draft;
  ui.choose(t.edits, cx);
  assert.equal(app.overlay, "removal", `${t.form} asks first`);
  texts.length = 0;
  renderRemovalConfirm(cx, app);
  assert.ok(texts.includes(t.message), `${t.form}: ${t.message}`);
  ui.cancelRemoval(cx);
  assert.equal(ui.state.draft, kept, `${t.form}: Cancel keeps the rows`);
  ui.choose(t.edits, cx);
  ui.confirmRemoval(cx);
  await settle();
  assert.equal(ui.state.draft.rows, 0, `${t.form}: Continue removes them`);
  act(undos.at(-1).undo);
  await settle();
  assert.equal(ui.state.draft.rows, 1, `${t.form}: Undo brings them back`);
}
undos.length = 0;
const no = TRIGGERS[0].edits;
const typedDraft = ui.state.draft;
ui.choose(no, cx);
assert.equal(app.overlay, "removal", "item 13 No with a typed row asks first");
assert.equal(ui.state.values[K("no")], "false", "the old answer shows while it asks");
assert.equal(ui.state.draft, typedDraft, "and the draft is the old one");
texts.length = 0;
renderRemovalConfirm(cx, app);
assert.ok(texts.includes("Changing item 13 to No removes the 1 Schedule 1 row you typed. Continue?"), "it says what goes");
assert.ok(ids.has("removal-continue") && ids.has("removal-cancel"), "with Continue and Cancel");
ui.cancelRemoval(cx);
assert.equal(app.overlay, null, "Cancel closes it");
assert.equal(ui.state.values[K("no")], "false", "Cancel keeps the old answer");
assert.equal(ui.state.draft.rows, 1, "and the rows");
assert.equal(undos.length, 0, "and offers no Undo");
ui.choose(no, cx);
ui.confirmRemoval(cx);
await settle();
assert.equal(app.overlay, null);
assert.equal(ui.state.values[K("no")], "true", "Continue applies the answer");
assert.equal(ui.state.draft.rows, 0, "and the removal");
assert.equal(undos.length, 1, "and offers Undo");
assert.equal(undos[0].message, "Removed 1 row.");
act(undos[0].undo);
await settle();
assert.equal(ui.state.draft.rows, 1, "Undo brings the rows back");
assert.equal(ui.state.values[K("no")], "false", "and the old answer");
assert.equal(ui.state.values[K("value1")], "1,000.00", "and what the rows showed");
// With nothing typed, nothing is asked.
ui.state.draft = { ...ui.state.draft, rows: 0 };
ui.choose(no, cx);
assert.equal(app.overlay ?? null, null, "no rows typed: no question");
assert.equal(ui.state.values[K("no")], "true", "the answer applies at once");
assert.equal(undos.length, 1, "and no Undo is offered");
console.log("PASS an answer that removes typed rows asks first, and Undo brings them back.");
