// The "fast lane" offer of a stored return (variant D), driven through the
// real app/shell/form_page.js with a mocked host: when a library sample's
// complete period already has a stored return, the app's modal offers what
// its status allows (draft: Load; queued: View; submitted or filed: View
// read-only, and Amend where the host offers it) and Keep this form. Keep
// blanks every period part and keeps the other typed values; Esc or a click
// outside is Keep; each action can be undone once and Undo restores the
// sample exactly; "Don't ask again for drafts" loads a later draft straight
// away but still asks for queued and filed returns.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);
const schema = JSON.parse(fs.readFileSync(new URL("app/forms/forms-schema.json", root))).find((f) => f.c === "2551Q");
const K = (tail) => `frm2551Qv2018:${tail}`;

const entities = [], tasks = [], texts = [], buttons = new Map(), dismissers = new Map(), checkboxes = new Map(), calls = [];
let owner = null;
class View {}
function context(entity = null) {
  return {
    notify() {},
    new(Class, props) {
      const e = { dirty: true, released: false, set_props(p) { e.props = p; e.view.update?.(p); e.dirty = true; }, release() { e.released = true; return true; } };
      e.view = new Class(props); e.props = props; entities.push(e);
      const old = owner; owner = e; e.view.init?.(props, context(e)); owner = old;
      return e;
    },
    spawn(fn) { const p = Promise.resolve(fn(context(entity))); tasks.push(p); return { cancel() {} }; },
    timer: { after() { return { cancel() {} }; }, every() { return { cancel() {} }; } },
    focus_handle() { return { focus() {}, is_focused: () => false, release() { return true; } }; },
    stop_propagation() {},
    theme() { return { appearance: "light", is_dark: false, colors: {} }; },
  };
}
function element() {
  const el = new Proxy({}, { get(o, k) {
    if (k === "then") return undefined;
    return (...args) => {
      if (k === "when" && args[0]) args[1](el);
      if (k === "child") mount(args[0]);
      if (k === "children") for (const c of args[0]) mount(c);
      return el;
    };
  } });
  return el;
}
function mount(e) { if (e?.view && e.dirty && !e.released) { e.dirty = false; const old = owner; owner = e; e.view.render(context(e)); owner = old; } }
const InputState = { new({ value = "" } = {}) { let text = value; return { value: () => text, set_value: (v) => { text = v; }, on() {}, release() { return true; } }; } };
const contextVM = vm.createContext({ console, Map, Set, JSON, Date, window: { viewport_size: () => ({ width: 1400, height: 900 }) } });
function mock(name, exports) { return new vm.SyntheticModule(Object.keys(exports), function () { for (const [k, v] of Object.entries(exports)) this.setExport(k, v); }, { context: contextVM, identifier: name }); }
const uiExports = { tokens: () => new Proxy({}, { get: () => "#000" }), SPACE: [0, 4, 8, 12, 16, 20, 24], RADIUS: {}, SIZE: { validationLine: 18 }, BREAK: { formNav: 760 } };
for (const n of ["Text", "Line", "Code", "Icon", "AppButton", "IconButton", "Avatar", "Chip", "Switch", "Checkbox", "TextField", "Dropdown", "PageHeader", "MetaLine", "Overlay", "CheckboxLabel"]) uiExports[n] = element;
uiExports.Text = (_cx, _v, text) => { texts.push(String(text)); return element(); };
uiExports.AppButton = (_cx, id, label, opts = {}) => { buttons.set(id, { label, variant: opts.variant, onClick: opts.onClick }); return element(); };
uiExports.Overlay = (_cx, id, card, onDismiss) => { dismissers.set(id, onDismiss); return element().child(card); };
uiExports.CheckboxLabel = (_cx, id, checked, onChange, label) => { checkboxes.set(id, { checked, onChange, label }); return element(); };
const mod = new vm.SourceTextModule(fs.readFileSync(new URL("app/shell/form_page.js", root), "utf8"), { context: contextVM });
await mod.link((s) => ({
  "gpui-kit": mock("gpui-kit", { div: element, svg: element, View }),
  "gpui-base": mock("gpui-base", { InputState, CalendarState: {}, Popup: { new: element } }),
  "gpui-component": mock("gpui-component", { Spinner: { new: element } }),
  "./ui.js": mock("./ui.js", uiExports),
  "./store.js": mock("./store.js", { formatTin: (x) => x, errorText: (e) => String(e && e.message ? e.message : e) }),
  "./periods.js": mock("./periods.js", { periodLabel: () => "Q1 2024" }),
})[s]);
await mod.evaluate();

