// PR 5: what the 1701Q page does that the shared mechanisms do not cover,
// driven through the real app/shell/form_page.js with a mocked host and a
// small form: item 11's month, day and year boxes (the host marks the item
// `one_date`, screen_form_module/rows_income_q.rs mark_one_date) are one
// date box. Typed (05171980, 05/17/1980, "May 17, 1980") or picked, the
// date reaches the host as its three boxes in one batch, MM, DD and YYYY;
// a blank box clears the three; a date that is not one says so and sends
// nothing; the box follows the three boxes when the host changes them (an
// Undo); a locked item shows the date. Which keys are parts of one date is
// the host's to say (crates/bir-shell screen_form_module tests).
// Decision 11's question: an answer that removes what the filer typed (Civil
// Status off Married, the host's `removes`) is asked first; Continue applies
// it and offers Undo, which brings the old draft back; a save, or any later
// edit, retires that Undo.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);

const K = (tail) => `frmTEST:${tail}`;
const CIVIL = "page:TEST:civilStatus";
const schema = {
  c: "TEST", t: "Test return", rev: "2026", freq: "Quarterly", pages: 1, n: 4, q: 1,
  parts: [
    {
      n: "Part I · Background Information",
      items: [
        { no: "9", l: "Taxpayer/Filer's Name", f: [{ k: K("name"), t: "text", l: "Taxpayer/Filer's Name" }] },
        {
          no: "11", l: "Date of Birth (MM/DD/YYYY)", one_date: true,
          f: [
            { k: K("txtBirthMonth"), t: "int", l: "Date of Birth" },
            { k: K("txtBirthDay"), t: "int", l: "Date of Birth" },
            { k: K("txtBirthYear"), t: "int", l: "Date of Birth" },
          ],
        },
        { no: "", l: "Civil Status", f: [{ k: CIVIL, t: "sel", l: "Civil Status", o: ["Single", "Married", "Legally Separated", "Widow/er"] }] },
        { no: "21", l: "Spouse's Name", f: [{ k: K("spouse"), t: "text", l: "Spouse's Name" }] },
        {
          no: "12", l: "Date of Registration (three boxes)",
          f: [
            { k: K("regMonth"), t: "int", l: "Month" },
            { k: K("regDay"), t: "int", l: "Day" },
            { k: K("regYear"), t: "int", l: "Year" },
          ],
        },
      ],
    },
  ],
};
const [MONTH, DAY, YEAR] = ["txtBirthMonth", "txtBirthDay", "txtBirthYear"].map(K);
const ONE = `${MONTH}#date`;
const idOf = () => "0:1";

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
const kit = mock("gpui-kit", { div: element, svg: element, View, list: (_id, n, _key, render) => { const el = element(); for (let i = 0; i < n; i++) el.child(render(i)); return el; } });
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
const { FormPageActions, renderFormPage, renderFormSplit, oneDateText, NOTE_CHECKS } = mod.namespace;

