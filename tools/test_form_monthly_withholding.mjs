// The form page as the monthly withholding forms (PR 3: 0619E, 0619F,
// 1600PT, 1600VT, 1600WP, 1601C) use it, driven through the real
// app/shell/form_page.js with a mocked host. Which boxes these are is the
// host's to say (crates/bir-shell screen_form_module/tests/monthly.rs);
// this proves what the page does with what the host sends:
//  - a printed item with no box (1601C items 37-40, 0619E 19-22, 0619F
//    20-23, 1600WP item 11) shows its number, caption and note, and draws
//    no control;
//  - a locked, blank rate (1600PT/1600VT/1600WP Part II, 1600WP Schedule
//    1.1) says what it waits for (the host's `waits`), in its box; with a
//    value it shows the value;
//  - 1601C item 13A's official blank 0 shows Choose..., and the reliefs are
//    listed by name;
//  - 1600PT/1600VT item 1's two-digit year shows the whole year;
//  - an answer that removes what the filer typed (the host's `removes`:
//    1600PT/1600VT items 3 and 10 and a cleared ATC with its tax base,
//    1600WP items 4 and 7) asks first: Cancel and Esc keep the old answer
//    and the rows, Continue applies it and offers Undo, which brings both
//    back; a later edit or another page retires the Undo.
// The page code is PR 2's (fix/findings-q3-withholding), carried verbatim.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);

const K = (tail) => `frmTEST:${tail}`;
const WAITS = "Shows the ATC's rate once this row's ATC is chosen.";
const NOTE = "eBIRForms keeps these boxes disabled, so this page has nothing to fill here.";
const schema = {
  c: "TEST", t: "Test return", rev: "2026", freq: "Quarterly", pages: 1, n: 4, q: 1,
  parts: [
    {
      n: "Part II · Computation of Tax",
      items: [
        { no: "14", l: "ATC (Items 14 to 19)", rows: 2, start: 14, cols: [{ l: "ATC", t: "sel" }, { l: "Tax Base", t: "money" }, { l: "Tax Rate", t: "pct", c: true }], keys: [[K("atc1"), K("base1"), K("rate1")], [K("atc2"), K("base2"), K("rate2")]] },
        { no: "23", l: "Over-remittance", f: [{ k: K("over"), t: "money", l: "Over-remittance" }] },
        { no: "37", l: "Cash/Bank Debit Memo", p: 1, f: [], note: NOTE },
        { no: "13", l: "Special Law or International Tax Treaty?", f: [{ k: K("yes"), t: "radio", l: "Yes" }, { k: K("no"), t: "radio", l: "No" }] },
        { no: "13A", l: "If yes, specify", f: [{ k: K("relief"), t: "sel", l: "If yes, specify", o: [{ value: "Special Rate", label: "Special Rate" }, { value: "International Tax Treaty", label: "International Tax Treaty" }, { value: "Both", label: "Both" }] }] },
        { no: "1", l: "For the month (MM/20YY)", f: [{ k: K("year"), t: "sel", l: "Filing two-digit year", o: [{ value: "25", label: "2025" }, { value: "26", label: "2026" }] }] },
        { no: "1", l: "For the Period – From (MM/DD/YYYY)", f: [{ k: K("fromMonth"), t: "int", l: "Date withholding month" }, { k: K("fromDay"), t: "int", l: "Date withholding day" }, { k: K("fromYear"), t: "int", l: "Date withholding year" }] },
        { no: "1", l: "For the Period – To (MM/DD/YYYY)", f: [{ k: K("toMonth"), t: "int", l: "Date withholding to month", c: 1 }, { k: K("toDay"), t: "int", l: "Date withholding to day", c: 1 }, { k: K("toYear"), t: "int", l: "Date withholding to year", c: 1 }] },
      ],
    },
  ],
};

