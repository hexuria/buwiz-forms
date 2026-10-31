// Saving a Form library sample (decided 2026-10-17), driven through the real
// app/shell/form_page.js with a mocked host:
//  - a sample whose period is set saves as that period's return, with what
//    was typed into it (staged edits included), and the return it became
//    opens (exactly, by its stored key);
//  - a period that already has a stored return offers it (the existing-
//    return modal) and saves nothing, also when the host finds one that
//    was stored meanwhile;
//  - a period not set asks for it in the app's period modal (the form's
//    period parts); an incomplete answer asks again and says why; a period
//    typed but not answered yet is asked about first;
//  - Queue saves the sample the same way, then queues the return it
//    became;
//  - (decided 2026-10-22) the header is exactly Print preview, Check, Queue
//    and Save, and Print preview opens at once from the page's values: no
//    period modal, nothing saved, no stored return.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { kitMethods, strictElement } from "./kit_surface.mjs";
const root = new URL("../", import.meta.url);
const KIT = kitMethods(root);
const schema = JSON.parse(fs.readFileSync(new URL("app/forms/forms-schema.json", root))).find((f) => f.c === "2551Q");
const K = (tail) => `frm2551Qv2018:${tail}`;

const entities = [], tasks = [], timers = [], texts = [], buttons = new Map(), dismissers = new Map(), fields = new Map(), calls = [], toasts = [];
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
  return strictElement(KIT, (k, args, el) => {
    if (k === "when" && args[0]) args[1](el);
    if (k === "child") mount(args[0]);
    if (k === "children") for (const c of args[0]) mount(c);
  });
}
function mount(e) { if (e?.view && e.dirty && !e.released) { e.dirty = false; const old = owner; owner = e; e.view.render(context(e)); owner = old; } }
const InputState = { new({ value = "" } = {}) { let text = value; return { value: () => text, set_value: (v) => { text = v; }, on() {}, release() { return true; } }; } };
const contextVM = vm.createContext({ console, Map, Set, JSON, Date, window: { viewport_size: () => ({ width: 1400, height: 900 }) } });
function mock(name, exports) { return new vm.SyntheticModule(Object.keys(exports), function () { for (const [k, v] of Object.entries(exports)) this.setExport(k, v); }, { context: contextVM, identifier: name }); }
const uiExports = { tokens: () => new Proxy({}, { get: () => "#000" }), SPACE: [0, 4, 8, 12, 16, 20, 24], RADIUS: {}, SIZE: { validationLine: 18 }, BREAK: { formNav: 760 } };
for (const n of ["Text", "Line", "Code", "Icon", "AppButton", "IconButton", "Avatar", "Chip", "Switch", "Checkbox", "TextField", "Dropdown", "PageHeader", "MetaLine", "Overlay", "CheckboxLabel"]) uiExports[n] = element;
uiExports.Text = (_cx, _v, text) => { texts.push(String(text)); return element(); };
uiExports.AppButton = (_cx, id, label, opts = {}) => { buttons.set(id, { label, variant: opts.variant, onClick: opts.onClick }); return element(); };
uiExports.TextField = (_cx, state, opts = {}) => { fields.set(opts.id, state); return element(); };
uiExports.Overlay = (_cx, id, card, onDismiss) => { dismissers.set(id, onDismiss); return element().child(card); };
const mod = new vm.SourceTextModule(fs.readFileSync(new URL("app/shell/form_page.js", root), "utf8"), { context: contextVM, identifier: "form_page.js" });
await mod.link((s) => ({
  "gpui-kit": mock("gpui-kit", { div: element, svg: element, View, list: (_id, n, _key, render) => { const el = element(); for (let i = 0; i < n; i++) el.child(render(i)); return el; } }),
  "gpui-base": mock("gpui-base", { InputState, CalendarState: {}, Popup: { new: element } }),
  "gpui-component": mock("gpui-component", { Spinner: { new: element } }),
  "./ui.js": mock("./ui.js", uiExports),
  "./store.js": mock("./store.js", { formatTin: (x) => x, errorText: (e) => String(e && e.message ? e.message : e) }),
  "./periods.js": mock("./periods.js", { periodLabel: () => "Q2 2025" }),
})[s]);
await mod.evaluate();
const { FormPageActions, renderSamplePeriod, renderExistingOffer, renderFormPage } = mod.namespace;

