// PR 14 (decisions P4-16 and P4-18, review F1, F2, F9): the page unticks
// the ticked box of a one-answer group of three or more by the group's size
// the host serves on each box (`rg`, screen_form_module.rs
// mark_radio_groups), not by the radios drawn in one item; a pair keeps its
// re-pick; a radio the host put in no group is never unticked. The items
// are the host's own served schema, exported by the bir-shell test
// radio_untick_rule::pr14_radio_page_fixture_is_the_hosts into
// tools/fixtures/form_radio_untick_page.json: 2200S's manner (items 15 and
// 16 radios, item 17 a tick box), 2200C's manner (one official group of
// three), 1701's tax-rate pair, 1701MS's 11 and 12, whose "false" is a
// different answer, and 2552's item 11 — the kept group (P5-11). Driven
// through the real app/shell/form_page.js with a
// mocked host (the harness of tools/test_form_as_needed.mjs). What the host
// then writes is its own (crates/bir-shell screen_form_module
// tests::radio_untick_rule).
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);

const served = JSON.parse(fs.readFileSync(new URL("tools/fixtures/form_radio_untick_page.json", root), "utf8"));
let code = "2200S";
const schemaOf = (form) => ({ c: form, t: form, rev: "2018", freq: "As needed", pages: 1, n: 1, parts: [{ n: "Part", items: served[form].items }] });
const field = (form, key) => served[form].items.flatMap((it) => it.f || []).find((f) => f.k === key);

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
const kit = mock("gpui-kit", { div: element, svg: element, View, list: element });
const base = mock("gpui-base", { InputState, CalendarState, Popup: { new: element }, Scrollbar: { vertical: element }, VirtualListScrollHandle: { new: () => ({ scroll_to_item() {}, scroll_to_bottom() {}, release() { return true } }) } });
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
let hostOpen = {};
const opened = () => ({
  draft: { id: 7 }, status: "Draft", editable: true, values: { ...hostValues }, options: {}, locks: {},
  waits: {}, profile: [], profile_edit: [], sample: false, blank: [], needs: {}, off: {}, penalties: [], pools: {}, errors: [],
  ...hostOpen,
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
      if (name === "screen_form_schema") return schemaOf(code);
      if (name === "screen_form_open") return Promise.resolve(opened());
      if (name === "screen_form_fill") {
        const values = { ...(args[1].values || hostValues), ...args[2] };
        hostValues = values;
        return { draft: { id: 7, values }, values: { ...values }, options: {}, rejected: {}, penalties: [], pools: {}, removes: null };
      }
      if (name === "screen_form_sample_period") return Promise.resolve({ complete: false, existing: null });
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
async function openWith(form, values, extra = {}) {
  code = form;
  hostValues = values;
  hostOpen = extra;
  ui.open({ code: form, tin: "12345678800000", year: 2026, period: 0 }, cx);
  await settle();
  ui.state.openParts = new Set([0]);
  frame();
}
const reclick = (key) => {
  assert.ok(clicks.has(`fp-${key}`), `${key} is drawn as a radio`);
  const before = fills().length;
  act(clicks.get(`fp-${key}`).fn, {});
  return fills().length === before ? null : JSON.parse(lastEdits());
};

// ---- The host's sizes: every manner box carries 3, 1701's tax rate 2,
// 1701MS's 11c and 12b none.
for (const key of ["frm2200S:chkPymntManner_1", "frm2200S:chkPymntManner_2", "frm2200S:chkPymntManner_3"]) assert.equal(field("2200S", key).rg, 3, key);
for (const key of ["frm2200C:rdoPg1Pt2MOPPaymentActual", "frm2200C:rdoPg1Pt2MOPPrepayment", "frm2200C:rdoPg1Pt2MOPOther"]) assert.equal(field("2200C", key).rg, 3, key);
for (const key of ["frm1701:rdoPg1I21TaxRateG", "frm1701:rdoPg1I21TaxRateP"]) assert.equal(field("1701", key).rg, 2, key);
for (const key of ["frm1701MS:txtTaxpayerNo11c", "frm1701MS:txtTaxpayerNo12b"]) assert.equal(field("1701MS", key).rg, undefined, key);

// ---- 2200S and 2200C: re-clicking each ticked manner radio unticks it.
for (const [form, keys] of [
  ["2200S", ["frm2200S:chkPymntManner_1", "frm2200S:chkPymntManner_2"]],
  ["2200C", ["frm2200C:rdoPg1Pt2MOPPaymentActual", "frm2200C:rdoPg1Pt2MOPPrepayment", "frm2200C:rdoPg1Pt2MOPOther"]],
]) {
  for (const key of keys) {
    await openWith(form, { [key]: "true" });
    assert.deepEqual(reclick(key), { [key]: "false" }, `${form} ${key}: re-clicked, the manner is left unanswered`);
  }
}
await openWith("2200S", { "frm2200S:chkPymntManner_1": "true" });
assert.deepEqual(reclick("frm2200S:chkPymntManner_2"), { "frm2200S:chkPymntManner_2": "true" }, "picking another manner ticks it");
console.log("PASS 2200S and 2200C manner of payment: re-clicking a ticked manner unticks it.");

// ---- 2200S item 17's tick box unticks as a tick box.
await openWith("2200S", { "frm2200S:chkPymntManner_2": "true", "frm2200S:chkPymntManner_3": "true" });
const other = checkboxes.get("fp-frm2200S:chkPymntManner_3");
assert.ok(other && other.checked, "item 17 shows ticked");
act(other.onChange, false);
assert.equal(lastEdits(), JSON.stringify({ "frm2200S:chkPymntManner_3": "false" }), "item 17 unticked sends its untick");
console.log("PASS 2200S manner of payment: item 17's tick box unticks.");

// ---- 1701's tax rate is a pair: re-picking the ticked one sends it
// again with the other "false", never an untick (the ATC stays: the host's
// own test, pr14_tax_rate_pairs_keep_the_atc). Both members, and picking
// the unticked one unticks only the pair's other member.
await openWith("1701", { "frm1701:rdoPg1I21TaxRateG": "true" });
assert.deepEqual(reclick("frm1701:rdoPg1I21TaxRateG"), { "frm1701:rdoPg1I21TaxRateG": "true", "frm1701:rdoPg1I21TaxRateP": "false" }, "a pair's answer given again runs again");
await openWith("1701", { "frm1701:rdoPg1I21TaxRateP": "true" });
assert.deepEqual(reclick("frm1701:rdoPg1I21TaxRateP"), { "frm1701:rdoPg1I21TaxRateP": "true", "frm1701:rdoPg1I21TaxRateG": "false" }, "the P member given again runs again");
await openWith("1701", { "frm1701:rdoPg1I21TaxRateG": "true" });
assert.deepEqual(reclick("frm1701:rdoPg1I21TaxRateP"), { "frm1701:rdoPg1I21TaxRateP": "true", "frm1701:rdoPg1I21TaxRateG": "false" }, "picking P unticks only G");
console.log("PASS 1701 tax rate: a two-member pair keeps its re-pick.");

// ---- 1701's spouse tax-rate pair (item 12) behaves the same.
await openWith("1701", { "frm1701:rdoPg2I12TaxRateG": "true" });
assert.deepEqual(reclick("frm1701:rdoPg2I12TaxRateG"), { "frm1701:rdoPg2I12TaxRateG": "true", "frm1701:rdoPg2I12TaxRateP": "false" }, "the spouse's answer given again runs again");
await openWith("1701", { "frm1701:rdoPg2I12TaxRateP": "true" });
assert.deepEqual(reclick("frm1701:rdoPg2I12TaxRateP"), { "frm1701:rdoPg2I12TaxRateP": "true", "frm1701:rdoPg2I12TaxRateG": "false" }, "the spouse's P given again runs again");
console.log("PASS 1701 spouse tax rate: the pair keeps its re-pick.");

// ---- 1701A's tax-rate pairs, taxpayer and spouse.
for (const [key, other] of [
  ["frm1701A:optTaxRate_1", "frm1701A:optTaxRate_2"],
  ["frm1701A:optTaxRate_2", "frm1701A:optTaxRate_1"],
  ["frm1701A:optSpouseTaxRate_1", "frm1701A:optSpouseTaxRate_2"],
  ["frm1701A:optSpouseTaxRate_2", "frm1701A:optSpouseTaxRate_1"],
]) {
  await openWith("1701A", { [key]: "true" });
  assert.deepEqual(reclick(key), { [key]: "true", [other]: "false" }, `${key}: the pair's answer given again runs again`);
}
console.log("PASS 1701A tax rates: the taxpayer and spouse pairs keep their re-picks.");

// ---- 1701MS item 11 draws three affiliations in one item — the
// taxpayer's a/b pair (one group), the ungrouped c, and the spouse's
// a1..d1 (another group). A pick unticks only its own group's members
// (review F1): a sends "false" for b only, never c or a1..d1; a1 sends
// "false" for b1..d1 only.
await openWith("1701MS", { "frm1701MS:txtTaxpayerNo11b": "true", "frm1701MS:txtTaxpayerNo11c": "true", "frm1701MS:txtTaxpayerNo11a1": "true" });
assert.deepEqual(reclick("frm1701MS:txtTaxpayerNo11a"), { "frm1701MS:txtTaxpayerNo11a": "true", "frm1701MS:txtTaxpayerNo11b": "false" }, "a unticks only b, its own group");
await openWith("1701MS", { "frm1701MS:txtTaxpayerNo11a": "true", "frm1701MS:txtTaxpayerNo11c": "true", "frm1701MS:txtTaxpayerNo11b1": "true" });
assert.deepEqual(reclick("frm1701MS:txtTaxpayerNo11a1"), { "frm1701MS:txtTaxpayerNo11a1": "true", "frm1701MS:txtTaxpayerNo11b1": "false", "frm1701MS:txtTaxpayerNo11c1": "false", "frm1701MS:txtTaxpayerNo11d1": "false" }, "a1 unticks only b1..d1, its own group");
// Item 12: the ungrouped b is not a sibling of the group of seven.
await openWith("1701MS", { "frm1701MS:txtTaxpayerNo12b": "true" });
assert.deepEqual(reclick("frm1701MS:txtTaxpayerNo12c"), { "frm1701MS:txtTaxpayerNo12c": "true", "frm1701MS:txtTaxpayerNo12d": "false", "frm1701MS:txtTaxpayerNo12e": "false", "frm1701MS:txtTaxpayerNo12b1": "false", "frm1701MS:txtTaxpayerNo12c1": "false", "frm1701MS:txtTaxpayerNo12d1": "false", "frm1701MS:txtTaxpayerNo12e1": "false" }, "c unticks its group, never the ungrouped b");
console.log("PASS 1701MS 11 and 12: a pick unticks only its own group's members.");

// ---- 1701MS's 11c and 12b are in no group: re-clicking them sends nothing.
for (const key of ["frm1701MS:txtTaxpayerNo11c", "frm1701MS:txtTaxpayerNo12b"]) {
  await openWith("1701MS", { [key]: "true" });
  assert.equal(reclick(key), null, `${key}: re-clicked, nothing is sent and the answer stays`);
}
console.log("PASS 1701MS 11c and 12b: a ticked radio of no group stays as it is.");

// ---- 2552 item 11, the kept group (P5-11): re-clicking the ticked
// member emits its "false" — the same-group member's untick — and nothing
// else; the host then keeps the answer (the shell suite's 2552 case,
// pr14_an_untick_never_fails_the_rest_of_the_batch). A pick unticks only
// its own group's members (review F1). With the group unticked a click is
// a pick again — the page cannot emit an untick for a member that is not
// ticked; that no-op is the host's ("unticking a member that is not
// ticked does nothing").
await openWith("2552", { "frm2552:rdoPg1I11TransactionLSE": "true" });
assert.deepEqual(reclick("frm2552:rdoPg1I11TransactionLSE"), { "frm2552:rdoPg1I11TransactionLSE": "false" }, "the ticked member's re-click emits only its untick");
await openWith("2552", { "frm2552:rdoPg1I11TransactionLSE": "true" });
assert.deepEqual(reclick("frm2552:rdoPg1I11TransactionPrimary"), { "frm2552:rdoPg1I11TransactionPrimary": "true", "frm2552:rdoPg1I11TransactionLSE": "false", "frm2552:rdoPg1I11TransactionSecondary": "false" }, "a pick unticks only its own group's members");
await openWith("2552", {});
assert.deepEqual(reclick("frm2552:rdoPg1I11TransactionLSE"), { "frm2552:rdoPg1I11TransactionLSE": "true", "frm2552:rdoPg1I11TransactionPrimary": "false", "frm2552:rdoPg1I11TransactionSecondary": "false" }, "the group unticked: a click picks, never unticks");
console.log("PASS 2552 item 11: the kept group's untick and grp-scoped pick emit as the group's.");

// ---- 0605's quarters are period parts of a sample whose period is set:
// re-clicking the ticked one asks nothing and sends nothing (review F2).
{
  const parts = ["itemQuarter_1", "itemQuarter_2", "itemQuarter_3", "itemQuarter_4"].map((key) => ({ key, part: "quarter" }));
  await openWith("0605", { itemQuarter_1: "true" }, { sample: true, period_set: true, period_parts: parts });
  const before = calls.length;
  assert.equal(reclick("itemQuarter_1"), null, "re-clicking the quarter sends no fill");
  assert.equal(calls.length, before, "re-clicking the quarter asks the host nothing");
  assert.equal(ui.state.periodComplete, true, "the sample's set period stays set");
  console.log("PASS 0605 quarters: re-clicking the ticked quarter is a no-op.");
}