const quarterKeys = [1, 2, 3, 4].map((n) => K(`qtr_${n}`));
const periodParts = [{ key: K("txtYear"), part: "year", quarter: null }, ...quarterKeys.map((key, i) => ({ key, part: "quarter", quarter: i + 1 }))];
// Q1 a draft, Q2 queued, Q3 submitted (amendable), Q4 confirmed (not amendable here).
const stored = {
  1: { status: "Draft", actions: ["load", "keep"], updated_at: "2026-10-01T08:00:00Z" },
  2: { status: "Queued", actions: ["view", "keep"] },
  3: { status: "Submitted", actions: ["view", "amend", "keep"] },
  4: { status: "Confirmed", actions: ["view", "keep"] },
};
const sampleOpened = () => ({
  draft: { screen_form_sample: { blank: [] }, sample: true }, status: "Draft", editable: true,
  values: { [K("txtYear")]: "", ...Object.fromEntries(quarterKeys.map((k) => [k, ""])), [K("amendedRtn_2")]: "" },
  options: {}, locks: {}, profile: [], profile_edit: [], sample: true,
  blank: [K("txtYear"), ...quarterKeys], needs: {}, period_parts: periodParts, period_set: false, saved_periods: [], errors: [],
});
const returnOpened = (draft) => ({ draft, status: "Draft", editable: true, values: {}, options: {}, locks: {}, profile: [], profile_edit: [], sample: false, blank: [], needs: {}, period_parts: [], errors: [] });
const app = {
  prefs: {}, branchFor: () => ({ tin: "12345678800000" }), taxpayers: () => [], layout: () => ({ column: 1056 }),
  overlay: null, existingOffer: null, session: { fastDrafts: false }, undo: null, undos: [],
  invalidate() {}, toast() {}, go() {}, startNewReturn() {}, templates: () => [],
  toastUndo(message, undo) { this.undos.push(message); this.undo = undo; },
  openForm(opts, cx, keep) { calls.push(["openForm", opts]); this.formUi.open(opts, cx, keep); },
  store: {
    templates: () => [],
    write() { throw Error("no writes"); },
    call(name, ...args) {
      calls.push([name, ...args]);
      if (name === "screen_form_schema") return schema;
      if (name === "screen_form_sample") return Promise.resolve(sampleOpened());
      if (name === "screen_form_open") return Promise.resolve(returnOpened({ id: 9, opened: args.slice(2) }));
      if (name === "screen_form_amend") return Promise.resolve({ ...returnOpened({ amended: args.slice(2) }), amends: "Submitted" });
      if (name === "screen_form_fill") return { draft: args[1], values: { ...args[2] }, options: {}, rejected: {} };
      if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: {}, options: {}, errors: [] });
      if (name === "screen_form_sample_period") {
        const parts = args[2];
        const quarter = quarterKeys.findIndex((k) => parts[k] === "true") + 1;
        if (parts[K("txtYear")] !== "2024" || !quarter) return Promise.resolve({ complete: false });
        const s = stored[quarter];
        return Promise.resolve({
          complete: true,
          opens: { year: 2024, period: quarter },
          existing: { form_code: "2551Q", year: 2024, period: quarter, label: `Q${quarter} 2024`, total_payable: 0, submitted_at: null, confirmed_at: null, updated_at: "", ...s },
        });
      }
      throw Error(name);
    },
  },
};
app.formUi = new mod.namespace.FormPageActions(app);
const ui = app.formUi;
const cx = context();
const settle = async () => { while (tasks.length) await Promise.all(tasks.splice(0)); };
const json = (v) => JSON.stringify(v);
function offer() {
  texts.length = 0; buttons.clear(); dismissers.clear(); checkboxes.clear();
  mod.namespace.renderExistingOffer(cx, app);
  return { title: texts[0], buttons: [...buttons.keys()], texts: [...texts] };
}
async function tick(quarter) {
  ui.choose(Object.fromEntries(quarterKeys.map((k, i) => [k, i + 1 === quarter ? "true" : "false"])), cx);
  await settle();
}
const press = async (id) => { buttons.get(id).onClick({}, cx); await settle(); };
const undo = async () => { const run = app.undo; app.undo = null; run(cx); await settle(); };

ui.open({ code: "2551Q", tin: "12345678800000", year: 2026, period: 0, sample: true }, cx);
await settle();
const sample = ui.state;
ui.edit(K("txtYear"), "2024", cx);
ui.choose({ [K("amendedRtn_2")]: "true" }, cx);
await settle();
const typed = json(sample.typedEdits);