const quarterKeys = [1, 2, 3, 4].map((n) => K(`qtr_${n}`));
const periodParts = [{ key: K("txtYear"), part: "year", quarter: null }, ...quarterKeys.map((key, i) => ({ key, part: "quarter", quarter: i + 1 }))];
const AMOUNT = K("txtSheets");
// 2024 Q1 has a stored draft; every other complete period is new.
const sampleOpened = () => ({
  draft: { screen_form_sample: { blank: [K("txtYear"), ...quarterKeys], set: false }, which: "placeholder", typed: {} },
  status: "Draft", editable: true,
  values: { [K("txtYear")]: "", ...Object.fromEntries(quarterKeys.map((k) => [k, ""])), [AMOUNT]: "" },
  options: {}, locks: {}, profile: [], profile_edit: [], sample: true,
  blank: [K("txtYear"), ...quarterKeys], needs: {}, period_parts: periodParts, period_set: false, errors: [],
});
const fresh = (year, quarter) => ({
  ...sampleOpened(),
  draft: { screen_form_sample: { blank: [], set: true }, which: `fresh ${year} Q${quarter}`, typed: {} },
  values: { [K("txtYear")]: String(year), ...Object.fromEntries(quarterKeys.map((k, i) => [k, String(i + 1 === quarter)])), [AMOUNT]: "" },
  blank: [], period_set: true,
});
const existing = { form_code: "2551Q", year: 2024, period: 1, label: "Q1 2024", status: "Draft", actions: ["load", "keep"], updated_at: "2026-10-01T08:00:00Z", total_payable: 0, submitted_at: null, confirmed_at: null };
let storedMeanwhile = false;
const app = {
  prefs: {}, branchFor: () => ({ tin: "12345678800000" }), taxpayers: () => [], layout: () => ({ column: 1056 }),
  overlay: null, existingOffer: null, fastLane: { drafts: false }, samplePeriod: null,
  invalidate() {}, toast(text) { toasts.push(text); }, go() {}, startNewReturn() { calls.push(["startNewReturn"]); }, toastUndo() {},
  openForm(opts, cx, keep, then) { calls.push(["openForm", opts]); this.formUi.open(opts, cx, keep, then); },
  store: {
    templates: () => [],
    write(name, ...args) {
      calls.push([name, ...args]);
      if (name === "screen_form_save") {
        const draft = args[1];
        if (storedMeanwhile) return { saved: false, existing, opens: { year: 2024, period: 1, key: 1 } };
        assert.ok(draft.screen_form_sample?.set, "only a sample rebuilt for its period reaches save");
        const quarter = Number(draft.which.slice(-1));
        return { saved: true, id: 77, status: "Draft", stored_period: quarter, opens: { year: 2025, period: quarter, key: quarter }, draft: { id: 77 } };
      }
      if (name === "queue_form_draft") return 1;
      throw Error(`no write ${name}`);
    },
    call(name, ...args) {
      calls.push([name, ...args]);
      if (name === "screen_form_schema") return schema;
      if (name === "screen_form_sample") return Promise.resolve(sampleOpened());
      if (name === "screen_form_open") return Promise.resolve({ draft: { id: 77 }, status: "Draft", editable: true, values: {}, options: {}, locks: {}, profile: [], profile_edit: [], sample: false, blank: [], needs: {}, period_parts: [], errors: [] });
      // A fill keeps what was typed in the draft (so a save can be checked).
      if (name === "screen_form_fill") return { draft: { ...args[1], typed: { ...(args[1].typed || {}), ...args[2] } }, values: { ...args[2] }, options: {}, rejected: {} };
      if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: {}, options: {}, errors: [] });
      if (name === "screen_form_print") return true;
      if (name === "screen_form_print_sample") return { path: "/tmp/preview.html" };
      if (name === "screen_form_preview_sync") return true;
      if (name === "screen_form_sample_period") {
        const parts = args[2];
        const quarter = quarterKeys.findIndex((k) => parts[k] === "true") + 1;
        const year = parts[K("txtYear")];
        if (!/^\d{4}$/.test(year || "") || !quarter) return Promise.resolve({ complete: false });
        if (year === "2024" && quarter === 1) return Promise.resolve({ complete: true, opens: { year: 2024, period: 1, key: 1 }, existing });
        return Promise.resolve({ complete: true, opens: { year: Number(year), period: quarter, key: null, new_key: quarter }, sample: fresh(Number(year), quarter) });
      }
      throw Error(name);
    },
  },
};
app.formUi = new FormPageActions(app);
const ui = app.formUi;
const cx = context();
const settle = async () => {
  for (;;) {
    while (timers.length) timers.shift().fn(context());
    if (!tasks.length) break;
    await Promise.all(tasks.splice(0));
  }
};
const json = (v) => JSON.stringify(v);
const named = (name) => calls.filter((c) => c[0] === name);
const press = async (id) => { buttons.get(id).onClick({}, cx); await settle(); };
async function openSample() {
  app.overlay = null; app.existingOffer = null; storedMeanwhile = false;
  ui.open({ code: "2551Q", tin: "12345678800000", year: 2026, period: 0, sample: true }, cx);
  await settle();
}
const tick = (quarter) => Object.fromEntries(quarterKeys.map((k, i) => [k, String(i + 1 === quarter)]));
function modal() {
  texts.length = 0; buttons.clear(); fields.clear(); dismissers.clear();
  renderSamplePeriod(cx, app);
  return [...texts];
}