let hostValues = { [CIVIL]: "Married", [K("spouse")]: "DELA CRUZ, MARIA", [K("name")]: "DELA CRUZ, JUAN", [MONTH]: "05", [DAY]: "17", [YEAR]: "1980", [K("regMonth")]: "01", [K("regDay")]: "02", [K("regYear")]: "2003" };
let editable = true;
const opened = () => ({
  draft: { id: 7 }, status: editable ? "Draft" : "Submitted", editable, values: { ...hostValues }, options: {}, locks: {},
  profile: [], profile_edit: [], sample: false, blank: [], needs: {}, off: {}, penalties: [], pools: {}, errors: [],
  // The host offers a one-time note only until it is recorded seen.
  notes: offeredNotes.filter((n) => !seenKeys.has(n.key)).map((n) => ({ ...n })),
});
// D3: the one-time notes the host offers, and the keys it has recorded seen
// (screen_form_notes_seen records each only if not yet recorded).
let offeredNotes = [];
const seenKeys = new Set();
const app = {
  prefs: {}, branchFor: () => null, taxpayers: () => [], layout: () => ({ column: 1056 }),
  invalidate() {}, toast() {}, startNewReturn() {},
  toastState: null,
  toastUndo(what, run) { this.toastState = { text: what, action: { run } }; },
  store: {
    templates: () => [],
    write(name, ...args) { calls.push([name, ...args]); return { id: 7, status: "Draft", draft: args[1] }; },
    call(name, ...args) {
      calls.push([name, ...args]);
      if (name === "screen_form_schema") return schema;
      if (name === "screen_form_open") return Promise.resolve(opened());
      if (name === "screen_form_fill") {
        // Like the host, a fill works on the draft it is given (an Undo
        // refills the draft it was offered with).
        const values = { ...(args[1].values || hostValues), ...args[2] };
        let removes = null;
        if (args[2][CIVIL] && args[2][CIVIL] !== "Married" && (args[1].values || hostValues)[K("spouse")]) {
          removes = { message: "Without Married at Civil Status, the spouse's part goes. Continue?", done: "Cleared the spouse's part", rows: 1 };
          values[K("spouse")] = "";
        }
        return { draft: { id: 7, values }, values: { ...values }, options: {}, rejected: {}, penalties: [], pools: {}, removes };
      }
      if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: { ...(args[1].values || hostValues) }, options: {}, errors: [], off: {}, penalties: [], pools: {} });
      if (name === "screen_form_preview_sync" || name === "screen_form_print") return true;
      if (name === "screen_form_notes_seen") {
        const fresh = args[0].filter((key) => !seenKeys.has(key));
        for (const key of fresh) seenKeys.add(key);
        return fresh;
      }
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
async function openAll() {
  ui.open({ code: "TEST", tin: "12345678800000", year: 2026, period: 2 }, cx);
  await settle();
  ui.ensureInputs(0, cx);
  ui.state.openParts = new Set([0]);
  frame();
}

// ---- The three boxes read as one date.
assert.equal(oneDateText({ [MONTH]: "05", [DAY]: "17", [YEAR]: "1980" }, { month: { k: MONTH }, day: { k: DAY }, year: { k: YEAR } }), "05/17/1980");
assert.equal(oneDateText({}, { month: { k: MONTH }, day: { k: DAY }, year: { k: YEAR } }), "", "three blank boxes are no date");
await openAll();
const one = ui.state.inputs.get(ONE);
assert.ok(one, "item 11 has one date box");
assert.equal(one.value(), "May 17, 1980", "it shows the three boxes as May 17, 1980");
for (const k of [MONTH, DAY, YEAR]) assert.equal(ui.state.inputs.has(k), false, `${k} has no box of its own`);
assert.ok(clicks.has(`fp-date-${idOf()}`), "with one calendar");
assert.ok(ui.state.inputs.has(K("regMonth")), "an item the host does not mark keeps its three boxes");
console.log("PASS 1701Q item 11: the month, day and year boxes show as one date.");

// ---- Typed: the three boxes go to the host together.
for (const typed of ["12102020", "12-10-2020", "12/10/2020", "Dec 10, 2020"]) {
  one.type("01012000");
  one.type(typed);
  assert.deepEqual([ui.state.pending[MONTH], ui.state.pending[DAY], ui.state.pending[YEAR]], ["12", "10", "2020"], `${typed} stages MM, DD and YYYY`);
  one.fire("blur");
  assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [MONTH]: "12", [DAY]: "10", [YEAR]: "2020" }), "one batch with the three boxes");
  assert.equal(one.value(), "Dec 10, 2020", `${typed} shows as Dec 10, 2020`);
}
await settle();
const before = fills().length;
one.type("13312020");
assert.equal(Object.keys(ui.state.pending).length, 0, "a date that is not one sends nothing");
one.fire("blur");
assert.equal(fills().length, before, "and fills nothing");
assert.match(ui.state.dateErrors[idOf()], /valid date/, "and says why");
assert.equal(one.value(), "13312020", "it stays as typed");
one.type("");
assert.deepEqual([ui.state.pending[MONTH], ui.state.pending[DAY], ui.state.pending[YEAR]], ["", "", ""], "a blank box clears the three");
assert.equal(ui.state.dateErrors[idOf()], undefined, "and has nothing to say");
one.fire("blur");
await settle();
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [MONTH]: "", [DAY]: "", [YEAR]: "" }));
console.log("PASS 1701Q item 11: a typed date reaches the draft as its three boxes; a blank clears them.");

