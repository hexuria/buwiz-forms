// The "fast lane" offer of a stored return (variant D), driven through the
// real app/shell/form_page.js with a mocked host: when a library sample's
// complete period already has a stored return, the app's modal offers what
// its status allows (draft: Load; queued: View; submitted or filed: View
// read-only, and Amend where the host offers it) and Keep this form. Keep
// blanks every period part and keeps the other typed values; Esc or a click
// outside is Keep; each action can be undone once and Undo restores the
// sample exactly, with the offer it was answered from; "Don't ask again for
// drafts" loads a later draft straight away but still asks for queued and
// filed returns. Also: the period is asked about once its typing settles,
// a period change closes a stale offer, a sample built for an earlier
// period goes back to the placeholder, an Undo toast shows only once the
// return opened, Undo never drops unsaved edits, an unsaved amendment
// prints without saving, and an amended return opens its filed original.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);
const schema = JSON.parse(fs.readFileSync(new URL("app/forms/forms-schema.json", root))).find((f) => f.c === "2551Q");
const K = (tail) => `frm2551Qv2018:${tail}`;

const entities = [], tasks = [], timers = [], texts = [], buttons = new Map(), dismissers = new Map(), checkboxes = new Map(), calls = [], toasts = [];
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
    timer: { after(ms, fn) { timers.push({ ms, fn }); return { cancel() {} }; }, every() { return { cancel() {} }; } },
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
// A choice's hint is drawn inside its button, under the label.
uiExports.AppButton = (_cx, id, label, opts = {}) => { buttons.set(id, { label, variant: opts.variant, onClick: opts.onClick, block: opts.block, hint: opts.hint }); if (opts.hint) texts.push(opts.hint); return element(); };
const overlays = new Map();
uiExports.Overlay = (_cx, id, card, onDismiss, opts = {}) => { dismissers.set(id, onDismiss); overlays.set(id, opts); return element().child(card); };
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
// 2024: Q1 a draft, Q2 queued, Q3 submitted (amendable), Q4 confirmed (not
// amendable here). 2025 has nothing stored: a fresh sample per quarter.
const stored = {
  1: { status: "Draft", actions: ["load", "keep"], updated_at: "2026-10-01T08:00:00Z" },
  2: { status: "Queued", actions: ["view", "keep"] },
  3: { status: "Submitted", actions: ["view", "amend", "keep"] },
  4: { status: "Confirmed", actions: ["view", "keep"] },
};
const withDraftValues = (r) => ({ ...r, draft: { ...r.draft, values: r.values } });
const sampleOpened = () => withDraftValues({
  draft: { screen_form_sample: { blank: [] }, sample: true, which: "placeholder" }, status: "Draft", editable: true,
  values: { [K("txtYear")]: "", ...Object.fromEntries(quarterKeys.map((k) => [k, ""])), [K("amendedRtn_2")]: "", [K("txtSheets")]: "" },
  options: {}, locks: {}, profile: [], profile_edit: [], sample: true,
  blank: [K("txtYear"), ...quarterKeys], needs: {}, period_parts: periodParts, period_set: false, errors: [],
});
const freshSample = (year, quarter) => withDraftValues({
  ...sampleOpened(),
  draft: { screen_form_sample: { blank: [] }, sample: true, which: `fresh ${year} Q${quarter}` },
  values: { [K("txtYear")]: String(year), ...Object.fromEntries(quarterKeys.map((k, i) => [k, i + 1 === quarter ? "true" : "false"])), [K("amendedRtn_2")]: "", [K("txtSheets")]: "7" },
  blank: [], period_set: true,
});
const returnOpened = (draft, extra = {}) => ({ draft, status: "Draft", editable: true, values: {}, options: {}, locks: {}, profile: [], profile_edit: [], sample: false, blank: [], needs: {}, period_parts: [], errors: [], ...extra });
let failOpen = null;
const app = {
  prefs: {}, branchFor: () => ({ tin: "12345678800000" }), taxpayers: () => [], layout: () => ({ column: 1056 }),
  overlay: null, existingOffer: null, fastLane: { drafts: false }, undo: null, undos: [],
  invalidate() {}, toast(text) { toasts.push(text); }, go() {}, startNewReturn() { calls.push(["startNewReturn"]); }, templates: () => [],
  toastUndo(message, undo) { this.undos.push(message); this.undo = undo; },
  openForm(opts, cx, keep, then) { calls.push(["openForm", opts]); this.formUi.open(opts, cx, keep, then); },
  store: {
    templates: () => [],
    write(name, ...args) {
      calls.push([name, ...args]);
      if (name === "screen_form_save") return { draft: { ...args[1], id: 77 }, status: "Draft" };
      throw Error("no writes");
    },
    call(name, ...args) {
      calls.push([name, ...args]);
      if (name === "screen_form_schema") return schema;
      if (name === "screen_form_sample") return Promise.resolve(sampleOpened());
      if (name === "screen_form_open") {
        if (failOpen) return Promise.reject(Error(failOpen));
        return Promise.resolve(returnOpened({ id: 9, opened: args.slice(2) }, { filed_original: args[2] === 2024 && args[3] === 3 ? { id: 5, status: "Submitted", filed_on: "2026-09-01T00:00:00Z" } : null }));
      }
      if (name === "screen_form_amend") return Promise.resolve({ ...returnOpened({ amended: args.slice(2) }), amends: "Submitted" });
      if (name === "screen_form_filed_original") return Promise.resolve(returnOpened({ snapshot: args[1] }, { editable: false, status: "Submitted", filed_original_view: { status: "Submitted", filed_on: "2026-09-01T00:00:00Z" } }));
      // The mock draft carries its values; a fill answers them all.
      if (name === "screen_form_fill") return { draft: args[1], values: { ...(args[1].values || {}), ...args[2] }, options: {}, rejected: {} };
      if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: {}, options: {}, errors: [] });
      if (name === "screen_form_print") return true;
      if (name === "screen_form_sample_period") {
        const parts = args[2];
        const quarter = quarterKeys.findIndex((k) => parts[k] === "true") + 1;
        const year = parts[K("txtYear")];
        // The host counts a year only with all four digits.
        if (!/^\d{4}$/.test(year || "") || !quarter) return Promise.resolve({ complete: false });
        const key = 100 + quarter; // the stored key the host answers (a number)
        if (year !== "2024") return Promise.resolve({ complete: true, opens: { year: Number(year), period: quarter, key: null }, sample: freshSample(Number(year), quarter) });
        const s = stored[quarter];
        return Promise.resolve({
          complete: true,
          opens: { year: 2024, period: quarter, key },
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
// Timers run (the period ask waits for its typing to settle), then the host.
const settle = async () => {
  for (;;) {
    while (timers.length) timers.shift().fn(context());
    if (!tasks.length) break;
    await Promise.all(tasks.splice(0));
  }
};
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
const asks = () => calls.filter((c) => c[0] === "screen_form_sample_period");
const lastOpen = () => calls.filter((c) => c[0] === "screen_form_open").at(-1);

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
// The modal: centred (the shared Overlay's default), 420 wide, full-width
// choices whose explanation sits inside the button.
assert.equal(overlays.get("existing-offer").width, 420);
assert.ok(!overlays.get("existing-offer").top, "centred on both axes, not anchored to the top");
assert.equal(buttons.get("existing-load").block, true);
assert.equal(buttons.get("existing-load").hint, "Continue where you left off.");
assert.equal(buttons.get("existing-keep").hint, "Stay in the sample; the period clears.");
assert.ok(o.texts.includes("Continue where you left off.") && o.texts.includes("Stay in the sample; the period clears."));
assert.ok(checkboxes.has("existing-fast-lane"), "drafts offer the fast lane");

// Keep: every period part blank, the other typed values kept; Undo brings
// the period back, every box as it showed, and the offer it was kept from.
const periodBefore = json(sample.periodValues), valuesBefore = json(sample.values);
const inputsOf = (s) => json([...s.inputs].map(([k, input]) => [k, input.value()]));
const inputsBefore = inputsOf(sample);
await press("existing-keep");
assert.equal(app.overlay, null);
assert.equal(json(sample.periodValues), "{}", "the period clears");
assert.equal(sample.values[K("txtYear")], "");
assert.equal(sample.inputs.get(K("txtYear")).value(), "", "the year box clears");
assert.equal(sample.values[quarterKeys[0]], "");
assert.equal(sample.values[K("amendedRtn_2")], "true", "the other typed values stay");
assert.equal(json(sample.typedEdits), typed);
assert.match(app.undos.at(-1), /Kept this form/);
await undo();
assert.equal(json(sample.periodValues), periodBefore, "Undo brings the period back");
assert.equal(json(sample.values), valuesBefore, "exactly as it was");
assert.equal(inputsOf(sample), inputsBefore, "every box shows what it showed");
assert.equal(app.overlay, "existing", "Undo brings back the offer Keep answered");
assert.equal(app.existingOffer, sample.existing);
app.overlay = null; app.existingOffer = null;

// Esc or a click outside is Keep.
sample.periodAsked = null;
await tick(1);
offer();
dismissers.get("existing-offer")(cx);
await settle();
assert.equal(app.overlay, null);
assert.equal(json(sample.periodValues), "{}", "Esc kept the form and cleared the period");
await undo();

// Load: exactly the matched return opens (its stored key); Undo restores
// the sample exactly, with the offer open again.
sample.periodAsked = null;
await tick(1);
offer();
await press("existing-load");
assert.notEqual(ui.state, sample, "the saved draft is open");
assert.equal(json(calls.filter((c) => c[0] === "openForm").at(-1)[1]), json({ tin: "12345678800000", code: "2551Q", year: 2024, period: 1, key: 101 }));
assert.equal(json(lastOpen().slice(1)), json(["12345678800000", "2551Q", 2024, 1, 101]), "opened by its stored key");
assert.match(app.undos.at(-1), /Loaded your saved Q1 2024 draft/);
await undo();
assert.equal(ui.state, sample, "Undo restores the very same sample");
assert.equal(json(sample.values), valuesBefore);
assert.equal(json(sample.periodValues), periodBefore);
assert.equal(app.overlay, "existing", "with the offer it was loaded from");
app.overlay = null; app.existingOffer = null;

// Queued: View it, never Amend; Undo returns.
sample.periodAsked = null;
await tick(2);
o = offer();
assert.equal(o.title, "Q2 2024 is queued to send to BIR");
assert.equal(json(o.buttons), json(["existing-view", "existing-keep"]));
assert.ok(!checkboxes.has("existing-fast-lane"), "only drafts offer the fast lane");
await press("existing-view");
assert.notEqual(ui.state, sample);
assert.equal(lastOpen()[5], 102);
await undo();
assert.equal(ui.state, sample);

// Submitted: View read-only, Amend (danger), Keep; Amend opens a new
// amended return of exactly that return; its filed version is kept in the
// return's history.
sample.periodAsked = null;
await tick(3);
o = offer();
assert.equal(o.title, "Q3 2024 was submitted");
assert.equal(json(o.buttons), json(["existing-view", "existing-amend", "existing-keep"]));
assert.equal(buttons.get("existing-amend").variant, "danger");
assert.ok(o.texts.includes("New amended return. The filed version is kept in this return's history."));
assert.ok(!o.texts.some((t) => /stays as filed/.test(t)), "no wording says the original stays as filed");
await press("existing-amend");
const amendCall = calls.filter((c) => c[0] === "screen_form_amend").at(-1);
assert.equal(json(amendCall.slice(1)), json(["12345678800000", "2551Q", 2024, 3, 103]), "a new amended return of that very return");
assert.equal(ui.state.amends, "Submitted");
// D3: print preview of an amendment not saved yet prints from the page and
// saves nothing.
const writesBefore = calls.filter((c) => c[0] === "screen_form_save").length;
ui.print(cx);
assert.equal(calls.filter((c) => c[0] === "screen_form_save").length, writesBefore, "printing an unsaved amendment never saves it");
assert.deepEqual(calls.filter((c) => c[0] === "screen_form_print").at(-1)[2], ui.state.draft, "it prints the draft on the page");
await undo();
assert.equal(ui.state, sample, "Undo drops the unsaved amendment");
app.overlay = null; app.existingOffer = null;

// D6: Undo after the amendment's first save: back to the sample, and the
// toast says the amendment stays saved. Unsaved edits are never dropped.
sample.periodAsked = null;
await tick(3);
offer();
await press("existing-amend");
const amendment = ui.state;
ui.edit(K("txtSheets"), "2", cx);
const undoAfterEdit = app.undo;
toasts.length = 0;
undoAfterEdit(cx);
await settle();
assert.equal(ui.state, amendment, "Undo refuses while an edit is not saved");
assert.match(toasts.at(-1), /Undo would lose edits you have not saved/);
ui.save(cx);
assert.equal(ui.state.saved, true);
undoAfterEdit(cx);
await settle();
assert.equal(ui.state, sample, "after the save, Undo returns to the sample");
assert.equal(toasts.at(-1), "Back to the sample. The amended return was saved and stays saved.");
app.overlay = null; app.existingOffer = null;

// Confirmed with no amended answer: filed, View read-only and Keep only.
sample.periodAsked = null;
await tick(4);
o = offer();
assert.equal(o.title, "Q4 2024 was filed");
assert.equal(json(o.buttons), json(["existing-view", "existing-keep"]));
await press("existing-keep");
await undo();
app.overlay = null; app.existingOffer = null;

// V4: the Undo toast shows only once the return opened; a failed open says
// why and leaves the sample on screen.
sample.periodAsked = null;
await tick(1);
offer();
const undosBefore = app.undos.length;
failOpen = "database is locked";
await press("existing-load");
failOpen = null;
assert.equal(app.undos.length, undosBefore, "no Undo toast for a return that did not open");
assert.equal(ui.state, sample, "the sample stays");
assert.equal(toasts.at(-1), "Could not open Q1 2024: database is locked");

// Don't ask again for drafts: a later draft loads straight away...
sample.periodAsked = null;
await tick(1);
offer();
checkboxes.get("existing-fast-lane").onChange(true, cx);
assert.equal(app.fastLane.drafts, true, "the fast lane is the app's own field");
await press("existing-keep");
sample.periodValues = JSON.parse(periodBefore);
sample.periodAsked = null;
await tick(1);
assert.equal(app.overlay, null, "no modal for a draft");
assert.notEqual(ui.state, sample, "loaded straight away");
assert.match(app.undos.at(-1), /chose not to be asked again/);
await undo();
assert.equal(ui.state, sample);
assert.equal(app.overlay, null, "Undo of a fast-lane load opens no offer: none was open");
// ...but queued and filed returns still ask.
sample.periodAsked = null;
await tick(3);
assert.equal(app.overlay, "existing", "a filed return still asks");
await press("existing-keep");

// The fast lane never loads mid-typing: the year is asked about once its
// typing settles, and only all four digits make a period.
await tick(1);
const yearInput = sample.inputs.get(K("txtYear"));
const opensBefore = calls.filter((c) => c[0] === "screen_form_open").length;
const asksBefore = asks().length;
for (const text of ["2", "20", "202"]) {
  yearInput.set_value(text);
  ui.edit(K("txtYear"), text, cx);
}
await settle();
assert.equal(asks().length, asksBefore + 1, "one ask once the typing settled");
assert.equal(asks().at(-1)[3][K("txtYear")], "202");
assert.equal(ui.state, sample, "nothing loaded on a partial year");
assert.equal(calls.filter((c) => c[0] === "screen_form_open").length, opensBefore);
yearInput.set_value("2024");
ui.edit(K("txtYear"), "2024", cx);
await settle();
assert.notEqual(ui.state, sample, "the whole year loads the draft");
await undo();
app.fastLane.drafts = false;

// B6: a period change closes a stale offer at once; its buttons then do
// nothing, and Start a return waits for the new answer.
sample.periodAsked = null;
await tick(1);
offer();
const staleLoad = buttons.get("existing-load").onClick;
ui.choose(Object.fromEntries(quarterKeys.map((k, i) => [k, i === 1 ? "true" : "false"])), cx);
assert.equal(app.overlay, null, "the offer closes before the new answer");
assert.equal(app.existingOffer, null);
assert.equal(sample.existing, null, "the last answer is forgotten");
assert.equal(sample.opens, null);
const openFormsBefore = calls.filter((c) => c[0] === "openForm").length;
staleLoad({}, cx);
await settle();
assert.equal(calls.filter((c) => c[0] === "openForm").length, openFormsBefore, "a stale Load opens nothing");
assert.equal(ui.state, sample);
await press("existing-keep");
app.overlay = null; app.existingOffer = null;
// Leaving the sample (another form) clears any offer.
sample.periodValues = JSON.parse(periodBefore);
sample.periodAsked = null;
await tick(1);
assert.equal(app.overlay, "existing");
const keptSample = ui.state;
ui.open({ code: "2551Q", tin: "12345678800000", year: 2024, period: 2 }, cx, true);
assert.equal(app.overlay, null, "another form closes the offer");
assert.equal(app.existingOffer, null);
await settle();
ui.restoreForm(keptSample, cx);
assert.equal(ui.state, sample);
app.overlay = null; app.existingOffer = null;

// B7: a sample built for a complete period goes back to the placeholder
// once its period is incomplete, and before an existing return is offered.
sample.periodValues = {}; sample.periodAsked = null;
yearInput.set_value("2025");
ui.edit(K("txtYear"), "2025", cx);
await tick(2);
assert.equal(sample.periodSet, true);
assert.equal(sample.draft.which, "fresh 2025 Q2");
assert.equal(sample.inputs.get(K("txtSheets")).value(), "7", "a box the fresh sample filled");
ui.choose({ [quarterKeys[1]]: "false" }, cx);
await settle();
assert.equal(sample.opens, null);
assert.equal(sample.periodSet, false, "incomplete: back to the placeholder");
assert.equal(sample.draft.which, "placeholder");
assert.equal(sample.inputs.get(K("txtYear")).value(), "2025", "what the filer typed stays");
await tick(3);
assert.equal(sample.draft.which, "fresh 2025 Q3");
yearInput.set_value("2024");
ui.edit(K("txtYear"), "2024", cx);
await settle();
assert.equal(sample.draft.which, "placeholder", "an existing answer: back to the placeholder");
assert.equal(sample.periodSet, false);
assert.equal(app.overlay, "existing", "then the offer");
assert.equal(sample.inputs.get(K("txtSheets")).value(), "", "nothing of the earlier period stays");
// V2: Keep, then Undo: every box shows exactly what it showed.
const shownBeforeKeep = inputsOf(sample);
await press("existing-keep");
await undo();
assert.equal(inputsOf(sample), shownBeforeKeep);
app.overlay = null; app.existingOffer = null;

// B4: no period answer (an entry-numbered form, or the period not set):
// Start a return opens New return.
sample.periodValues = {}; sample.periodAsked = null;
ui.choose({ [quarterKeys[2]]: "false" }, cx);
await settle();
ui.startReturn(cx);
assert.equal(json(calls.at(-1)), json(["startNewReturn"]));
// A period part set without typing (a picker, a click) shows in its box.
ui.choose({ [K("txtYear")]: "2031" }, cx);
await settle();
assert.equal(yearInput.value(), "2031", "the year box shows the year set");

// D4: an amended return names its filed original; View the filed original
// opens it read-only from the history, with a strip saying what it is.
ui.open({ code: "2551Q", tin: "12345678800000", year: 2024, period: 3, key: 103 }, cx);
await settle();
assert.equal(ui.state.filedOriginal.id, 5);
ui.openFiledOriginal(cx);
await settle();
assert.equal(json(calls.filter((c) => c[0] === "screen_form_filed_original").at(-1)), json(["screen_form_filed_original", "2551Q", 5]));
assert.equal(ui.state.editable, false, "read-only");
assert.equal(ui.state.filedOriginalView.status, "Submitted");
assert.equal(
  mod.namespace.filedOriginalStrip(ui.state.filedOriginalView),
  "This is the version filed on Sep 1, 2026, kept in this return's history; the current return is its amendment. Nothing here can be changed.",
);
ui.backToAmendment(cx);
await settle();
assert.equal(json(lastOpen().slice(1)), json(["12345678800000", "2551Q", 2024, 3, 103]), "back to the amendment");
assert.equal(mod.namespace.FILED_KEPT, "The filed version is kept in this return's history.");

// No grey box without a visible reason: a shut box that shows blank draws
// its lock note (filer words) under its item; one with a value does not.
const NOT_YET = "Can't be filled on this page yet. It will be added for this form.";
const emailItem = schema.parts.flatMap((p) => p.items).find((it) => (it.f || []).some((f) => f.k === "txtEmail#occurrence-1"));
ui.state.status = "Draft";
ui.state.editable = true;
ui.state.locks = { "txtEmail#occurrence-1": "repeats the official field `txtEmail`, which this page cannot write", "txtEmail#occurrence-2": "same" };
ui.state.lockNotes = { "txtEmail#occurrence-1": NOT_YET, "txtEmail#occurrence-2": NOT_YET };
ui.state.values = { "txtEmail#occurrence-1": "", "txtEmail#occurrence-2": "" };
assert.equal(json(ui.lockNotesFor(emailItem)), json([NOT_YET]), "the shut, blank email says why, once");
ui.state.values = { "txtEmail#occurrence-1": "a@b.ph", "txtEmail#occurrence-2": "a@b.ph" };
assert.equal(json(ui.lockNotesFor(emailItem)), json([]), "a box showing its value needs no note");
ui.state.locks = {};
ui.state.values = {};
assert.equal(json(ui.lockNotesFor(emailItem)), json([]), "an open box needs no note");
console.log("PASS: a stored return is offered per its status and opened exactly; Keep, Esc, Load, View and Amend undo exactly; stale offers close; the fast lane skips only drafts and never loads mid-typing.");