const entities = [], tasks = [], texts = [], timers = [], undos = [];
// The answers that remove typed rows, with what the host says (its own
// tests pin each message: q3_withholding_removing_typed_rows_is_reported,
// q3_withholding_item_13_is_answered_on_the_page_and_prints).
const TRIGGERS = [
  { form: "1600WP item 4", key: `frmTEST:no`, value: "true", edits: { "frmTEST:no": "true", "frmTEST:yes": "false" }, message: "Changing item 4 to No removes the 1 Part II ATC row you chose and the 1 Schedule 1.1 row you typed. Continue?" },
  { form: "1600PT item 3", key: `frmTEST:withheldNo`, value: "true", edits: { "frmTEST:withheldNo": "true" }, message: "Changing item 3 to No removes the 1 Part II ATC row you chose. Continue?" },
  { form: "1600PT item 10", key: `frmTEST:government`, value: "true", edits: { "frmTEST:government": "true" }, message: "Changing item 10 removes the 1 Part II ATC row you chose. Continue?" },
  { form: "1600WP item 7", key: `frmTEST:wpGovernment`, value: "true", edits: { "frmTEST:wpGovernment": "true" }, message: "Changing item 7 removes the 1 Part II ATC row you chose and clears the ATC of 1 Schedule 1.1 row. Continue?" },
  { form: "1600PT cleared ATC", key: `frmTEST:atc1`, value: "", edits: { "frmTEST:atc1": "" }, message: "Clearing this ATC removes its row and its tax base of ₱1,000.00. Continue?" },
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
const clicks = new Map(), dismiss = new Map();
uiExports.AppButton = (_cx, id, _label, opts = {}) => { const el = element(); el.id(id); if (opts.onClick) clicks.set(id, opts.onClick); return el; };
uiExports.Overlay = (_cx, id, card, onDismiss) => { dismiss.set(id, onDismiss); const el = element(); el.id(id); el.child(card); return el; };
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
const part0Index = (label) => schema.parts[0].items.findIndex((it) => it.l === label);

let hostValues = {
  [K("atc1")]: "WI330", [K("base1")]: "1000", [K("rate1")]: " 25.0",
  [K("atc2")]: "", [K("base2")]: "", [K("rate2")]: "",
  [K("over")]: "0.00", [K("relief")]: "0", [K("value1")]: "0.00", [K("year")]: "26",
  [K("fromMonth")]: "03", [K("fromDay")]: "16", [K("fromYear")]: "2026", [K("toMonth")]: "03", [K("toDay")]: "31", [K("toYear")]: "2026",
  [K("yes")]: "true", [K("no")]: "false", [K("value1")]: "0.00",
};
const opened = () => ({
  draft: { id: 7, rows: 1 }, status: "Draft", editable: true, values: { ...hostValues }, options: {},
  locks: { [K("rate1")]: "cannot be filled here: derived by the app", [K("rate2")]: "cannot be filled here: derived by the app", [K("year")]: "picks which return this is; reopen the form for another TIN or period" },
  lock_notes: { [K("rate1")]: "Can't be filled on this page yet.", [K("rate2")]: "Can't be filled on this page yet." },
  profile: [], profile_edit: [], sample: false, blank: [], needs: {}, waits: { [K("rate1")]: WAITS, [K("rate2")]: WAITS },
  off: {}, penalties: [], pools: {}, errors: [],
});
const app = {
  prefs: {}, branchFor: () => null, taxpayers: () => [], layout: () => ({ column: 1056 }),
  invalidate() {}, toast() {}, startNewReturn() {},
  toastUndo(message, undo) { undos.push({ message, undo }); app.toastState = { message, action: { label: "Undo", run: undo } }; },
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
        if (edits[K("over")] !== undefined) draft.over = edits[K("over")];
        const values = { ...hostValues, ...edits, [K("value1")]: draft.rows ? "1,000.00" : "0.00", ...(draft.over !== undefined ? { [K("over")]: draft.over } : {}) };
        if (!Object.keys(edits).length) Object.assign(values, draft.answer || {});
        draft.answer = { [K("yes")]: values[K("yes")], [K("no")]: values[K("no")] };
        return { draft, values, options: {}, rejected: {}, penalties: [], pools: {}, removes };
      }
      if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: { ...hostValues, ...(args[1].answer || {}), [K("value1")]: args[1].rows ? "1,000.00" : "0.00", ...(args[1].over !== undefined ? { [K("over")]: args[1].over } : {}) }, options: {}, errors: [], off: {}, penalties: [], pools: {} });
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
assert.ok(texts.includes("Cash/Bank Debit Memo"), "item 37's caption shows");
assert.ok(ids.has("fp-item-0:2"), "as an item row of its own");
assert.ok(texts.includes(NOTE), "and its note");
assert.ok(![...ids].some((id) => /^fp-frmTEST:/.test(id) && id.includes("37")), "and no control");
const part = ui.state.schema.parts[0];
assert.ok(part.items.some((it) => it.no === "37"), "the page keeps an item that has only its note");
console.log("PASS a printed item with no box shows its caption and note.");

