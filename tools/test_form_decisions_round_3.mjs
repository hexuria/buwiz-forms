// PR 13 (maintainer decisions, page 4): what the page does for these
// decisions, driven through the real app/shell/form_page.js with a mocked
// host and a small form shaped like the boxes they touch:
//  - P4-1: a money box (1800's item 17A) typed 8816764.47 shows
//    8,816,764.47 after blur, after Check and after Save, as the host holds
//    it.
//  - P4-14: a kept penalty group (an older 2200 return) shows "Kept as
//    entered before penalties were computed." under its boxes while they
//    hold 0.00, because the host lists them in `notes_always`; Edit
//    penalties and the one-click "Use the computed amounts" still work, and
//    once the host computes the amounts the note goes. A locked box showing
//    a value that the host does not list draws no note, as before.
//  - P4-17: a locked box the host lists (1702RT item 42) says why under it
//    while it shows an amount.
//  - P4-10: an item with a `from` hint (1701 Part VIII item 6) shows what
//    the other item holds and "Use it", which fills the box through the
//    ordinary fill; nothing is filled by itself.
// Which boxes are listed, and the hint, are the host's to say (crates/
// bir-shell screen_form_module tests: excise, annual_corporate,
// annual_individual and donor_estate).
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);

const K = (tail) => `frmTEST:${tail}`;
const NOTE_KEPT = "Kept as entered before penalties were computed.";
const NOTE_42 = "Computed for this year: MCIT before 2020 is 2% of gross income (item 33).";
const NOTE_PROFILE = "Comes from the tax profile.";
const PENALTIES = ["Surcharge", "Interest", "Compromise"].map(K);
const schema = {
  c: "TEST", t: "Test return", rev: "2018", freq: "As needed", pages: 1, n: 8, q: 1,
  parts: [
    {
      n: "Part I · Test",
      items: [
        { no: "17A", l: "Payments for Prior Gifts", f: [{ k: K("TaxCredit17A"), t: "money", l: "17A" }] },
        { no: "19", l: "Penalties", f: PENALTIES.map((k, i) => ({ k, t: "money", l: ["Surcharge", "Interest", "Compromise"][i] })) },
        { no: "42", l: "MCIT Due (2% of Item 33)", f: [{ k: K("Mcit42"), t: "money", l: "MCIT", w: true }] },
        { no: "7", l: "Registered name", f: [{ k: K("Name"), t: "text", l: "Name" }] },
        { no: "8", l: "Special Tax Credits (To Part VIII Item 6)", f: [{ k: K("Part7_8A"), t: "money", l: "A" }] },
        {
          no: "6", l: "Add: Special Tax Credit, if any (From Part VII Item 8)",
          from: { k: K("Part7_8A"), to: K("Part8_6A"), l: "From Part VII item 8" },
          f: [{ k: K("Part8_6A"), t: "money", l: "A" }],
        },
      ],
    },
  ],
};

const entities = [], tasks = [], texts = [], timers = [];
const clicks = new Map(), downOut = new Map(), opacity = new Map(), ids = new Set();
let calendarSteps = 0;
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
function runTimers() { while (timers.length) act(timers.shift()); }
// A host call's context lives only while the handler runs: one used after
// it returns (from a timer, say) throws, as the shell does.
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
// The calendar's grid: a month that the arrows (and the lists) step.
const calendars = [];
const CalendarState = { new() {
  let value = null, y = 2026, m = 10, onChange = null;
  const cal = {
    set_value(v) { value = v; if (typeof v === "string") { y = Number(v.slice(0, 4)); m = Number(v.slice(5, 7)); } },
    value: () => value, year: () => y, month: () => m, today: () => "2026-10-17", month_days: () => [[]],
    next_month() { calendarSteps += 1; m += 1; if (m > 12) { m = 1; y += 1; } },
    prev_month() { calendarSteps += 1; m -= 1; if (m < 1) { m = 12; y -= 1; } },
    on(_ev, fn) { onChange = fn; }, pick(day) { value = day; if (onChange) act(onChange, day); }, release() { return true; },
  };
  calendars.push(cal);
  return cal;
} };
const contextVM = vm.createContext({ console, Map, Set, JSON, Date, Math, Number, String, Object, Array, window: { viewport_size: () => ({ width: 1400, height: 900 }) } });
function mock(name, exports) { return new vm.SyntheticModule(Object.keys(exports), function () { for (const [k, v] of Object.entries(exports)) this.setExport(k, v); }, { context: contextVM, identifier: name }); }
const kit = mock("gpui-kit", { div: element, svg: element, View });
const base = mock("gpui-base", { InputState, CalendarState, Popup: { new: element } });
const components = mock("gpui-component", { Spinner: { new: element } });
const uiExports = { tokens: () => new Proxy({}, { get: () => "#000" }), SPACE: [0, 4, 8, 12, 16, 20, 24], RADIUS: {}, SIZE: { validationLine: 18 }, BREAK: { formNav: 760 } };
for (const n of ["Text", "Line", "Code", "Icon", "AppButton", "IconButton", "Avatar", "Chip", "Switch", "Checkbox", "TextField", "Dropdown", "PageHeader", "MetaLine", "Overlay", "CheckboxLabel"]) uiExports[n] = element;
const dropdowns = new Map();
uiExports.AppButton = (_cx, id, _label, opts = {}) => { const el = element(); el.id(id); if (opts.onClick) el.on_click(opts.onClick); return el; };
uiExports.IconButton = (_cx, id, _icon, opts = {}) => { const el = element(); el.id(id); if (opts.onClick) el.on_click(opts.onClick); return el; };
uiExports.Dropdown = (_cx, id, opts) => { dropdowns.set(id, opts); const el = element(); el.id(id); return el; };
uiExports.Text = (_cx, _variant, text) => { texts.push(String(text)); return element(); };
uiExports.Line = uiExports.Text;
const store = mock("./store.js", { formatTin: (x) => x, errorText: (e) => String(e && e.message ? e.message : e) });
const periods = mock("./periods.js", { periodLabel: () => "October 2026" });
const mod = new vm.SourceTextModule(fs.readFileSync(new URL("app/shell/form_page.js", root), "utf8"), { context: contextVM });
await mod.link((s) => ({ "gpui-kit": kit, "gpui-base": base, "gpui-component": components, "./ui.js": mock("./ui.js", uiExports), "./store.js": store, "./periods.js": periods })[s]);
await mod.evaluate();
const { FormPageActions, renderFormPage } = mod.namespace;

