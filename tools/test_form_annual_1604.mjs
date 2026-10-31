// The form page as the annual information returns (PR 6: 1604C, 1604F,
// 1604E) use it, driven through the real app/shell/form_page.js. The host
// side is the host's own: tools/fixtures/form_1604_page.json holds the
// real schema entries of 1604C and 1604F, their opened locks, the drafts
// and values a typed return has, and what item 11 No returns (draft,
// values and removes), pinned by crates/bir-shell
// screen_form_module/tests/annual_1604.rs pr6_page_fixture_is_the_hosts.
// This proves what the page does with it:
//  - a Part II schedule draws its printed column heads, each row's printed
//    month or quarter caption in place of a row number (the layout's
//    `numbered: false`), and its TOTAL row; without that flag the number
//    column comes back beside the captions;
//  - item 11 No on 1604C (clears items 11A to 13) and on 1604F (clears item
//    11A, tax_relief_details) asks first: Cancel keeps the answer and what
//    was typed, Continue takes the host's draft and offers the host's Undo
//    text, and Undo brings the typed draft back;
//  - a date box holding text that is not a date says so only once the
//    filer has edited it (a stored legacy "4/10/26" opens and blurs quietly).
// The page code is PR 2's (fix/findings-q3-withholding).
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);
const FIX = JSON.parse(fs.readFileSync(new URL("tools/fixtures/form_1604_page.json", root), "utf8"));
const DATE_ERROR = "Enter a valid date as MM/DD/YYYY (e.g. 12/01/2020 or 12012020)";

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
const InputState = { new({ value = "" } = {}) { let text = value; const on = {}; return { value: () => text, set_value: (v) => { text = v; }, on(ev, fn) { on[ev] = fn; }, fire(ev) { on[ev]?.({}, context()); }, release() { return true; } }; } };
const CalendarState = { new() { return { set_value() {}, value: () => null, year: () => 2026, month: () => 10, today: () => "2026-10-17", month_days: () => [[]], on() {}, release() { return true; } }; } };
const contextVM = vm.createContext({ console, Map, Set, JSON, Date, Math, Number, String, Object, Array, window: { viewport_size: () => ({ width: 1400, height: 900 }) } });
function mock(name, exports) { return new vm.SyntheticModule(Object.keys(exports), function () { for (const [k, v] of Object.entries(exports)) this.setExport(k, v); }, { context: contextVM, identifier: name }); }
const kit = mock("gpui-kit", { div: element, svg: element, View, list: (_id, n, _key, render) => { const el = element(); for (let i = 0; i < n; i++) el.child(render(i)); return el; } });
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

