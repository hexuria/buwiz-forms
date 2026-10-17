// Form page rules 1 and 2 (docs/form-findings rules page), driven through the
// real app/shell/form_page.js with a mocked host:
//  - a Form library sample shows its period and date boxes blank, keeps one
//    blank until the host accepts an edit for it, does not judge a blank
//    one, offers Start a return, and is never saved, queued or printed;
//  - profile-filled items fold into "From tax profile · N filled boxes"; a
//    box the profile left blank stays in paper order; the profile mark turns
//    into Edit on hover or keyboard focus; Edit (click or Enter) opens the
//    item's profile boxes and keeps the keyboard; only boxes opened with Edit
//    reach screen_form_fill as `unlocked`; moving between boxes of one item
//    keeps it open, and leaving the item gives it its profile look back with
//    the value kept on this (now unsaved) return.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { kitMethods } from "./kit_surface.mjs";
const root = new URL("../", import.meta.url);
// Mock elements answer only the methods the real kit declares.
const KIT = kitMethods(root);
const schema = JSON.parse(fs.readFileSync(new URL("app/forms/forms-schema.json", root))).find((f) => f.c === "2551Q");
const K = (tail) => `frm2551Qv2018:${tail}`;
const partOf = (key) => schema.parts.findIndex((p) => p.items.some((it) => (it.f || []).some((f) => f.k === key)));
const itemOf = (key) => schema.parts[partOf(key)].items.findIndex((it) => (it.f || []).some((f) => f.k === key));
const BG = partOf(K("telNo"));
assert.ok(BG >= 0, "2551Q has a telNo box");