// ---- Picked: the calendar fills the three and the box shows the day.
one.type("05171980");
one.fire("blur");
await settle();
frame();
act(clicks.get(`fp-date-${idOf()}`).fn, {});
const cal = ui.state.picker.cal;
assert.deepEqual([cal.year(), cal.month()], [1980, 5], "the calendar opens on the date");
cal.pick("1985-03-07");
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [MONTH]: "03", [DAY]: "07", [YEAR]: "1985" }), "a picked day fills the three boxes");
assert.equal(one.value(), "Mar 7, 1985", "and shows as Mar 7, 1985");
console.log("PASS 1701Q item 11: a picked day fills the three boxes.");

// ---- The box follows the three boxes when the host changes them (Undo).
ui.applyFill({ draft: { id: 7 }, values: { ...hostValues, [MONTH]: "01", [DAY]: "02", [YEAR]: "2003" }, options: {}, rejected: {}, penalties: [], pools: {} }, {});
assert.equal(one.value(), "Jan 2, 2003", "the host's three boxes show in the one box");
console.log("PASS 1701Q item 11: the date box follows the draft.");

// ---- A return that is no longer a draft shows the date, locked.
editable = false;
hostValues = { ...hostValues, [MONTH]: "05", [DAY]: "17", [YEAR]: "1980" };
await openAll();
assert.ok(texts.includes("May 17, 1980"), "a locked item shows the date");
assert.ok(!ids.has(`fp-date-${idOf()}`), "and no calendar");
console.log("PASS 1701Q item 11: a locked return shows the date.");

// ---- Decision 11: leaving Married asks first; Continue offers Undo.
editable = true;
hostValues = { ...hostValues, [CIVIL]: "Married", [K("spouse")]: "DELA CRUZ, MARIA" };
await openAll();
const asked = () => app.overlay === "removal";
ui.choose({ [CIVIL]: "Single" }, cx);
assert.ok(asked(), "the removal is asked first");
assert.equal(ui.state.values[CIVIL], "Married", "while asked, the old answer shows");
assert.equal(ui.state.values[K("spouse")], "DELA CRUZ, MARIA", "and the spouse's name stays");
ui.cancelRemoval(cx);
assert.equal(ui.state.values[K("spouse")], "DELA CRUZ, MARIA", "Cancel keeps both");
ui.choose({ [CIVIL]: "Single" }, cx);
ui.confirmRemoval(cx);
await settle();
assert.equal(ui.state.values[CIVIL], "Single", "Continue applies the answer");
assert.equal(ui.state.values[K("spouse")], "", "and clears the spouse's part");
assert.equal(app.toastState?.text, "Cleared the spouse's part.", "an Undo is offered");
const fillsBeforeUndo = fills().length;
act(app.toastState.action.run);
await settle();
assert.equal(fills().length, fillsBeforeUndo + 1, "Undo refills the old draft");
assert.equal(JSON.stringify(lastFill()[3]), "{}");
assert.equal(ui.state.values[CIVIL], "Married", "Undo brings the answer back");
assert.equal(ui.state.values[K("spouse")], "DELA CRUZ, MARIA", "and the spouse's name");
// A save retires the Undo: it would bring back a draft older than the saved one.
ui.choose({ [CIVIL]: "Widow/er" }, cx);
ui.confirmRemoval(cx);
await settle();
const undo = app.toastState.action.run;
ui.save(cx);
assert.equal(app.toastState, null, "a save retires the Undo");
const fillsAfterSave = fills().length;
act(undo);
assert.equal(fills().length, fillsAfterSave, "and a stale Undo does nothing");
assert.equal(ui.state.values[CIVIL], "Widow/er");
console.log("PASS 1701Q Civil Status: leaving Married asks first, Undo brings it back, a save retires the Undo.");
// ---- D3: a one-time note is recorded seen only once the page drew it.
// The mocked host is the real one's shape: it offers the note until the
// key is recorded seen, and records each key only if not yet recorded.
const NOTE = { key: "notice_seen:1701q_q3_due_nov15:7", text: "The third-quarter 1701Q is now due November 15, or the next business day." };
const noteCalls = () => calls.filter((c) => c[0] === "screen_form_notes_seen");
const WIDE = { column: 1056, padX: 24, mainWidth: 1280 };
function wideFrame() { owner = null; texts.length = 0; ids.clear(); opacity.clear(); for (const e of entities) e.dirty = true; return renderFormSplit(cx, app, WIDE); }
async function openOnly() {
  ui.open({ code: "TEST", tin: "12345678800000", year: 2026, period: 3 }, cx);
  await settle();
}
editable = false;
offeredNotes = [NOTE];
timers.length = 0;
// An opening the page never draws (it fails before its first frame):
// nothing is recorded, so the next opening offers the note again. It
// checks NOTE_CHECKS (50) times in all, then gives up.
let checks = 0;
const check = ui.recordNotesWhenShown.bind(ui);
ui.recordNotesWhenShown = (...args) => {
  checks += 1;
  return check(...args);
};
await openOnly();
assert.equal(ui.state.notes.length, 1, "the opening holds the note");
runTimers();
assert.equal(NOTE_CHECKS, 50);
assert.equal(checks, NOTE_CHECKS, "50 checks, then it gives up");
delete ui.recordNotesWhenShown;
assert.equal(noteCalls().length, 0, "not drawn, not recorded");
assert.equal(seenKeys.size, 0);
// Narrow window (renderFormPage): shown, then recorded once.
await openAll();
assert.ok(texts.includes(NOTE.text), "the note shows");
assert.equal(noteCalls().length, 0, "recorded only after the frame");
runTimers();
assert.deepEqual(noteCalls().at(-1)[1], [NOTE.key], "then recorded");
assert.ok(seenKeys.has(NOTE.key));
frame();
assert.ok(texts.includes(NOTE.text), "and it stays on this page");
runTimers();
assert.equal(noteCalls().length, 1, "recorded once");
// The next opening: the host offers nothing.
await openAll();
assert.ok(!texts.includes(NOTE.text), "the next opening is silent");
assert.equal(ui.state.notes.length, 0);
console.log("PASS D3 (narrow): a one-time note is recorded seen only after the page drew it, and shows once.");