// The host's amount text: "1,234.50" (official_amount); blank stays blank.
const money = (v) => {
  const n = Number(String(v).replace(/,/g, ""));
  if (String(v).trim() === "" || Number.isNaN(n)) return String(v);
  const [i, f] = Math.abs(n).toFixed(2).split(".");
  return `${n < 0 ? "-" : ""}${i.replace(/\B(?=(\d{3})+(?!\d))/g, ",")}.${f}`;
};
const KEPT_LOCK = "kept as entered before penalties were computed; choose Edit penalties to change them.";
const COMPUTED_LOCK = "computed from the due date; choose Edit penalties to enter the amounts BIR agreed to";
let kept = true;
let listNotes = true;
let hostValues = {
  [K("TaxCredit17A")]: "0.00",
  [PENALTIES[0]]: "0.00", [PENALTIES[1]]: "0.00", [PENALTIES[2]]: "0.00",
  [K("Mcit42")]: "59,001",
  [K("Name")]: "EXAMPLE TRADING CORP",
  [K("Part7_8A")]: "1,500.00",
  [K("Part8_6A")]: "0.00",
};
const locks = () => ({
  ...Object.fromEntries(PENALTIES.map((k) => [k, kept ? KEPT_LOCK : COMPUTED_LOCK])),
  [K("Mcit42")]: "For years before 2020, MCIT is 2% of gross income (item 33), which the form computes.",
  [K("Name")]: "comes from the tax profile",
});
const lockNotes = () => ({
  ...Object.fromEntries(PENALTIES.map((k) => [k, kept ? NOTE_KEPT : "Computed by the form."])),
  [K("Mcit42")]: NOTE_42,
  [K("Name")]: NOTE_PROFILE,
});
const group = () => ({ keys: PENALTIES, edited: false, applies: true, kept });
const noted = () => [...(kept ? PENALTIES : []), K("Mcit42")];
const opened = () => ({
  draft: { id: 7 }, status: "Draft", editable: true, values: { ...hostValues }, options: {}, locks: locks(),
  lock_notes: lockNotes(), ...(listNotes ? { notes_always: noted() } : {}),
  profile: [K("Name")], profile_edit: [], sample: false, blank: [], needs: {}, off: {}, penalties: [group()], pools: {}, errors: [],
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
      if (name === "screen_form_fill") {
        const edits = args[2];
        const relocks = {}, relockNotes = {};
        if (PENALTIES.every((k) => edits[k] === "")) {
          // "Use the computed amounts" on a kept group: the host joins the
          // due date and computes (rows_excise.rs join_if_group_blanked).
          kept = false;
          Object.assign(hostValues, { [PENALTIES[0]]: "11,250.00", [PENALTIES[1]]: "1,183.56", [PENALTIES[2]]: "10,000.00" });
          for (const k of PENALTIES) { relocks[k] = COMPUTED_LOCK; relockNotes[k] = "Computed by the form."; }
        }
        for (const [k, v] of Object.entries(edits)) if (!PENALTIES.includes(k)) hostValues[k] = money(v);
        return { draft: args[1], values: { ...hostValues }, options: {}, rejected: {}, penalties: [group()], pools: {}, relocks, relock_notes: relockNotes, notes_always: noted() };
      }
      if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: { ...hostValues }, options: {}, errors: [], off: {}, penalties: [group()], pools: {} });
      if (name === "screen_form_preview_sync" || name === "screen_form_print") return true;
      throw Error(name);
    },
  },
};
app.formUi = new FormPageActions(app);
const ui = app.formUi;
const cx = context();
async function settle() { while (tasks.length) await Promise.all(tasks.splice(0)); }
function frame() { owner = null; texts.length = 0; ids.clear(); opacity.clear(); for (const e of entities) e.dirty = true; renderFormPage(cx, app, 1056); }
const fills = () => calls.filter((c) => c[0] === "screen_form_fill");
const lastFill = () => fills().at(-1);
const item = (no) => ui.state.schema.parts[0].items.find((it) => it.no === no);
const click = (id) => { const c = clicks.get(id); assert.ok(c, `button ${id}`); act(c.fn, {}); };
async function openTest() {
  ui.open({ code: "TEST", tin: "12345678800000", year: 2026, period: 1 }, cx);
  await settle();
  ui.ensureInputs(0, cx);
  ui.state.openParts = new Set([0]);
  frame();
}