// ---- A period set: Save makes it that period's return and opens it.
await openSample();
ui.setPeriodPart({ [K("txtYear")]: "2025", ...tick(2) }, cx);
await settle();
assert.equal(ui.state.periodSet, true, "rebuilt for 2025 Q2");
ui.edit(AMOUNT, "7", cx); // staged, not flushed yet
let saves = named("screen_form_save").length;
ui.save(cx);
await settle();
assert.equal(named("screen_form_save").length, saves + 1, "saved");
const sent = named("screen_form_save").at(-1)[2];
assert.equal(sent.which, "fresh 2025 Q2", "the sample built for its period");
assert.equal(sent.typed[AMOUNT], "7", "with what was typed, staged edits included");
assert.equal(json(named("openForm").at(-1)[1]), json({ tin: "12345678800000", code: "2551Q", year: 2025, period: 2, returnKey: 2 }), "the return it became opens exactly");
assert.ok(!ui.state.sample && ui.state.draft.id === 77, "the page now holds the return");
assert.ok(toasts.at(-1).startsWith("Saved."), toasts.at(-1));
console.log("PASS: a sample with its period set saves as that period's return, typed values and all, and opens it.");

// ---- A period with a stored return: the offer, nothing saved.
await openSample();
ui.setPeriodPart({ [K("txtYear")]: "2024", ...tick(1) }, cx);
await settle();
app.overlay = null; // the answer offered it; Save offers it again
saves = named("screen_form_save").length;
ui.save(cx);
assert.equal(app.overlay, "existing", "the existing-return modal");
assert.equal(app.existingOffer, existing);
assert.equal(named("screen_form_save").length, saves, "a sample never saves over a stored return");
texts.length = 0; buttons.clear();
renderExistingOffer(cx, app);
assert.ok(buttons.has("existing-load") && buttons.has("existing-keep"), "Load / Keep");
// Stored meanwhile: the host finds it at save time and answers the offer.
await openSample();
ui.setPeriodPart({ [K("txtYear")]: "2025", ...tick(3) }, cx);
await settle();
storedMeanwhile = true;
ui.save(cx);
assert.equal(app.overlay, "existing", "the host's offer");
assert.equal(json(ui.state.opens), json({ year: 2024, period: 1, key: 1 }), "it opens what the host matched");
assert.equal(named("openForm").filter((c) => c[1].returnKey === undefined && !c[1].sample).length, 0, "nothing opened by itself");
storedMeanwhile = false;
console.log("PASS: a period with a stored return offers it and saves nothing.");

// ---- No period yet: the app's period modal.
await openSample();
saves = named("screen_form_save").length;
ui.save(cx);
assert.equal(app.overlay, "sample-period", "Save asks for the period");
let shown = modal();
assert.ok(shown.includes("Which period is this return for?"));
assert.ok(fields.has("sample-period-year"), "the year box");
assert.ok(["sample-period-q1", "sample-period-q2", "sample-period-q3", "sample-period-q4"].every((id) => buttons.has(id)), "a choice per quarter");
// Only the year: asked, incomplete, asked again with why.
fields.get("sample-period-year").set_value("2025");
await press("sample-period-save");
assert.equal(app.overlay, "sample-period", "an incomplete period asks again");
shown = modal();
assert.ok(shown.includes("Enter the whole period to save this sample."), "and says why");
assert.equal(fields.get("sample-period-year").value(), "2025", "keeping what was typed");
assert.equal(named("screen_form_save").length, saves, "nothing saved yet");
// The quarter too: the sample is rebuilt for it and saved.
await press("sample-period-q4");
modal();
assert.equal(buttons.get("sample-period-q4").variant, "primary", "the chosen quarter shows");
await press("sample-period-save");
assert.equal(app.overlay, null, "the modal closes");
assert.equal(named("screen_form_save").length, saves + 1, "saved once the period is complete");
assert.equal(named("screen_form_save").at(-1)[2].which, "fresh 2025 Q4");
assert.equal(json(named("openForm").at(-1)[1]), json({ tin: "12345678800000", code: "2551Q", year: 2025, period: 4, returnKey: 4 }));
// The modal's period is a stored one: the offer.
await openSample();
ui.save(cx);
modal();
fields.get("sample-period-year").set_value("2024");
await press("sample-period-q1");
modal();
saves = named("screen_form_save").length;
await press("sample-period-save");
assert.equal(app.overlay, "existing", "the stored Q1 2024 is offered");
assert.equal(named("screen_form_save").length, saves);
// Cancel (or Esc) closes it with nothing written.
await openSample();
ui.save(cx);
modal();
dismissers.get("sample-period")(cx);
assert.equal(app.overlay, null);
assert.equal(app.samplePeriod, null);
console.log("PASS: with no period, Save asks for it in the app's modal, again when incomplete, then saves or offers.");

