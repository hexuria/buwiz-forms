// Form page rules 1 and 2 (docs/form-findings rules page), driven through the
// real app/shell/form_page.js with a mocked host:
//  - a Form library sample shows its period and date boxes blank, keeps them
//    blank after the host refills them, does not judge them, and can never be
//    saved, queued or printed;
//  - profile-filled items fold into "From tax profile · N filled boxes"; a
//    box the profile left blank stays in paper order; Edit (click or Enter)
//    opens a profile box, its edit reaches screen_form_fill as `unlocked`,
//    and clicking away gives the box its profile look back with the value
//    kept on this (now unsaved) return.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);
const schema = JSON.parse(fs.readFileSync(new URL("app/forms/forms-schema.json", root))).find((f) => f.c === "2551Q");
const K = (tail) => `frm2551Qv2018:${tail}`;
const partOf = (key) => schema.parts.findIndex((p) => p.items.some((it) => (it.f || []).some((f) => f.k === key)));
const itemOf = (key) => schema.parts[partOf(key)].items.findIndex((it) => (it.f || []).some((f) => f.k === key));
const BG = partOf(K("telNo"));
assert.ok(BG >= 0, "2551Q has a telNo box");

const traces = [], entities = [], tasks = [], texts = [], buttons = [], toasts = [];
const clicks = new Map(), keys = new Map(), downOut = new Map();
const calls = [];
let rootDirty = true, owner = null;
class View {}
function context(entity = null) {
  return {
    notify() { if (entity) entity.dirty = true; else rootDirty = true; },
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
  const creationOwner = owner;
  const target = {};
  const el = new Proxy(target, { get(o, k) {
    if (k === "then") return undefined;
    return (...args) => {
      if (k === "id") { o.id = args[0]; if (String(o.id).startsWith("fp-item-")) traces.push(o.id); }
      if (k === "on_click") clicks.set(o.id, { fn: args[0], entity: creationOwner });
      if (k === "on_key_down") keys.set(o.id, { fn: args[0], entity: creationOwner });
      if (k === "on_mouse_down_out") downOut.set(o.id, { fn: args[0], entity: creationOwner });
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
const inputs = new Map();
const InputState = { new({ value = "" } = {}) { let text = value; const on = {}; const s = { value: () => text, set_value: (v) => { text = v; }, on(ev, fn) { on[ev] = fn; }, fire(ev) { on[ev]?.({}, context()); }, release() { return true; } }; return s; } };
const contextVM = vm.createContext({ console, Map, Set, JSON, window: { viewport_size: () => ({ width: 1400, height: 900 }) } });
function mock(name, exports) { return new vm.SyntheticModule(Object.keys(exports), function () { for (const [k, v] of Object.entries(exports)) this.setExport(k, v); }, { context: contextVM, identifier: name }); }
const kit = mock("gpui-kit", { div: element, svg: element, View });
const base = mock("gpui-base", { InputState, CalendarState: {}, Popup: { new: element } });
const components = mock("gpui-component", { Spinner: { new: element } });
const uiExports = { tokens: () => new Proxy({}, { get: () => "#000" }), SPACE: [0, 4, 8, 12, 16, 20, 24], RADIUS: {}, SIZE: { validationLine: 18 }, BREAK: { formNav: 760 } };
for (const n of ["Text", "Line", "Code", "Icon", "AppButton", "IconButton", "Avatar", "Chip", "Switch", "Checkbox", "TextField", "Dropdown", "PageHeader", "MetaLine"]) uiExports[n] = element;
uiExports.AppButton = (_cx, id) => { buttons.push(id); return element(); };
uiExports.Text = (_cx, _variant, text) => { texts.push(String(text)); return element(); };
const store = mock("./store.js", { formatTin: (x) => x, errorText: (e) => String(e && e.message ? e.message : e) });
const periods = mock("./periods.js", { periodLabel: () => "Q3 2026" });
const mod = new vm.SourceTextModule(fs.readFileSync(new URL("app/shell/form_page.js", root), "utf8"), { context: contextVM });
await mod.link((s) => ({ "gpui-kit": kit, "gpui-base": base, "gpui-component": components, "./ui.js": mock("./ui.js", uiExports), "./store.js": store, "./periods.js": periods })[s]);
await mod.evaluate();

// What the host sends: period boxes as the draft holds them (the sample's
// own `blank` list says which to show blank), profile boxes filled except ZIP.
const PROFILE_REASON = "comes from the taxpayer profile; change it in the profile";
let hostValues = {
  [K("txtYear")]: "2026", [K("rtnMonth")]: "12", [K("qtr_1")]: "true",
  [K("txtRDOCode")]: "039", [K("registeredName")]: "EXAMPLE TRADING CORP",
  [K("registeredAddress")]: "1 EXAMPLE STREET", [K("zipCode")]: "", [K("telNo")]: "0281234567",
};
const filled = [K("txtRDOCode"), K("registeredName"), K("registeredAddress"), K("telNo")];
const opened = (sample) => ({
  draft: sample ? {} : { id: 7 }, status: "Draft", editable: true,
  values: sample ? { ...hostValues, [K("txtYear")]: "", [K("rtnMonth")]: "", [K("qtr_1")]: "" } : { ...hostValues },
  options: {}, locks: Object.fromEntries(filled.map((k) => [k, PROFILE_REASON])),
  profile: filled, profile_edit: filled, sample,
  blank: sample ? [K("txtYear"), K("rtnMonth"), K("qtr_1")] : [],
  errors: [],
});
let validateErrors = [];
const app = {
  prefs: {}, branchFor: () => null, taxpayers: () => [], layout: () => ({ column: 1056 }),
  invalidate: () => { rootDirty = true; }, toast(text) { toasts.push(text); rootDirty = true; },
  store: {
    templates: () => [],
    write(name, ...args) { calls.push([name, ...args]); if (name === "screen_form_save") return { id: 7, status: "Draft", draft: args[1] }; return 1; },
    call(name, ...args) {
      calls.push([name, ...args]);
      if (name === "screen_form_schema") return schema;
      if (name === "screen_form_sample") return Promise.resolve(opened(true));
      if (name === "screen_form_open") return Promise.resolve(opened(false));
      if (name === "screen_form_fill") { hostValues = { ...hostValues, ...args[2] }; return { draft: args[1], values: { ...hostValues }, options: {}, rejected: {} }; }
      if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: { ...hostValues }, options: {}, errors: validateErrors });
      if (name === "screen_form_preview_sync" || name === "screen_form_print") return true;
      throw Error(name);
    },
  },
};
app.formUi = new mod.namespace.FormPageActions(app);
const cx = context();
async function settle() { while (tasks.length) await Promise.all(tasks.splice(0)); }
// One window frame: the page and the retained rows it attaches (a row it
// leaves out, folded, is not drawn).
function frame() { rootDirty = false; owner = null; texts.length = 0; buttons.length = 0; mod.namespace.renderFormPage(cx, app, 1056); }
function press(map, id, arg = {}) { const h = map.get(id); assert.ok(h, `handler for ${id}`); const old = owner; owner = h.entity; h.fn(arg, context(h.entity)); owner = old; }
const ui = app.formUi;

// ---- Rule 1: a Form library sample.
ui.open({ code: "2551Q", tin: "12345678800000", year: 2026, period: 0, sample: true }, cx);
await settle();
assert.equal(calls.filter((c) => c[0] === "screen_form_open").length, 0, "a sample never opens a stored return");
assert.ok(calls.some((c) => c[0] === "screen_form_sample"), "a sample opens through screen_form_sample");
rootDirty = true; frame();
assert.equal(ui.state.values[K("txtYear")], "", "year blank");
assert.equal(ui.state.values[K("qtr_1")], "", "quarter blank");
assert.ok(!buttons.includes("fp-save") && !buttons.includes("fp-queue") && !buttons.includes("fp-print"), `no Save/Queue/Print on a sample: ${buttons}`);
assert.ok(texts.some((t) => /not saved.*Forms tab/.test(t)), "the sample says how to start a return");
assert.ok(texts.includes("Sample from the Form library"), "no period in the taxpayer bar");

// The host's refill (the draft still holds the year it was built for) stays
// blank, and a check on it is not shown until the filer types a year.
validateErrors = [{ field: "year", keys: [K("txtYear")], message: "Future filing is not allowed" }];
ui.choose({ [K("amendedRtn_2")]: "true" }, cx);
await settle(); frame();
assert.equal(ui.state.values[K("txtYear")], "", "year stays blank after a fill");
assert.equal(ui.state.errors.length, 0, "the blank year is not judged");
ui.edit(K("txtYear"), "2025", cx); ui.flush(cx);
await settle(); frame();
assert.equal(ui.state.values[K("txtYear")], "2025", "a typed year shows");
assert.equal(ui.state.errors.length, 1, "a typed year is judged");

// Save, Queue and Print never write a stored return from a sample.
const writes = () => calls.filter((c) => ["screen_form_save", "screen_form_queue", "queue_form_draft", "screen_form_print"].includes(c[0])).length;
ui.save(cx); ui.queue(cx); ui.print(cx);
assert.equal(writes(), 0, "a sample is never saved, queued or printed");
assert.ok(toasts.at(-1).includes("Forms tab"), "the refusal says how to start a return");

// ---- Rule 2: a return opened for the taxpayer.
validateErrors = [];
ui.open({ code: "2551Q", tin: "12345678800000", year: 2026, period: 3 }, cx);
await settle();
ui.jumpTo(BG, cx);
traces.length = 0; rootDirty = true; frame();
assert.ok(buttons.includes("fp-save"), "a return keeps Save");
const tucked = ui.tuckedItems(BG);
const rdo = itemOf(K("txtRDOCode")), name = itemOf(K("registeredName")), tel = itemOf(K("telNo")), addr = itemOf(K("zipCode"));
assert.deepEqual([...tucked], [rdo, name, tel], "filled profile items fold; the address item has a blank ZIP");
assert.ok(texts.includes("From tax profile · 3 filled boxes"), `fold label: ${texts.filter((t) => t.startsWith("From"))}`);
assert.ok(traces.includes(`fp-item-${BG}:${addr}`), "the item with the blank profile ZIP stays in paper order");
assert.ok(!traces.includes(`fp-item-${BG}:${tel}`), "folded items are not drawn");
assert.equal(ui.isLocked(K("zipCode")), false, "a blank profile box is not locked");
assert.equal(ui.isLocked(K("telNo")), true, "a filled profile box is locked until Edit");

press(clicks, `fp-profile-${BG}`);
traces.length = 0; frame();
assert.ok(traces.includes(`fp-item-${BG}:${tel}`), "the unfolded group draws its items");

// Edit by click: the box opens, the edit reaches the host as `unlocked`.
press(clicks, `fp-edit-${BG}:${tel}`);
clicks.delete(`fp-edit-${BG}:${tel}`);
frame();
assert.equal(ui.isLocked(K("telNo")), false, "Edit opens the box");
assert.ok(!clicks.has(`fp-edit-${BG}:${tel}`), "Edit is gone while typing");
ui.edit(K("telNo"), "0287654321", cx); ui.flush(cx);
const fill = calls.filter((c) => c[0] === "screen_form_fill").at(-1);
assert.equal(JSON.stringify(fill[3]), JSON.stringify({ [K("telNo")]: "0287654321" }));
assert.equal(JSON.stringify(fill[4]), JSON.stringify([K("telNo")]), "the opened profile box is sent as unlocked");
assert.equal(ui.state.dirty, true, "the return now has unsaved changes");
// Clicking away: back to the profile look, the value kept on this return.
press(downOut, `fp-item-${BG}:${tel}`);
assert.equal(ui.isLocked(K("telNo")), true, "clicking away closes the box");
assert.equal(ui.state.values[K("telNo")], "0287654321", "the edit stays on this return");
assert.ok(!calls.some((c) => /profile/.test(c[0]) && c[0] !== "screen_form_fill"), "nothing writes the profile");

// Keyboard: Tab lands on Edit (a tab stop), Enter opens the box; blur closes it.
await settle(); frame();
press(keys, `fp-edit-${BG}:${rdo}`, { key: "enter" });
assert.equal(ui.isLocked(K("txtRDOCode")), false, "Enter on Edit opens the box");
ui.endProfileEdit([K("txtRDOCode")]);
press(keys, `fp-edit-${BG}:${addr}`, { key: "enter" });
assert.equal(ui.isLocked(K("registeredAddress")), false, "Edit on an item left in paper order");
ui.state.inputs.get(K("registeredAddress")).fire("blur");
assert.equal(ui.isLocked(K("registeredAddress")), true, "blur gives the profile look back");
// A box the profile left blank is typed like any other and sent open.
ui.edit(K("zipCode"), "1100", cx); ui.flush(cx);
assert.equal(JSON.stringify(calls.filter((c) => c[0] === "screen_form_fill").at(-1)[4]), JSON.stringify([K("zipCode")]));
// A box locked for any other reason is never sent as unlocked.
ui.state.locks[K("txtSheets")] = "computed by BIR's own formulas";
assert.equal(ui.unlockedFor({ [K("txtSheets")]: "1" }).length, 0);
console.log("PASS: library sample blank and unsaved; profile boxes fold, open with Edit or Enter, keep the edit on this return.");