await openTest();

// ---- P4-1: a large amount shows as held, typed, checked and saved.
const box17A = ui.state.inputs.get(K("TaxCredit17A"));
box17A.type("8816764.47");
box17A.fire("blur");
await settle();
assert.equal(lastFill()[3][K("TaxCredit17A")], "8816764.47", "the host gets what was typed");
assert.equal(ui.state.values[K("TaxCredit17A")], "8,816,764.47", "the page holds the host's value");
assert.equal(box17A.value(), "8,816,764.47", "after blur the box shows it");
ui.check(cx);
await settle();
assert.equal(box17A.value(), "8,816,764.47", "after Check");
ui.save(cx);
await settle();
assert.equal(box17A.value(), "8,816,764.47", "after Save");
console.log("PASS P4-1: 1800 item 17A shows 8,816,764.47 typed, checked and saved.");

// ---- P4-14: a kept group says so over its 0.00; a listed locked box says
// why over its value; any other locked box with a value stays quiet.
assert.equal(JSON.stringify(ui.lockNotesFor(item("19"))), JSON.stringify([NOTE_KEPT]), "the kept note over 0.00");
assert.equal(JSON.stringify(ui.lockNotesFor(item("42"))), JSON.stringify([NOTE_42]), "item 42 says why over its amount");
assert.equal(JSON.stringify(ui.lockNotesFor(item("7"))), "[]", "a profile box showing its value draws no note");
frame();
assert.ok(ids.has("fp-penalty-edit-0:1"), "Edit penalties is offered");
assert.ok(texts.includes(NOTE_KEPT), "the kept note is drawn");
assert.ok(texts.includes(NOTE_42), "item 42's note is drawn");
// Without the host's list the page keeps its old rule (notes only under a
// blank box), so no other form's notes change.
listNotes = false;
await openTest();
assert.equal(JSON.stringify(ui.lockNotesFor(item("19"))), "[]", "unlisted: no note over 0.00");
assert.equal(JSON.stringify(ui.lockNotesFor(item("42"))), "[]", "unlisted: no note over an amount");
listNotes = true;
await openTest();
// The one-click path: Edit penalties, then Use the computed amounts.
click("fp-penalty-edit-0:1");
frame();
click("fp-penalty-computed-0:1");
await settle();
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify(Object.fromEntries(PENALTIES.map((k) => [k, ""]))), "the three boxes blanked in one batch");
assert.equal(ui.state.values[PENALTIES[0]], "11,250.00", "the computed amounts show");
assert.equal(ui.penaltyGroupOf(PENALTIES[0]).kept, false, "no longer kept");
assert.equal(JSON.stringify(ui.lockNotesFor(item("19"))), "[]", "computed amounts need no note");
assert.equal(JSON.stringify(ui.lockNotesFor(item("42"))), JSON.stringify([NOTE_42]), "item 42 stays listed");
console.log("PASS P4-14/P4-17: kept penalties and item 42 say why over their amounts; Use the computed amounts still works.");

// ---- P4-10: item 6 offers Part VII item 8's amount; Use it fills it.
kept = true;
await openTest();
const before = fills().length;
frame();
assert.ok(ids.has("fp-from-0:5"), "the hint is drawn under item 6");
assert.ok(texts.includes("From Part VII item 8: ₱1,500.00"), "it shows Part VII item 8's amount");
assert.ok(!ids.has("fp-from-0:4"), "no other item has one");
assert.equal(fills().length, before, "nothing is filled by itself");
click("fp-from-use-0:5");
await settle();
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [K("Part8_6A")]: "1,500.00" }), "Use it writes the amount through the ordinary fill");
assert.equal(ui.state.inputs.get(K("Part8_6A")).value(), "1,500.00", "the box shows it");
assert.equal(ui.state.values[K("Part8_6A")], "1,500.00");
// A return that is no longer a draft offers nothing.
ui.state.editable = false;
frame();
assert.ok(!ids.has("fp-from-0:5"), "no hint on a filed return");
console.log("PASS P4-10: 1701 Part VIII item 6 offers Part VII item 8's amount, filled only on Use it.");