// ---- 1600WP item 1: From and To are two dates; From has its calendar.
const from = part0Index("For the Period – From (MM/DD/YYYY)");
const to = part0Index("For the Period – To (MM/DD/YYYY)");
assert.ok(ids.has(`fp-item-0:${from}`) && ids.has(`fp-item-0:${to}`), "From and To are two items");
assert.ok(ids.has(`fp-date-0:${from}`), "From has its own calendar");
assert.ok(!ids.has(`fp-date-0:${to}`), "To, which the form computes, has none");
console.log("PASS 1600WP item 1 is two dates, From with its calendar.");

// ---- A picked date on a two-digit year list (1600PT/1600VT item 1).
{
  const months = ["01 - January", "02 - February", "03 - March"];
  const years = [{ value: "25", label: "2025" }, { value: "26", label: "2026" }];
  const date = { month: { k: K("pm"), t: "sel", o: months }, day: null, year: { k: K("py"), t: "sel", o: years } };
  const sent = () => calls.filter((c) => c[0] === "screen_form_fill").at(-1)[3];
  ui.pickDate(date, "2026-03-16", cx);
  ui.flush(cx);
  assert.equal(sent()[K("py")], "26", "the year list takes 2026 as its 26");
  assert.equal(sent()[K("pm")], "03", "the month list its 03");
  ui.pickDate(date, "2027-02-01", cx);
  ui.flush(cx);
  assert.ok(!(K("py") in sent()), "a year the list does not offer is not written");
  assert.equal(sent()[K("pm")], "02");
}
console.log("PASS a picked date writes a two-digit year list's own value and skips what it does not offer.");


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
assert.ok(texts.includes("2026"), "item 1's two-digit year 26 shows as 2026");
assert.ok(!texts.includes("26"), "never as a bare 26");
assert.equal(JSON.stringify(relief.options.map((o) => o.label)), JSON.stringify(["Choose…", "Special Rate", "International Tax Treaty", "Both"]), "and lists the reliefs by name");
console.log("PASS a dropdown's official blank 0 shows Choose..., never a raw 0.");

// ---- An answer that removes typed rows asks first.
const click = (id) => { const fn = clicks.get(id); assert.ok(fn, `button ${id}`); act(fn, {}); };
const ask = () => { clicks.clear(); dismiss.clear(); texts.length = 0; renderRemovalConfirm(cx, app); };
for (const t of TRIGGERS) {
  ui.state.draft = { ...ui.state.draft, rows: 1 };
  const kept = ui.state.draft;
  ui.choose(t.edits, cx);
  assert.equal(app.overlay, "removal", `${t.form} asks first`);
  assert.ok(ui.askingRemoval(), `${t.form}: the question is open`);
  assert.equal(ui.state.draft, kept, `${t.form}: the draft is the old one while it asks`);
  ask();
  assert.ok(texts.includes(t.message), `${t.form}: ${t.message}`);
  click("removal-cancel");
  assert.equal(app.overlay, null, `${t.form}: Cancel closes it`);
  assert.equal(ui.state.draft, kept, `${t.form}: Cancel keeps the rows`);
  ui.choose(t.edits, cx);
  ask();
  act(dismiss.get("removal-confirm"));
  assert.equal(app.overlay, null, `${t.form}: Esc is Cancel`);
  assert.equal(ui.state.draft, kept, `${t.form}: Esc keeps the rows`);
  ui.choose(t.edits, cx);
  ask();
  click("removal-continue");
  await settle();
  assert.equal(ui.state.draft.rows, 0, `${t.form}: Continue removes them`);
  assert.equal(ui.state.dirty, true, `${t.form}: the return has unsaved changes`);
  ui.state.dirty = false;
  act(undos.at(-1).undo);
  await settle();
  assert.equal(ui.state.draft.rows, 1, `${t.form}: Undo brings them back`);
  assert.equal(ui.state.dirty, true, `${t.form}: Undo leaves unsaved changes too`);
}
undos.length = 0;
const no = TRIGGERS[0].edits;
ui.choose(no, cx);
assert.equal(ui.state.values[K("no")], "false", "the old answer shows while it asks");
click("removal-continue");
await settle();
assert.equal(ui.state.values[K("no")], "true", "Continue applies the answer");
assert.equal(undos[0].message, "Removed 1 row.");
act(undos[0].undo);
await settle();
assert.equal(ui.state.values[K("no")], "false", "Undo brings the old answer back");
assert.equal(ui.state.values[K("value1")], "1,000.00", "and what the rows showed");
console.log("PASS an answer that removes typed rows asks first; Cancel, Esc and Continue click through; Undo brings them back.");