// ---- A period typed but not answered yet is asked about first.
await openSample();
ui.setPeriodPart({ [K("txtYear")]: "2025", ...tick(1) }, cx); // its ask waits on a timer
saves = named("screen_form_save").length;
ui.save(cx);
await settle();
assert.equal(named("screen_form_save").length, saves + 1, "saved on the answer");
assert.equal(named("screen_form_save").at(-1)[2].which, "fresh 2025 Q1");
console.log("PASS: Save waits for the period's answer, then saves.");

// ---- Queue saves first, then acts on the return.
await openSample();
ui.setPeriodPart({ [K("txtYear")]: "2025", ...tick(2) }, cx);
await settle();
saves = named("screen_form_save").length;
ui.queue(cx);
await settle();
assert.equal(named("screen_form_save").length, saves + 1, "Queue saves the sample");
assert.equal(json(named("queue_form_draft").at(-1)), json(["queue_form_draft", "12345678800000", "2551Q", 2025, 2]), "then queues the return");
await openSample();
ui.queue(cx);
assert.equal(app.overlay, "sample-period", "with no period, Queue asks for it too");
console.log("PASS: Queue saves a sample as Save does, then queues its return.");

// ---- Decided 2026-10-22: a sample's header is Print preview, Check, Queue and Save.
const HEADER = ["fp-print", "fp-check", "fp-start-return", "fp-queue", "fp-save", "fp-filed-original", "fp-back-to-amendment", "fp-cancel-queue"];
await openSample();
buttons.clear();
renderFormPage(cx, app, 1056);
// The Check button is its own entity, drawn when the header mounts it.
assert.deepEqual([...buttons.keys()].filter((id) => HEADER.includes(id)).sort(), ["fp-check", "fp-print", "fp-queue", "fp-save"], "exactly four header buttons on a sample");
console.log("PASS: a sample's header is Print preview, Check, Queue and Save.");

// ---- Print preview on a fresh sample opens at once: no period, nothing saved.
const STORING = ["screen_form_save", "screen_form_queue", "queue_form_draft", "screen_form_print", "screen_form_sample_period"];
await openSample();
app.overlay = null; app.samplePeriod = null; // Queue's modal above
ui.edit(AMOUNT, "9", cx); // staged, not flushed yet
let mark = calls.length;
buttons.get("fp-print").onClick({}, cx);
assert.equal(app.overlay, null, "no period modal");
assert.equal(app.samplePeriod, null);
let after = calls.slice(mark);
assert.deepEqual(after.filter((c) => STORING.includes(c[0])), [], "nothing saved, queued or printed as a return");
const preview = named("screen_form_print_sample").at(-1);
assert.ok(preview, "the sample preview verb");
assert.equal(preview[1], "2551Q");
assert.equal(preview[2], ui.state.draft, "the draft on the page");
assert.equal(json(preview[3]), json({ [AMOUNT]: "9" }), "with the staged edits");
assert.equal(ui.state.previewOpen, true, "the preview follows edits");
assert.ok(ui.state.sample, "the page still holds the sample");
await settle();
assert.deepEqual(calls.slice(mark).filter((c) => STORING.includes(c[0])), [], "still nothing stored after the edit flushes");
assert.ok(named("screen_form_preview_sync").length > 0 && calls.slice(mark).some((c) => c[0] === "screen_form_preview_sync"), "live sync follows the sample");
// Its period set, Print preview still saves nothing.
await openSample();
ui.setPeriodPart({ [K("txtYear")]: "2025", ...tick(3) }, cx);
await settle();
mark = calls.length;
ui.print(cx);
await settle();
after = calls.slice(mark);
assert.deepEqual(after.filter((c) => STORING.includes(c[0])), [], "a set period does not save on print either");
assert.equal(named("screen_form_print_sample").at(-1)[2].which, "fresh 2025 Q3", "it previews the sample built for its period");
console.log("PASS: Print preview on a sample opens at once from the page and stores nothing.");