const traces = [], entities = [], tasks = [], texts = [], buttons = [], toasts = [], timers = [];
const clicks = new Map(), keys = new Map(), downOut = new Map(), hovers = new Map(), opacity = new Map(), focusStyled = new Set(), focusTracked = new Set();
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
    timer: { after(_ms, fn) { timers.push(fn); return { cancel() {} }; }, every() { return { cancel() {} }; } },
    focus_handle() { return { focus() {}, is_focused: () => false, release() { return true; } }; },
    stop_propagation() {},
    theme() { return { appearance: "light", is_dark: false, colors: {} }; },
  };
}
function runTimers() { while (timers.length) timers.shift()(context()); }
function element() {
  const creationOwner = owner;
  const target = {};
  const el = new Proxy(target, { get(o, k) {
    if (typeof k !== "string" || !KIT.has(k)) return undefined;
    return (...args) => {
      if (k === "id") { o.id = args[0]; if (String(o.id).startsWith("fp-item-")) traces.push(o.id); }
      if (k === "on_click") clicks.set(o.id, { fn: args[0], entity: creationOwner });
      if (k === "on_key_down") keys.set(o.id, { fn: args[0], entity: creationOwner });
      if (k === "on_mouse_down_out") downOut.set(o.id, { fn: args[0], entity: creationOwner });
      if (k === "on_hover") hovers.set(o.id, { fn: args[0], entity: creationOwner });
      if (k === "opacity") opacity.set(o.id, args[0]);
      if (k === "focus") focusStyled.add(o.id);
      if (k === "track_focus") focusTracked.add(o.id);
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
const InputState = { new({ value = "" } = {}) { let text = value; const on = {}; return { value: () => text, set_value: (v) => { text = v; }, on(ev, fn) { on[ev] = fn; }, fire(ev) { on[ev]?.({}, context()); }, release() { return true; } }; } };
const contextVM = vm.createContext({ console, Map, Set, JSON, window: { viewport_size: () => ({ width: 1400, height: 900 }) } });
function mock(name, exports) { return new vm.SyntheticModule(Object.keys(exports), function () { for (const [k, v] of Object.entries(exports)) this.setExport(k, v); }, { context: contextVM, identifier: name }); }
const kit = mock("gpui-kit", { div: element, svg: element, View });
const base = mock("gpui-base", { InputState, CalendarState: {}, Popup: { new: element } });
const components = mock("gpui-component", { Spinner: { new: element } });
const uiExports = { tokens: () => new Proxy({}, { get: () => "#000" }), SPACE: [0, 4, 8, 12, 16, 20, 24], RADIUS: {}, SIZE: { validationLine: 18 }, BREAK: { formNav: 760 } };
for (const n of ["Text", "Line", "Code", "Icon", "AppButton", "IconButton", "Avatar", "Chip", "Switch", "Checkbox", "TextField", "Dropdown", "PageHeader", "MetaLine", "Overlay", "CheckboxLabel"]) uiExports[n] = element;
uiExports.AppButton = (_cx, id, _label, opts = {}) => { buttons.push(id); const el = element(); el.id(id); if (opts.onClick) el.on_click(opts.onClick); return el; };
uiExports.Text = (_cx, _variant, text) => { texts.push(String(text)); return element(); };
uiExports.Line = uiExports.Text;
const store = mock("./store.js", { formatTin: (x) => x, errorText: (e) => String(e && e.message ? e.message : e) });
const periods = mock("./periods.js", { periodLabel: () => "Q3 2026" });
const mod = new vm.SourceTextModule(fs.readFileSync(new URL("app/shell/form_page.js", root), "utf8"), { context: contextVM });
await mod.link((s) => ({ "gpui-kit": kit, "gpui-base": base, "gpui-component": components, "./ui.js": mock("./ui.js", uiExports), "./store.js": store, "./periods.js": periods })[s]);
await mod.evaluate();

// What the host sends: period boxes as the draft holds them (the sample's
// own `blank` list says which it shows blank), profile boxes filled except
// ZIP. The host refuses a month of 13.
const PROFILE_REASON = "comes from the taxpayer profile; change it in the profile";
const IDENTITY_REASON = "picks which return this is; reopen the form for another TIN or period";
let hostValues = {
  [K("txtYear")]: "2027", [K("rtnMonth")]: "12", [K("qtr_1")]: "true",
  [K("txtRDOCode")]: "039", [K("registeredName")]: "EXAMPLE TRADING CORP",
  [K("registeredAddress")]: "1 EXAMPLE STREET", [K("zipCode")]: "", [K("telNo")]: "0281234567",
};
const filled = [K("txtRDOCode"), K("registeredName"), K("registeredAddress"), K("telNo")];
const blankKeys = [K("txtYear"), K("rtnMonth"), K("qtr_1")];
const opened = (sample) => ({
  draft: sample ? { screen_form_sample: { blank: blankKeys, open: [] } } : { id: 7 }, status: "Draft", editable: true,
  values: sample ? { ...hostValues, ...Object.fromEntries(blankKeys.map((k) => [k, ""])) } : { ...hostValues },
  options: {},
  locks: {
    ...Object.fromEntries(filled.map((k) => [k, PROFILE_REASON])),
    [K("txtYear")]: IDENTITY_REASON,
    // A radio group a sample keeps shut, with its reason (item 1, "For the").
    ...(sample ? { [K("forThe_1")]: IDENTITY_REASON, [K("forThe_2")]: IDENTITY_REASON } : {}),
  },
  profile: filled, profile_edit: filled, sample,
  blank: sample ? [...blankKeys, K("forThe_1"), K("forThe_2")] : [],
  // The host's reason for a blank box it keeps locked (the year, and the
  // "For the" radios).
  needs: sample ? { [K("txtYear")]: "Shows once the period is typed", [K("forThe_1")]: "Set when you start a return" } : {},
  errors: [],
});
let validateErrors = [];
const app = {
  prefs: {}, branchFor: () => null, taxpayers: () => [], layout: () => ({ column: 1056 }),
  invalidate: () => { rootDirty = true; }, toast(text) { toasts.push(text); rootDirty = true; },
  startNewReturn(_cx, code) { calls.push(["startNewReturn", code]); },
  store: {
    templates: () => [],
    write(name, ...args) { calls.push([name, ...args]); if (name === "screen_form_save") return { id: 7, status: "Draft", draft: args[1] }; return 1; },
    call(name, ...args) {
      calls.push([name, ...args]);
      if (name === "screen_form_schema") return schema;
      if (name === "screen_form_sample") return Promise.resolve(opened(true));
      if (name === "screen_form_open") return Promise.resolve(opened(false));
      if (name === "screen_form_fill") {
        const rejected = {};
        const edits = { ...args[2] };
        for (const [k, v] of Object.entries(edits)) if (v === "13") { rejected[k] = "must be a month (got 13)"; delete edits[k]; }
        hostValues = { ...hostValues, ...edits };
        return { draft: args[1], values: { ...hostValues }, options: {}, rejected };
      }
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
function frame() { rootDirty = false; owner = null; texts.length = 0; buttons.length = 0; opacity.clear(); mod.namespace.renderFormPage(cx, app, 1056); }
function press(map, id, arg = {}) { const h = map.get(id); assert.ok(h, `handler for ${id}`); const old = owner; owner = h.entity; h.fn(arg, context(h.entity)); owner = old; }
const json = (v) => JSON.stringify(v);
const ui = app.formUi;

// ---- Rule 1: a Form library sample.
ui.open({ code: "2551Q", tin: "12345678800000", year: 2026, period: 0, sample: true }, cx);
await settle();
assert.equal(calls.filter((c) => c[0] === "screen_form_open").length, 0, "a sample never opens a stored return");
assert.equal(json(calls.find((c) => c[0] === "screen_form_sample")), json(["screen_form_sample", "12345678800000", "2551Q"]));
frame();
assert.equal(ui.state.values[K("txtYear")], "", "year blank");
assert.equal(ui.state.values[K("qtr_1")], "", "quarter blank");
assert.ok(!buttons.includes("fp-save") && !buttons.includes("fp-queue") && !buttons.includes("fp-print"), `no Save/Queue/Print on a sample: ${buttons}`);
assert.ok(texts.some((t) => /not saved.*Start a return.*New return/.test(t)), "the sample says how to start a return");
assert.ok(texts.includes("Sample from the Form library"), "no period in the taxpayer bar");
assert.ok(texts.includes("Shows once the period is typed"), "a locked blank box says what it waits for");
assert.ok(texts.includes("Set when you start a return"), "a shut radio group says what it waits for");
press(clicks, "fp-start-return");
assert.equal(json(calls.at(-1)), json(["startNewReturn", "2551Q"]), "Start a return opens New return for this form");

// The host's refill stays blank, and a check naming only blank boxes waits.
// The host holds back what the blank period cannot judge; the page shows
// exactly the messages it is given.
validateErrors = [
  { field: "year and sales", keys: [K("txtYear"), K("txtTax14")], message: "Sales need a year" },
];
ui.choose({ [K("amendedRtn_2")]: "true" }, cx);
await settle(); frame();
assert.equal(ui.state.values[K("txtYear")], "", "year stays blank after a fill");
assert.equal(json(ui.state.errors.map((e) => e.field)), json(["year and sales"]), "the host's messages show as given");
// An edit the host refuses keeps the box blank.
ui.choose({ [K("rtnMonth")]: "13" }, cx);
await settle(); frame();
assert.equal(ui.state.values[K("rtnMonth")], "", "a refused edit leaves the box blank");
// An edit it takes shows.
ui.choose({ [K("rtnMonth")]: "06" }, cx);
await settle(); frame();
assert.equal(ui.state.values[K("rtnMonth")], "06", "an accepted edit shows");

// Save, Queue and Print never write a stored return from a sample.
const writes = () => calls.filter((c) => ["screen_form_save", "screen_form_queue", "queue_form_draft", "screen_form_print", "cancel_queued", "screen_form_cancel_queue"].includes(c[0])).length;
ui.save(cx); ui.queue(cx); ui.print(cx); ui.cancelQueue(cx);
assert.equal(writes(), 0, "a sample is never saved, queued or printed");
assert.ok(toasts.at(-1).includes("New return"), "the refusal says how to start a return");

// ---- Rule 2: a return opened for the taxpayer.
validateErrors = [];
ui.open({ code: "2551Q", tin: "12345678800000", year: 2026, period: 3 }, cx);
await settle();
ui.jumpTo(BG, cx);
traces.length = 0; frame();
assert.ok(buttons.includes("fp-save"), "a return keeps Save");
const tucked = ui.tuckedItems(BG);
const rdo = itemOf(K("txtRDOCode")), name = itemOf(K("registeredName")), tel = itemOf(K("telNo")), addr = itemOf(K("zipCode"));
assert.deepEqual([...tucked], [rdo, name, tel], "filled profile items fold; the address item has a blank ZIP");
assert.ok(texts.includes("From tax profile · 3 filled boxes"), `fold label: ${texts.filter((t) => t.startsWith("From"))}`);
assert.ok(traces.includes(`fp-item-${BG}:${addr}`), "the item with the blank profile ZIP stays in paper order");
assert.ok(!traces.includes(`fp-item-${BG}:${tel}`), "folded items are not drawn");
assert.equal(ui.isLocked(K("zipCode")), false, "a blank profile box is not locked");
assert.equal(ui.isLocked(K("telNo")), true, "a filled profile box is locked until Edit");

// At rest the mark shows, not Edit; Edit shows on hover and on keyboard focus.
const ADDR = `fp-edit-${BG}:${addr}`;
assert.equal(opacity.get(ADDR), 0, "Edit is hidden at rest");
assert.ok(focusStyled.has(ADDR) && focusTracked.has(ADDR), "Edit is a tab stop that shows when focused");
press(hovers, `fp-item-${BG}:${addr}`, true);
frame();
assert.equal(opacity.get(ADDR), 1, "hovering the item shows Edit");
press(hovers, `fp-item-${BG}:${addr}`, false);
frame();
assert.equal(opacity.get(ADDR), 0, "Edit hides when the pointer leaves");

// The fold is a tab stop: Enter unfolds it, as a click does.
assert.ok(focusTracked.has(`fp-profile-${BG}`) && focusStyled.has(`fp-profile-${BG}`), "the fold takes the keyboard and shows it");
press(keys, `fp-profile-${BG}`, { key: "enter" });
assert.ok(ui.state.profileOpen.has(BG), "Enter unfolds");
press(clicks, `fp-profile-${BG}`);
assert.ok(!ui.state.profileOpen.has(BG), "a click folds it again");
press(clicks, `fp-profile-${BG}`);
traces.length = 0; frame();
assert.ok(traces.includes(`fp-item-${BG}:${tel}`), "the unfolded group draws its items");

// Before Edit nothing profile-locked is sent as unlocked.
assert.equal(ui.unlockedFor({ [K("txtRDOCode")]: "040" }).length, 0, "a profile box not opened is not unlocked");
// Edit by click: the box opens and the edit reaches the host as `unlocked`.
press(clicks, `fp-edit-${BG}:${tel}`);
clicks.delete(`fp-edit-${BG}:${tel}`);
frame();
assert.equal(ui.isLocked(K("telNo")), false, "Edit opens the box");
assert.ok(!clicks.has(`fp-edit-${BG}:${tel}`), "Edit is gone while typing");
assert.equal(opacity.get(`fp-edit-${BG}:${tel}`), 0, "its place keeps the keyboard, hidden");
ui.edit(K("telNo"), "0287654321", cx); ui.flush(cx);
const fill = calls.filter((c) => c[0] === "screen_form_fill").at(-1);
assert.equal(json(fill[3]), json({ [K("telNo")]: "0287654321" }));
assert.equal(json(fill[4]), json([K("telNo")]), "the opened profile box is sent as unlocked");
assert.equal(ui.state.dirty, true, "the return now has unsaved changes");
// Clicking away: back to the profile look, the value kept on this return.
press(downOut, `fp-item-${BG}:${tel}`);
assert.equal(ui.isLocked(K("telNo")), true, "clicking away closes the box");
assert.equal(ui.state.values[K("telNo")], "0287654321", "the edit stays on this return");
assert.ok(!calls.some((c) => /profile/.test(c[0])), "nothing writes the profile");

// Keyboard: Enter on the focused Edit opens the item's boxes.
await settle(); frame();
frame();
press(keys, ADDR, { key: "enter" });
assert.equal(ui.isLocked(K("registeredAddress")), false, "Enter on Edit opens the box");
frame();
assert.ok(focusTracked.has(ADDR) && opacity.get(ADDR) === 0, "the focused button stays in place");
// Moving between the boxes of one item keeps it open; leaving it closes it.
const address = ui.state.inputs.get(K("registeredAddress")), zip = ui.state.inputs.get(K("zipCode"));
address.fire("focus"); address.fire("blur"); zip.fire("focus");
runTimers();
assert.equal(ui.isLocked(K("registeredAddress")), false, "tabbing to the ZIP keeps the address open");
zip.fire("blur");
runTimers();
assert.equal(ui.isLocked(K("registeredAddress")), true, "leaving the item gives the profile look back");
// A box the profile left blank is typed like any other and sent open.
ui.edit(K("zipCode"), "1100", cx); ui.flush(cx);
assert.equal(json(calls.filter((c) => c[0] === "screen_form_fill").at(-1)[4]), json([K("zipCode")]));
// Focus landing in another item's box closes an item opened with Edit (a
// select or radio item has no box of its own to blur).
press(keys, `fp-edit-${BG}:${rdo}`, { key: "enter" });
assert.equal(ui.isLocked(K("txtRDOCode")), false, "the RDO select is open");
zip.fire("focus");
assert.equal(ui.isLocked(K("txtRDOCode")), true, "focus in the address item closes the RDO item");
// Edit on one item closes another still open.
press(keys, `fp-edit-${BG}:${rdo}`, { key: "enter" });
frame();
press(keys, ADDR, { key: "enter" });
assert.equal(ui.isLocked(K("txtRDOCode")), true, "opening the address closes the RDO");
assert.equal(ui.isLocked(K("registeredAddress")), false);
ui.endProfileEdit([K("registeredAddress")]);
// A box locked for any other reason is never sent as unlocked.
ui.state.locks[K("txtSheets")] = "computed by BIR's own formulas";
assert.equal(ui.unlockedFor({ [K("txtSheets")]: "1" }).length, 0);
console.log("PASS: library sample blank and unsaved; profile boxes fold, show Edit on hover or focus, keep the edit on this return.");

// ---- A stored draft that followed a later profile change says what moved.
{
  const plain = app.store.call;
  app.store.call = (name, ...args) =>
    name === "screen_form_open" ? Promise.resolve({ ...opened(false), profile_updates: ["RDO 050 → 043", "ZIP 1100 → 1200"] }) : plain.call(app.store, name, ...args);
  ui.open({ code: "2551Q", tin: "12345678800000", year: 2026, period: 3 }, cx);
  await settle(); frame();
  assert.ok(texts.includes("Updated from your tax profile: RDO 050 → 043; ZIP 1100 → 1200"), "the note lists what changed");
  app.store.call = plain;
  ui.open({ code: "2551Q", tin: "12345678800000", year: 2026, period: 3 }, cx);
  await settle(); frame();
  assert.ok(!texts.some((t) => t.startsWith("Updated from your tax profile")), "nothing moved: no note");
  console.log("PASS: a draft that followed the tax profile says what changed at the top.");
}

// ---- A sample's period (option B): page inputs, a host answer once complete.
// The page asks about a period a moment after its last change (timers).
async function settleAll() { for (;;) { runTimers(); if (!tasks.length) break; await settle(); } }
const quarterKeys = [1, 2, 3, 4].map((n) => K(`qtr_${n}`));
const periodParts = [
  { key: K("txtYear"), part: "year", quarter: null },
  ...quarterKeys.map((key, i) => ({ key, part: "quarter", quarter: i + 1 })),
];
const baseCall = app.store.call;
const asks = [];
app.store.call = (name, ...args) => {
  if (name === "screen_form_sample") {
    calls.push([name, ...args]);
    return Promise.resolve({ ...opened(true), period_parts: periodParts });
  }
  if (name === "screen_form_sample_period") {
    const parts = args[2];
    asks.push(JSON.parse(JSON.stringify(parts)));
    const quarter = quarterKeys.findIndex((k) => parts[k] === "true") + 1;
    if (parts[K("txtYear")] !== "2024" || !quarter) return Promise.resolve({ complete: false });
    if (quarter === 1) {
      return Promise.resolve({
        complete: true,
        opens: { year: 2024, period: 1 },
        existing: { form_code: "2551Q", year: 2024, period: 1, label: "Q1 2024", status: "Draft", updated_at: "2026-10-01", total_payable: 0 },
      });
    }
    return Promise.resolve({
      complete: true,
      opens: { year: 2024, period: quarter },
      sample: {
        ...opened(true),
        draft: { screen_form_sample: { blank: [] }, rebuilt: quarter },
        values: { ...hostValues, [K("txtYear")]: "2024", [quarterKeys[quarter - 1]]: "true" },
        locks: Object.fromEntries(filled.map((k) => [k, PROFILE_REASON])),
        blank: [],
        needs: {},
        period_parts: periodParts,
        period_set: true,
      },
    });
  }
  return baseCall.call(app.store, name, ...args);
};
const opens = [];
app.openForm = (opts) => opens.push(opts);
ui.open({ code: "2551Q", tin: "12345678800000", year: 2026, period: 0, sample: true }, cx);
await settleAll();
const fills = () => calls.filter((c) => c[0] === "screen_form_fill");
const fillsBefore = fills().length;
// The year alone: a page input, no draft write, not complete.
ui.edit(K("txtYear"), "2024", cx);
await settleAll();
assert.equal(fills().length, fillsBefore, "a period part is never a draft write");
assert.equal(ui.state.values[K("txtYear")], "2024", "the typed year shows");
assert.equal(asks.at(-1)[K("txtYear")], "2024", "the host is asked with the parts");
assert.equal(ui.state.opens, null, "not complete");
// An amount typed meanwhile is kept for the rebuild.
ui.choose({ [K("amendedRtn_2")]: "true" }, cx);
await settleAll();
assert.equal(ui.state.typedEdits[K("amendedRtn_2")], "true");
// Q1 has a stored return: offered, never opened, the sample unchanged.
const draftBefore = ui.state.draft;
ui.choose(Object.fromEntries(quarterKeys.map((k, i) => [k, i === 0 ? "true" : "false"])), cx);
await settleAll();
assert.equal(ui.state.draft, draftBefore, "an existing return changes nothing by itself");
assert.equal(ui.state.existing.label, "Q1 2024", "offerExisting holds what the chosen UI needs");
assert.equal(opens.length, 0);
ui.startReturn(cx);
assert.equal(opens.length, 0, "Start a return offers the stored return, it does not open it");
// Q2 has none: the sample is rebuilt for it and the typed answer goes on again.
const before = fills().length;
ui.choose(Object.fromEntries(quarterKeys.map((k, i) => [k, i === 1 ? "true" : "false"])), cx);
await settleAll();
assert.equal(ui.state.draft.rebuilt ?? fills().at(-1)[2].rebuilt, 2, "the rebuilt draft");
assert.equal(ui.state.existing, null);
const reapplied = fills().slice(before).find((c) => c[2] && c[2].rebuilt === 2);
assert.ok(reapplied, "the typed values were filled onto the rebuilt draft");
assert.equal(reapplied[3][K("amendedRtn_2")], "true");
assert.equal(ui.state.periodSet, true);
ui.startReturn(cx);
assert.equal(JSON.stringify(opens.at(-1)), JSON.stringify({ tin: "12345678800000", code: "2551Q", year: 2024, period: 2 }), "Start a return opens that period's return");
console.log("PASS: a sample's period parts are page inputs; complete, the host offers a stored return or rebuilds the sample.");