// A later edit retires the Undo: it would bring back a draft without it.
ui.choose(no, cx);
click("removal-continue");
await settle();
const stale = undos.at(-1).undo;
assert.equal(app.toastState?.action?.run, stale, "Undo is offered");
ui.state.pending[K("over")] = "7";
ui.flush(cx);
assert.equal(app.toastState, null, "a later edit takes the Undo away");
act(stale);
await settle();
assert.equal(ui.state.draft.rows, 0, "and an old Undo does nothing");
assert.equal(ui.state.values[K("over")], "7", "the later edit stays");
console.log("PASS a later edit retires the removal's Undo.");

// Undo is for the page it was offered on.
ui.state.draft = { ...ui.state.draft, rows: 1 };
ui.choose(no, cx);
click("removal-continue");
await settle();
const offered = undos.at(-1).undo;
const pageA = ui.state;
ui.state = { ...pageA, draft: { id: 8, rows: 3 } };
act(offered);
await settle();
assert.equal(ui.state.draft.id, 8, "Undo on another return's page does nothing");
assert.equal(ui.state.draft.rows, 3);
ui.state = pageA;
ui.leavingPage();
assert.equal(app.toastState, null, "leaving the page takes the Undo away");
act(offered);
await settle();
assert.equal(ui.state.draft.rows, 0, "and it does nothing after");
const main = fs.readFileSync(new URL("app/main.js", root), "utf8");
assert.match(main, /go\(view, cx\) \{\n\s*\/\/[^\n]*\n\s*this\.formUi\?\.leavingPage\?\.\(\);/, "main.js go() leaves the page");
assert.match(main, /undoLast\(cx\) \{[\s\S]{0,200}if \(this\.formUi\?\.askingRemoval\?\.\(\)\) return;/, "Cmd+Z waits while the question is open");
assert.match(main, /openPalette\(cx\) \{\n[^\n]*\n\s*if \(this\.formUi\?\.askingRemoval\?\.\(\)\) this\.formUi\.cancelRemoval\(cx\);/, "Cmd+K cancels the question first");
assert.match(main, /startNewReturn\(cx, code\) \{\n[^\n]*\n\s*if \(this\.formUi\?\.askingRemoval\?\.\(\)\) this\.formUi\.cancelRemoval\(cx\);/, "New return cancels the question first");
assert.match(main, /if \(this\.overlay === "removal"\) return this\.formUi\.cancelRemoval\(cx\);/, "Esc cancels it");
console.log("PASS a removal's Undo acts only on its own page; the app's shortcuts respect the question.");

// Typed with the answer, in one batch: Cancel keeps what was typed.
ui.state.draft = { ...ui.state.draft, rows: 1 };
ui.state.pending[K("over")] = "500";
ui.choose(no, cx);
assert.equal(app.overlay, "removal");
click("removal-cancel");
const sent = calls.filter((c) => c[0] === "screen_form_fill").at(-1);
assert.equal(sent[3][K("over")], "500", "Cancel sends what was typed on its own");
assert.ok(!(K("no") in sent[3]), "without the answer");
assert.equal(ui.state.values[K("over")], "500", "and it is in the draft");
assert.equal(ui.state.draft.rows, 1, "the rows stay");
console.log("PASS Cancel keeps what was typed in the same batch.");

// Save never applies a removal without the question.
const saves = () => calls.filter((c) => c[0] === "screen_form_save").length;
const before = saves();
ui.state.pending = { ...no };
ui.state.chosenFrom = { [K("no")]: "false", [K("yes")]: "true" };
ui.save(cx);
assert.equal(app.overlay, "removal", "Save asks first");
assert.equal(saves(), before, "and saves nothing yet");
assert.equal(ui.state.draft.rows, 1, "the rows are still there");
click("removal-cancel");
console.log("PASS Save asks before it removes typed rows.");
