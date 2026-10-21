// The form page's parts column is a lazy `list`, not a column rebuilt on every
// frame: one flat item stream keys the page head, the narrow chip navigator,
// each part header, each "From tax profile" banner and every visible row, and
// list items embed retained views instead of creating state. Driven through
// the real app/shell/form_page.js with the host's own 1604C fixture (tools/
// fixtures/form_1604_page.json); this proves:
//  - both layouts hand the stream to list("fp-parts") with a paired vertical
//    scrollbar — wide keeps the pinned navigator, narrow makes the chips the
//    list's second item;
//  - item keys are stable and unique across renders, and the stream's length
//    follows what is open: a folded part contributes only its header;
//  - rendering a list item creates no entities, inputs or focus handles —
//    everything with handlers was retained when the part opened;
//  - a header item's click folds the part through the list's on_item_click;
//  - a part whose rows the profile filled gets a `g:` banner item, and its
//    tucked `r:` rows exist only while the fold is open.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);
const FIX = JSON.parse(fs.readFileSync(new URL("tools/fixtures/form_1604_page.json", root), "utf8"));

const entities = [], tasks = [], texts = [], timers = [];
const ids = new Set();
const lists = [], scrollbars = [], scrollCalls = [];
let owner = null, focusHandles = 0, inputsMade = 0;
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
    focus_handle() { focusHandles++; return { focus() {}, is_focused: () => false, release() { return true; } }; },
    stop_propagation() {},
    theme() { return { appearance: "light", is_dark: false, colors: {} } },
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
function mount(e) { if (e?.view && e.dirty && !e.released) { e.dirty = false; const old = owner; owner = e; e.view.render(context(e)); owner = old; } }
const InputState = { new({ value = "" } = {}) { inputsMade++; let text = value; const on = {}; return { value: () => text, set_value: (v) => { text = v; }, on(ev, fn) { on[ev] = fn; }, fire(ev) { on[ev]?.({}, context()); }, release() { return true; } }; } };
const CalendarState = { new() { return { set_value() {}, value: () => null, year: () => 2026, month: () => 10, today: () => "2026-10-17", month_days: () => [[]], on() {}, release() { return true; } }; } };
const contextVM = vm.createContext({ console, Map, Set, JSON, Date, Math, Number, String, Object, Array, window: { viewport_size: () => ({ width: 1400, height: 900 }) } });
function mock(name, exports) { return new vm.SyntheticModule(Object.keys(exports), function () { for (const [k, v] of Object.entries(exports)) this.setExport(k, v); }, { context: contextVM, identifier: name }); }
// The list mock records the stream the page hands over — count, keys, item
// renderer and the click dispatch — without materializing a single item.
function listMock(id, count, getKey, renderItem) {
  const entry = { id, count, keys: Array.from({ length: count }, (_, i) => getKey(i)), renderItem, click: null };
  lists.push(entry);
  const inner = element();
  const wrap = new Proxy(inner, { get(o, k) {
    if (k === "then") return undefined;
    if (k === "on_item_click") return (fn) => { entry.click = fn; return wrap; };
    const v = Reflect.get(o, k);
    return (...args) => { v(...args); return wrap; };
  } });
  return wrap;
}
const kit = mock("gpui-kit", { div: element, svg: element, View, list: listMock });
const base = mock("gpui-base", { InputState, CalendarState, Popup: { new: element }, Scrollbar: { vertical: (id) => { scrollbars.push(id); return element(); } }, VirtualListScrollHandle: { new: () => ({ scroll_to_item(i, s) { scrollCalls.push([i, s]); }, scroll_to_bottom() { scrollCalls.push(["bottom"]); }, release() { return true; } }) } });
const components = mock("gpui-component", { Spinner: { new: element } });
const uiExports = { tokens: () => new Proxy({}, { get: () => "#000" }), SPACE: [0, 4, 8, 12, 16, 20, 24], RADIUS: {}, SIZE: { validationLine: 18 }, BREAK: { formNav: 760 } };
for (const n of ["Text", "Line", "Code", "Icon", "AppButton", "IconButton", "Avatar", "Chip", "Switch", "Checkbox", "TextField", "Dropdown", "PageHeader", "MetaLine", "Overlay", "CheckboxLabel"]) uiExports[n] = element;
uiExports.Text = (_cx, _variant, text) => { texts.push(String(text)); return element(); };
uiExports.Line = uiExports.Text;
const store = mock("./store.js", { formatTin: (x) => x, errorText: (e) => String(e && e.message ? e.message : e) });
const periods = mock("./periods.js", { periodLabel: () => "Q3 2026" });
const mod = new vm.SourceTextModule(fs.readFileSync(new URL("app/shell/form_page.js", root), "utf8"), { context: contextVM });
await mod.link((s) => ({ "gpui-kit": kit, "gpui-base": base, "gpui-component": components, "./ui.js": mock("./ui.js", uiExports), "./store.js": store, "./periods.js": periods })[s]);
await mod.evaluate();
const { FormPageActions, renderFormPage, renderFormSplit } = mod.namespace;

const app = {
  prefs: {}, branchFor: () => null, taxpayers: () => [], layout: () => ({ column: 1056 }),
  invalidate() {}, toast() {},
  store: {
    templates: () => [],
    write() { return { id: 7, status: "Draft" }; },
    call(name, ...args) {
      if (name === "screen_form_schema") return FIX[args[0]].schema;
      if (name === "screen_form_open") {
        const f = FIX[args[1]];
        return Promise.resolve({
          draft: f.typed.draft, status: "Draft", editable: true, values: { ...f.typed.values }, options: {},
          locks: f.locks, lock_notes: f.lock_notes, profile: args.profile || [], profile_edit: [], sample: false, blank: [], needs: {},
          off: {}, penalties: [], pools: {}, errors: [],
        });
      }
      return Promise.resolve({});
    },
  },
};
app.formUi = new FormPageActions(app);
const ui = app.formUi;
const cx = context();
async function settle() { while (tasks.length) await Promise.all(tasks.splice(0)); }
const WIDE = { column: 1056, padX: 32, mainWidth: 1200 };
const NARROW = { column: 700, padX: 32, mainWidth: 764 };
const lastList = () => lists.at(-1);