const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
// The host's values for a draft it returned.
const valuesFor = (code, draft) => {
  const f = FIX[code];
  return same(draft, f.cleared.draft) ? f.cleared.values : f.typed.values;
};
const app = {
  prefs: {}, branchFor: () => null, taxpayers: () => [], layout: () => ({ column: 1056 }),
  invalidate() {}, toast() {}, startNewReturn() {},
  toastUndo(message, undo) { undos.push({ message, undo }); app.toastState = { message, action: { label: "Undo", run: undo } }; },
  store: {
    templates: () => [],
    write(name, ...args) { calls.push([name, ...args]); return { id: 7, status: "Draft", draft: args[1] }; },
    call(name, ...args) {
      calls.push([name, ...args]);
      if (name === "screen_form_schema") return FIX[args[0]].schema;
      if (name === "screen_form_open") {
        const f = FIX[args[1]];
        return Promise.resolve({
          draft: f.typed.draft, status: "Draft", editable: true, values: { ...f.typed.values }, options: {},
          locks: f.locks, lock_notes: f.lock_notes, profile: [], profile_edit: [], sample: false, blank: [], needs: {},
          off: {}, penalties: [], pools: {}, errors: [],
        });
      }
      if (name === "screen_form_fill") {
        const [code, draft, edits] = args;
        const f = FIX[code];
        const no = Object.entries(f.no).every(([k, v]) => edits[k] === v);
        if (no && same(draft, f.typed.draft)) {
          return { draft: f.cleared.draft, values: { ...f.cleared.values }, options: {}, rejected: {}, penalties: [], pools: {}, removes: f.cleared.removes };
        }
        return { draft, values: { ...valuesFor(code, draft), ...edits }, options: {}, rejected: {}, penalties: [], pools: {}, removes: null };
      }
      if (name === "screen_form_validate") {
        const [code, draft] = args;
        return Promise.resolve({ draft, values: { ...valuesFor(code, draft) }, options: {}, errors: [], off: {}, penalties: [], pools: {} });
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
const click = (id) => { const fn = clicks.get(id); assert.ok(fn, `button ${id}`); act(fn, {}); };
const ask = () => { clicks.clear(); dismiss.clear(); texts.length = 0; renderRemovalConfirm(cx, app); };
const count = (t) => texts.filter((x) => x === t).length;

async function openForm(code) {
  ui.open({ code, tin: "12345678800000", year: 2026, period: 0 }, cx);
  await settle();
  for (let i = 0; i < ui.state.schema.parts.length; i++) ui.ensureInputs(i, cx);
  ui.state.openParts = new Set(ui.state.schema.parts.map((_p, i) => i));
  frame();
}

for (const code of ["1604C", "1604F"]) {
  const f = FIX[code];
  await openForm(code);
  // ---- Part II: printed heads, captions in place of numbers, TOTAL rows.
  const tables = ui.state.schema.parts.flatMap((p) => p.items).filter((it) => it.rows);
  assert.equal(tables.length, code === "1604C" ? 1 : 3, code);
  for (const it of tables) {
    assert.equal(it.numbered, false, `${code} ${it.l}: the layout replaces the numbers`);
    assert.ok(texts.includes(it.l), `${code}: the title ${it.l}`);
    assert.ok(texts.includes(it.row_labels.caption), `${code}: ${it.row_labels.caption}`);
    for (const c of it.cols) assert.ok(texts.includes(c.l), `${code}: column ${c.l}`);
    for (const r of it.row_labels.rows) assert.ok(texts.includes(r), `${code}: row ${r}`);
  }
  assert.ok(count("TOTAL") >= tables.length, `${code}: a TOTAL row per schedule`);
  // Without `numbered: false` each table gains its "#" head and numbers.
  const before = Object.fromEntries(["#", "1", "4", "12"].map((t) => [t, count(t)]));
  for (const it of tables) delete it.numbered;
  frame();
  for (const it of tables) for (const r of it.row_labels.rows) assert.ok(texts.includes(r), `${code}: captions stay beside numbers`);
  const rows = tables.map((it) => it.rows);
  assert.equal(count("#") - before["#"], tables.length, `${code}: a # column per table`);
  assert.equal(count("1") - before["1"], tables.length, `${code}: row 1 numbered`);
  assert.equal(count("4") - before["4"], rows.filter((n) => n >= 4).length, `${code}: row 4 numbered`);
  assert.equal(count("12") - before["12"], rows.filter((n) => n >= 12).length, `${code}: row 12 numbered`);
  for (const it of tables) it.numbered = false;
  frame();
  console.log(`PASS ${code} Part II shows its printed heads, row captions in place of numbers, and TOTAL rows.`);

  // ---- Item 11 No asks first.
  const kept = ui.state.draft;
  assert.ok(same(kept, f.typed.draft), `${code}: the typed return`);
  ui.choose(f.no, cx);
  assert.equal(app.overlay, "removal", `${code} item 11 No asks first`);
  assert.equal(ui.state.draft, kept, `${code}: the draft is the old one while it asks`);
  ask();
  assert.ok(texts.includes(f.cleared.removes.message), `${code}: ${f.cleared.removes.message}`);
  click("removal-cancel");
  assert.equal(app.overlay, null, `${code}: Cancel closes it`);
  assert.equal(ui.state.draft, kept, `${code}: Cancel keeps what was typed`);
  ui.choose(f.no, cx);
  ask();
  click("removal-continue");
  await settle();
  assert.ok(same(ui.state.draft, f.cleared.draft), `${code}: Continue takes the host's draft`);
  if (code === "1604F") {
    assert.equal(kept.tax_relief_details, "PEZA");
    assert.equal(ui.state.draft.tax_relief_details, "", "item 11A is cleared");
    assert.equal(ui.state.values["frm1604f:taxRelief_2"], "true", "and item 11 reads No");
  } else {
    assert.equal(ui.state.draft.refund_date, "", "item 11A is cleared");
    assert.equal(ui.state.draft.overremittance, 0, "item 12 too");
  }
  assert.equal(undos.at(-1).message, `${f.cleared.removes.done}.`, `${code}: the host's Undo text`);
  act(undos.at(-1).undo);
  await settle();
  assert.ok(same(ui.state.draft, f.typed.draft), `${code}: Undo brings the typed return back`);
  if (code === "1604F") assert.equal(ui.state.draft.tax_relief_details, "PEZA");
  console.log(`PASS ${code} item 11 No asks first; Cancel keeps, Continue clears, Undo brings back.`);
}

// ---- A date box with text that is not a date (a stored legacy "4/10/26").
{
  const k = "frm1604f:txtSched1Date2";
  const input = ui.state.inputs.get(k);
  assert.ok(input, "the date box has an input");
  input.set_value("4/10/26");
  input.fire("focus");
  input.fire("blur");
  assert.ok(!Object.values(ui.state.dateErrors || {}).includes(DATE_ERROR), "untouched, it blurs quietly");
  input.fire("change");
  input.fire("blur");
  assert.ok(Object.values(ui.state.dateErrors || {}).includes(DATE_ERROR), "edited, it says what a date is");
  input.set_value("04/10/2026");
  input.fire("change");
  input.fire("blur");
  assert.ok(!Object.values(ui.state.dateErrors || {}).includes(DATE_ERROR), "a whole date clears the message");
}
console.log("PASS a date box says it holds no date only after the filer edits it.");