// Wide window (renderFormSplit, 760 px and up): the same.
seenKeys.clear();
await openOnly();
assert.ok(wideFrame(), "the wide layout draws the page");
assert.ok(texts.includes(NOTE.text), "the note shows on the wide layout");
const recordsBefore = noteCalls().length;
runTimers();
assert.equal(noteCalls().length, recordsBefore + 1, "then is recorded");
assert.ok(seenKeys.has(NOTE.key));
await openOnly();
wideFrame();
assert.ok(!texts.includes(NOTE.text), "and the next opening is silent");
runTimers();
assert.equal(noteCalls().length, recordsBefore + 1, "nothing more to record");
console.log("PASS D3 (wide): a one-time note is recorded seen only after the split page drew it, and shows once.");

// Two openings at once: both were offered the note before either recorded
// it (the host offers it until it is recorded). The one drawn first records
// it; the other's record call records nothing, so that page drops it.
seenKeys.clear();
const other = new FormPageActions(app);
await openOnly();
other.open({ code: "TEST", tin: "12345678800000", year: 2026, period: 3 }, cx);
await settle();
assert.equal(ui.state.notes.length, 1, "this opening was offered the note");
assert.equal(other.state.notes.length, 1, "and so was the other");
frame();
assert.ok(texts.includes(NOTE.text), "this page draws the note");
app.formUi = other;
frame();
assert.ok(texts.includes(NOTE.text), "and the other page draws it too, before either records it");
runTimers();
assert.ok(seenKeys.has(NOTE.key), "the page whose record call ran first recorded it");
assert.equal(noteCalls().slice(-2).map((c) => c[1].length).join(","), "1,1", "both asked to record it");
frame();
assert.ok(!texts.includes(NOTE.text), "and drops it once its record call records nothing");
app.formUi = ui;
frame();
assert.ok(texts.includes(NOTE.text), "while the page that recorded it keeps it");
offeredNotes = [];
editable = true;
console.log("PASS D3: of two openings offered the note at once, only the one that records it keeps it.");
