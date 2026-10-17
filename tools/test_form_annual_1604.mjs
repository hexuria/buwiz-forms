// The form page as the annual information returns (PR 6: 1604C, 1604F,
// 1604E) use it, driven through the real app/shell/form_page.js with a
// mocked host. Which boxes these are, and what the host says, is the
// host's to pin (crates/bir-shell screen_form_module/tests/annual_1604.rs:
// pr6_part_ii_schedules_follow_the_printed_rows_and_columns,
// pr6_item_11_no_asks_before_clearing_what_was_typed); this proves what the
// page does with it:
//  - a Part II schedule draws its printed column heads, each row's printed
//    month (1604C) or quarter (1604F) caption in place of a row number, and
//    its TOTAL row under it; a table without printed row captions is still
//    numbered;
//  - 1604C item 11 No (clears items 11A to 13) and 1604F item 11 No (clears
//    item 11A) ask first: Cancel keeps the answer and what was typed,
//    Continue applies it and offers Undo naming what was cleared, and Undo
//    brings both back.
// The page code is PR 2's (fix/findings-q3-withholding); PR 6 makes printed
// row captions stand in for the row numbers.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);

const K = (tail) => `frm1604c:${tail}`;
const MONTHS = ["January", "February", "March"];
const COLS = ["Date of Remittance (MM/DD/YYYY)", "Drawee Bank/Bank Code/Agency", "TRA/eROR/eAR Number", "Taxes Withheld", "Adjustment", "Penalties", "Total Amount Remitted"];
const STEMS = ["txtSched1Date", "txtSched1BankVal", "txtTra", "txtSched1TaxWheld", "txtSched1Adj", "txtSched1Pen", "txtSched1TotalAmt"];
const TYPES = ["date", "text", "text", "money", "money", "money", "money"];
const MESSAGE = "Changing item 11 to No clears item 11A (date of refund 03/15/2026), item 12 (₱1,234.50) and item 13 (April). Continue?";
const F = (tail) => `frm1604f:${tail}`;
const QUARTERS = ["1st Quarter", "2nd Quarter", "3rd Quarter", "4th Quarter"];
const F_MESSAGE = "Changing item 11 to No clears item 11A (PEZA). Continue?";
const schema = {
  c: "1604C", t: "Test return", rev: "2018", freq: "Annual", pages: 1, n: 4, q: 1,
  parts: [
    {
      n: "Part I · Background Information",
      items: [
        { no: "11", l: "In case of overwithholding/overremittance after the year-end adjustments on compensation, have you released the refund/s to your employee/s?", f: [{ k: K("releasedOfFunds_1"), t: "radio", l: "Yes" }, { k: K("releasedOfFunds_2"), t: "radio", l: "No" }] },
        { no: "12", l: "Total amount of overremittance on Tax withheld under compensation", f: [{ k: K("txt12"), t: "money", l: "Total amount of overremittance" }] },
      ],
    },
    {
      n: "Part II · Summary of Remittances per BIR Form No. 1601-C",
      items: [
        {
          no: "", l: "January to December (Part II and its Continuation)", rows: 3,
          cols: COLS.map((l, c) => ({ l, t: TYPES[c], ...(c === 6 ? { c: 1 } : {}) })),
          keys: MONTHS.map((_m, r) => STEMS.map((s) => K(`${s}${r + 1}`))),
          row_labels: { caption: "Month", rows: MONTHS },
        },
        { no: "", l: "TOTAL", f: [{ k: K("txtSched1TaxWheldTotal"), t: "money", l: "Taxes Withheld", c: 1 }, { k: K("txtSched1Total"), t: "money", l: "Total Amount Remitted", c: 1 }] },
      ],
    },
    {
      n: "1604F",
      items: [
        { no: "11", l: "Are there payees availing of tax relief under Special Law or International Tax Treaty?", f: [{ k: F("taxRelief_1"), t: "radio", l: "Yes" }, { k: F("taxRelief_2"), t: "radio", l: "No" }] },
        { no: "11A", l: "If yes, specify", f: [{ k: F("availedTaxRelief"), t: "text", l: "If yes, specify" }] },
        {
          no: "", l: "Schedule 1 – Remittance per BIR Form No. 1601-FQ on Final Taxes Withheld", rows: 4,
          cols: [{ l: "Date of Remittance (MM/DD/YYYY)", t: "date" }, { l: "Taxes Withheld", t: "money" }],
          keys: QUARTERS.map((_q, r) => [F(`txtSched1Date${r + 1}`), F(`txtSched1TaxWheld${r + 1}`)]),
          row_labels: { caption: "Quarter", rows: QUARTERS },
        },
        { no: "", l: "A table with no printed row captions", rows: 2, cols: [{ l: "Plain", t: "text" }], keys: [["plain1"], ["plain2"]] },
      ],
    },
  ],
};