// Every part open: the stream is the head plus each part's header and rows.
ui.open({ code: "1604C", tin: "12345678800000", year: 2026, period: 0 }, cx);
await settle();
const parts = ui.state.schema.parts;
for (let i = 0; i < parts.length; i++) ui.ensureInputs(i, cx);
ui.state.openParts = new Set(parts.map((_p, i) => i));

renderFormSplit(cx, app, WIDE);
let L = lastList();
assert.equal(L.id, "fp-parts", "the parts column is the named list");
assert.ok(scrollbars.includes("fp-parts"), "a vertical scrollbar pairs with it");
const expected = 1 + parts.length + parts.reduce((n, p) => n + p.items.length, 0);
assert.equal(L.count, expected, "head + a header and a row per visible item");
assert.equal(L.keys[0], "P", "item 0 is the page head");
assert.equal(L.keys[1], "h:0", "item 1 is the first part's header");
assert.equal(new Set(L.keys).size, L.keys.length, "keys are unique");
assert.ok(L.keys.every((k) => k === "P" || k === "N" || /^[hgr]:\d+/.test(k)), "keys are domain-derived");

// Stable across renders: same stream, same keys.
renderFormSplit(cx, app, WIDE);
assert.deepEqual(lastList().keys, L.keys, "the item stream keys are stable");

// A lazy item renderer may not create state: entities, inputs and focus
// handles were all retained when the parts opened.
{
  const before = { e: entities.length, i: inputsMade, f: focusHandles };
  for (let i = 0; i < L.count; i++) mount(L.renderItem(i, cx));
  assert.deepEqual({ e: entities.length, i: inputsMade, f: focusHandles }, before, "rendering items creates nothing retained");
}

// A header item's click folds its part through the list's dispatch; the stream
// then drops that part's rows while every other key holds its place.
assert.ok(L.click, "the list wires on_item_click");
const part1 = parts[1].items.length;
assert.ok(ui.state.openParts.has(1));
L.click("h:1", cx);
assert.ok(!ui.state.openParts.has(1), "clicking a header item folds the part");
renderFormSplit(cx, app, WIDE);
L = lastList();
assert.equal(L.count, expected - part1, "a folded part contributes only its header");
assert.equal(L.keys[0], "P");
assert.ok(L.keys.includes("h:1") && L.keys.includes("h:2"), "the other headers hold their keys");
L.click("h:1", cx);
assert.ok(ui.state.openParts.has(1), "and it opens again");
console.log("PASS the parts list is a stable keyed stream and header clicks fold parts");

// jumpTo opens one part, folds the rest and asks the scroll handle to land
// the part's header at the top of the (now short) stream.
ui.jumpTo(2, cx);
assert.equal([...ui.state.openParts].join(), "2", "jumpTo folds the other parts");
renderFormSplit(cx, app, WIDE);
assert.equal(scrollCalls.at(-1)[1], "top", "jumpTo scrolls with the list's handle");
assert.equal(scrollCalls.at(-1)[0], lastList().keys.indexOf("h:2"), "…to the part header's index in the new stream");
ui.state.openParts = new Set(parts.map((_p, i) => i));
console.log("PASS jumpTo scrolls the chosen part's header to the top");

// Narrow windows: the chip navigator rides inside the same list as item 1.
renderFormSplit(cx, app, NARROW);
L = lastList();
assert.equal(L.keys[0], "P");
assert.equal(L.keys[1], "N", "the chips are the list's second item below 760 px");
assert.equal(L.count, expected + 1, "the stream gains only the navigator item");
// The eager path the tests use keeps the same stream (minus the head, which
// the page wrapper already drew).
renderFormPage(cx, app, 700);
assert.ok([...ids].some((id) => id.startsWith("fp-nav-")), "the eager path still draws the chips");
console.log("PASS the narrow layout virtualizes too, chips inside the list");

// A part whose items the profile filled whole fold into one banner item; its
// tucked rows enter the stream only while the fold is open.
{
  const pi = parts.findIndex((p) => p.items.some((it) => !it.rows && it.l !== "Additional details" && (it.f || []).length && it.f.every((f) => !f.c)));
  assert.ok(pi >= 0, "the fixture has an all-typed item to tuck");
  const it = parts[pi].items.find((x) => !x.rows && x.l !== "Additional details" && (x.f || []).length && x.f.every((f) => !f.c));
  const ii = parts[pi].items.indexOf(it);
  for (const f of it.f) ui.state.profile.add(f.k);
  ui.ensureInputs(pi, cx); // the fold's banner is a retained view now
  renderFormSplit(cx, app, WIDE);
  L = lastList();
  assert.ok(L.keys.includes(`g:${pi}`), "the fold banner is its own item");
  assert.ok(!L.keys.includes(`r:${pi}:${ii}`), "tucked rows stay out while folded");
  ui.state.profileOpen.add(pi);
  renderFormSplit(cx, app, WIDE);
  L = lastList();
  assert.ok(L.keys.includes(`r:${pi}:${ii}`), "and joins the stream when open");
  ui.state.profileOpen.delete(pi);
}
console.log("PASS the profile fold keeps its banner item and reveals its rows only when open");