// A draft: the modal, and nothing else changes.
const draftBefore = sample.draft;
await tick(1);
assert.equal(app.overlay, "existing", "the modal opens");
assert.equal(ui.state, sample);
assert.equal(sample.draft, draftBefore, "an existing answer changes nothing by itself");
let o = offer();
assert.equal(o.title, "You already have a Q1 2024 draft");
assert.ok(o.texts.includes("edited Oct 1, 2026."));
assert.equal(json(o.buttons), json(["existing-load", "existing-keep"]));
assert.equal(buttons.get("existing-load").variant, "primary");
assert.ok(o.texts.includes("Continue where you left off.") && o.texts.includes("Stay in the sample; the period clears."));
assert.ok(checkboxes.has("existing-fast-lane"), "drafts offer the fast lane");

// Keep: every period part blank, the other typed values kept; Undo brings the period back.
const periodBefore = json(sample.periodValues), valuesBefore = json(sample.values);
await press("existing-keep");
assert.equal(app.overlay, null);
assert.equal(json(sample.periodValues), "{}", "the period clears");
assert.equal(sample.values[K("txtYear")], "");
assert.equal(sample.values[quarterKeys[0]], "");
assert.equal(sample.values[K("amendedRtn_2")], "true", "the other typed values stay");
assert.equal(json(sample.typedEdits), typed);
assert.match(app.undos.at(-1), /Kept this form/);
await undo();
assert.equal(json(sample.periodValues), periodBefore, "Undo brings the period back");
assert.equal(json(sample.values), valuesBefore, "exactly as it was");

// Esc or a click outside is Keep.
sample.periodAsked = null;
await tick(1);
offer();
dismissers.get("existing-offer")(cx);
await settle();
assert.equal(app.overlay, null);
assert.equal(json(sample.periodValues), "{}", "Esc kept the form and cleared the period");
await undo();

// Load: the saved draft opens; Undo restores the sample exactly.
sample.periodAsked = null;
await tick(1);
offer();
await press("existing-load");
assert.notEqual(ui.state, sample, "the saved draft is open");
assert.equal(json(calls.filter((c) => c[0] === "openForm").at(-1)[1]), json({ tin: "12345678800000", code: "2551Q", year: 2024, period: 1 }));
assert.match(app.undos.at(-1), /Loaded your saved Q1 2024 draft/);
await undo();
assert.equal(ui.state, sample, "Undo restores the very same sample");
assert.equal(json(sample.values), valuesBefore);
assert.equal(json(sample.periodValues), periodBefore);

// Queued: View it, never Amend; Undo returns.
sample.periodAsked = null;
await tick(2);
o = offer();
assert.equal(o.title, "Q2 2024 is queued to send to BIR");
assert.equal(json(o.buttons), json(["existing-view", "existing-keep"]));
assert.ok(!checkboxes.has("existing-fast-lane"), "only drafts offer the fast lane");
await press("existing-view");
assert.notEqual(ui.state, sample);
await undo();
assert.equal(ui.state, sample);

// Submitted: View read-only, Amend (danger), Keep; Amend opens a new amended return.
sample.periodAsked = null;
await tick(3);
o = offer();
assert.equal(o.title, "Q3 2024 was submitted");
assert.equal(json(o.buttons), json(["existing-view", "existing-amend", "existing-keep"]));
assert.equal(buttons.get("existing-amend").variant, "danger");
assert.ok(o.texts.includes("New amended return; the original stays as filed."));
await press("existing-amend");
assert.ok(calls.some((c) => c[0] === "screen_form_amend" && c[3] === 2024 && c[4] === 3), "a new amended return");
assert.equal(ui.state.amends, "Submitted");
await undo();
assert.equal(ui.state, sample, "Undo drops the unsaved amendment");

// Confirmed with no amended answer: filed, View read-only and Keep only.
sample.periodAsked = null;
await tick(4);
o = offer();
assert.equal(o.title, "Q4 2024 was filed");
assert.equal(json(o.buttons), json(["existing-view", "existing-keep"]));
await press("existing-keep");
await undo();

// Don't ask again for drafts: a later draft loads straight away...
sample.periodAsked = null;
await tick(1);
offer();
checkboxes.get("existing-fast-lane").onChange(true, cx);
assert.equal(app.session.fastDrafts, true);
await press("existing-keep");
sample.periodValues = JSON.parse(periodBefore);
sample.periodAsked = null;
await tick(1);
assert.equal(app.overlay, null, "no modal for a draft");
assert.notEqual(ui.state, sample, "loaded straight away");
assert.match(app.undos.at(-1), /chose not to be asked again/);
await undo();
assert.equal(ui.state, sample);
// ...but queued and filed returns still ask.
sample.periodAsked = null;
await tick(3);
assert.equal(app.overlay, "existing", "a filed return still asks");
await press("existing-keep");
console.log("PASS: a stored return is offered per its status; Keep, Esc, Load, View and Amend undo exactly; the fast lane skips only drafts.");