const entities = [], tasks = [], texts = [], timers = [], undos = [];
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

let hostValues = {
  [K("releasedOfFunds_1")]: "true", [K("releasedOfFunds_2")]: "false", [K("txt12")]: "1,234.50",
  [K("txtSched1TaxWheldTotal")]: "1,000.00", [K("txtSched1Total")]: "1,075.00",
  [K("txtSched1Date1")]: "01/10/2026", [K("txtSched1TaxWheld1")]: "1,000.00", [K("txtSched1TotalAmt1")]: "1,075.00",
};
// What the host shows for a draft's answers.
const shown = (d) => ({
  [K("txt12")]: d.overremittance ? "1,234.50" : "0.00",
  [K("releasedOfFunds_1")]: String(d.refunds_released), [K("releasedOfFunds_2")]: String(!d.refunds_released),
  [F("taxRelief_1")]: String(d.tax_relief), [F("taxRelief_2")]: String(!d.tax_relief), [F("availedTaxRelief")]: d.relief,
});
hostValues = { ...hostValues, ...shown({ refunds_released: true, overremittance: 1234.5, tax_relief: true, relief: "PEZA" }) };
const opened = () => ({
  draft: { id: 7, refunds_released: true, overremittance: 1234.5, tax_relief: true, relief: "PEZA" }, status: "Draft", editable: true, values: { ...hostValues }, options: {},
  locks: { [K("txtSched1TaxWheldTotal")]: "computed by BIR's own formulas", [K("txtSched1Total")]: "computed by BIR's own formulas" },
  lock_notes: {}, profile: [], profile_edit: [], sample: false, blank: [], needs: {},
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
        // The host: item 11 No clears item 12 and says so (its own test pins
        // the message).
        const draft = { ...args[1] }, edits = args[2];
        let removes = null;
        if (edits[F("taxRelief_2")] === "true" && draft.tax_relief) {
          if (draft.relief) removes = { message: F_MESSAGE, done: "Cleared item 11A", rows: 0 };
          draft.tax_relief = false;
          draft.relief = "";
        }
        if (edits[K("releasedOfFunds_2")] === "true" && draft.refunds_released) {
          if (draft.overremittance) removes = { message: MESSAGE, done: "Cleared items 11A, 12 and 13", rows: 0 };
          draft.refunds_released = false;
          draft.overremittance = 0;
        }
        if (edits[K("releasedOfFunds_1")] === "true") draft.refunds_released = true;
        const values = { ...hostValues, ...edits, ...shown(draft) };
        return { draft, values, options: {}, rejected: {}, penalties: [], pools: {}, removes };
      }
      if (name === "screen_form_validate") {
        const d = args[1];
        return Promise.resolve({ draft: d, values: { ...hostValues, ...shown(d) }, options: {}, errors: [], off: {}, penalties: [], pools: {} });
      }
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
ui.open({ code: "1604C", tin: "12345678800000", year: 2026, period: 0 }, cx);
await settle();
for (let i = 0; i < schema.parts.length; i++) ui.ensureInputs(i, cx);
ui.state.openParts = new Set(schema.parts.map((_p, i) => i));
frame();

// ---- Part II: the printed heads, months and TOTAL row.
for (const head of COLS) assert.ok(texts.includes(head), `column head ${head}`);
assert.ok(texts.includes("Month"), "the row captions' head");
for (const month of MONTHS) assert.ok(texts.includes(month), `row caption ${month}`);
assert.ok(ids.has(`fp-table-${K("txtSched1Date1")}`), "one table");
assert.ok(texts.includes("TOTAL"), "the TOTAL row's caption");
assert.ok(texts.includes("1,075.00"), "a computed total shows its value");
assert.ok(!texts.some((t) => /^Schedule \d (month|quarter) \d/.test(t)), "no key-text row numbers");
for (const quarter of QUARTERS) assert.ok(texts.includes(quarter), `1604F row caption ${quarter}`);
assert.ok(texts.includes("Quarter"), "1604F's row captions' head");
// Printed captions stand in for the numbers: drawn again without its
// captions, each captioned table gains a "#" head and its row numbers (the
// 1604C table 1-3, the 1604F quarters 1-4); the uncaptioned table has them
// either way.
{
  const count = (t) => texts.filter((x) => x === t).length;
  const asIs = Object.fromEntries(["#", "1", "2", "3", "4"].map((t) => [t, count(t)]));
  const captioned = ui.state.schema.parts.flatMap((p) => p.items).filter((it) => it.row_labels);
  assert.equal(captioned.length, 2);
  const saved = captioned.map((it) => it.row_labels);
  for (const it of captioned) delete it.row_labels;
  frame();
  assert.ok(!MONTHS.some((m) => texts.includes(m)), "without captions no month shows");
  for (const [t, more] of [["#", 2], ["1", 2], ["2", 2], ["3", 2], ["4", 1]]) assert.equal(count(t) - asIs[t], more, `captions replace ${t}`);
  captioned.forEach((it, i) => (it.row_labels = saved[i]));
  frame();
}
console.log("PASS a Part II schedule shows its printed heads, row captions and TOTAL row.");

// ---- Item 11 No asks first.
const click = (id) => { const fn = clicks.get(id); assert.ok(fn, `button ${id}`); act(fn, {}); };
const ask = () => { clicks.clear(); dismiss.clear(); texts.length = 0; renderRemovalConfirm(cx, app); };
const no = { [K("releasedOfFunds_1")]: "false", [K("releasedOfFunds_2")]: "true" };
const kept = ui.state.draft;
ui.choose(no, cx);
assert.equal(app.overlay, "removal", "item 11 No asks first");
assert.equal(ui.state.draft, kept, "the draft is the old one while it asks");
ask();
assert.ok(texts.includes(MESSAGE), "with what it clears");
click("removal-cancel");
assert.equal(app.overlay, null, "Cancel closes it");
assert.equal(ui.state.draft, kept, "Cancel keeps item 12");
assert.equal(ui.state.values[K("releasedOfFunds_1")], "true", "and the Yes");
ui.choose(no, cx);
ask();
click("removal-continue");
await settle();
assert.equal(ui.state.draft.overremittance, 0, "Continue clears item 12");
assert.equal(ui.state.values[K("releasedOfFunds_2")], "true", "and answers No");
assert.equal(undos.at(-1).message, "Cleared items 11A, 12 and 13.");
act(undos.at(-1).undo);
await settle();
assert.equal(ui.state.draft.overremittance, 1234.5, "Undo brings item 12 back");
assert.equal(ui.state.values[K("releasedOfFunds_1")], "true", "and the Yes");
console.log("PASS 1604C item 11 No asks first; Cancel keeps, Continue clears, Undo brings back.");

// ---- 1604F item 11 No asks first, too.
{
  const noF = { [F("taxRelief_1")]: "false", [F("taxRelief_2")]: "true" };
  const keptF = ui.state.draft;
  ui.choose(noF, cx);
  assert.equal(app.overlay, "removal", "1604F item 11 No asks first");
  ask();
  assert.ok(texts.includes(F_MESSAGE), "with what it clears");
  click("removal-cancel");
  assert.equal(ui.state.draft, keptF, "Cancel keeps item 11A");
  assert.equal(ui.state.values[F("availedTaxRelief")], "PEZA");
  ui.choose(noF, cx);
  ask();
  click("removal-continue");
  await settle();
  assert.equal(ui.state.draft.relief, "", "Continue clears item 11A");
  assert.equal(ui.state.values[F("taxRelief_2")], "true", "and answers No");
  assert.equal(undos.at(-1).message, "Cleared item 11A.");
  act(undos.at(-1).undo);
  await settle();
  assert.equal(ui.state.draft.relief, "PEZA", "Undo brings item 11A back");
  assert.equal(ui.state.values[F("taxRelief_1")], "true", "and the Yes");
}
console.log("PASS 1604F item 11 No asks first; Cancel keeps, Continue clears, Undo brings back.");
